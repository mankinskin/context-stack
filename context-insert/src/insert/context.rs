use std::fmt::Debug;

use crate::{
    insert::result::ResultExtraction,
    interval::{
        IntervalGraph,
        init::InitInterval,
    },
    join::{
        context::frontier::FrontierSplitIterator,
        plan::{
            JoinPlan,
            publish,
        },
        select::select,
    },
};
use context_search::{
    AncestorPolicy,
    Response,
    BftQueue,
    ErrorState,
    SearchKind,
    Searchable,
};
use context_trace::legacy::*;

use crate::insert::{
    outcome::{
        InsertOutcome,
        Materialization,
    },
    publication::Publication,
    result::InsertResult,
};

#[derive(Debug, Clone, Default)]
pub struct InsertTraversal;

impl TraceKind for InsertTraversal {
    type Trav = HypergraphRef;
}

impl SearchKind for InsertTraversal {
    type Container = BftQueue;
    type Policy = AncestorPolicy<Self::Trav>;
    type EndNode = PositionAnnotated<ChildLocation>;
}

#[derive(Debug)]
pub struct InsertCtx<R: InsertResult = Token> {
    graph: HypergraphRef,
    _ty: std::marker::PhantomData<R>,
}
impl<R: InsertResult> From<HypergraphRef> for InsertCtx<R> {
    fn from(graph: HypergraphRef) -> Self {
        Self {
            graph,
            _ty: Default::default(),
        }
    }
}

impl<R: InsertResult> InsertCtx<R> {
    pub(crate) fn graph_ref(&self) -> &HypergraphRef {
        &self.graph
    }
    #[context_trace::instrument_sig(level = "info", skip(self))]
    pub fn insert(
        &mut self,
        searchable: impl Searchable<InsertTraversal> + Debug,
    ) -> Result<R, ErrorState> {
        self.insert_impl(searchable)
            .and_then(|res| res.map_err(|root| root.into()))
    }
    #[deprecated(since = "0.2.0", note = "Use `insert_next_match` instead.")]
    #[allow(deprecated)]
    #[context_trace::instrument_sig(level = "info", skip(self))]
    pub(crate) fn insert_or_get_complete(
        &mut self,
        searchable: impl Searchable<InsertTraversal> + Debug,
    ) -> Result<Result<R, R::Error>, ErrorReason> {
        self.insert_impl(searchable).map_err(|err| err.reason)
    }
    #[context_trace::instrument_sig(level = "info", skip(self))]
    pub fn insert_next_match(
        &mut self,
        searchable: impl Searchable<InsertTraversal> + Debug,
    ) -> Result<InsertOutcome, ErrorReason> {
        self.insert_next_match_impl(searchable)
            .map_err(|err| err.reason)
    }
    pub(crate) fn insert_init(
        &mut self,
        ext: R::Extract,
        init: InitInterval,
    ) -> Result<R, ErrorState> {
        let (joined, _) = self.insert_init_published(init)?;
        Ok(R::build_with_extract(joined, ext))
    }

    /// Split, join and publish an interval, returning the target token and
    /// the edits needed to translate locations recorded before publication.
    pub(crate) fn insert_init_published(
        &self,
        init: InitInterval,
    ) -> Result<(Token, Publication), ErrorState> {
        use crate::visualization::emit_insert_node_with_delta;
        use context_trace::legacy::graph::visualization::{
            DeltaOp,
            GraphMutation,
            Transition,
        };

        let plan = self.plan_init(init)?;

        // Single publication boundary: the graph is only mutated here.
        let (joined, publication) =
            publish(&self.graph, select(&self.graph, plan));

        // Emit: Join complete — include delta for the result node
        emit_insert_node_with_delta(
            Transition::JoinComplete {
                result_node: joined.index.0,
            },
            format!("Join complete — created token {}", joined.index.0),
            joined.index.0,
            GraphMutation::single(DeltaOp::AddNode {
                index: joined.index.0,
                width: 0, // width resolved later
            }),
        );

        Ok((joined, publication))
    }

    /// Split and plan an insertion without mutating the graph.
    pub(crate) fn plan_init(
        &self,
        init: InitInterval,
    ) -> Result<JoinPlan, ErrorState> {
        use crate::visualization::{
            emit_insert_node,
            emit_insert_node_with_delta,
            reset_step_counter,
        };
        use context_trace::legacy::graph::visualization::{
            DeltaOp,
            GraphMutation,
            PathNode,
            Transition,
        };

        // Reset step counter for new insert operation
        reset_step_counter();

        // Validate end_bound is not zero
        if *init.end_bound.as_ref() == 0 {
            return Err(ErrorReason::InvalidEndBound.into());
        }

        // Validate that the cache contains an entry for the root token.
        // A missing entry means the TraceCache was built incorrectly
        // (e.g. context-read returned a root that was never traversed).
        // Without this check the split pipeline would panic on an empty
        // positions map inside OffsetIndexRange::get_splits.
        if !init.cache.entries.contains_key(&init.root.vertex_index()) {
            return Err(ErrorReason::MissingCacheEntry(
                init.root.vertex_index(),
            )
            .into());
        }

        let root_idx = init.root.index.0;
        let root_width = init.root.width.0;

        // Emit: Split phase starting
        emit_insert_node(
            Transition::SplitStart {
                node: PathNode {
                    index: root_idx,
                    width: root_width,
                },
                split_position: *init.end_bound.as_ref(),
            },
            format!("Starting split phase on root {root_idx}"),
            root_idx,
        );

        // With interior mutability, we just pass a reference to the graph
        let interval = IntervalGraph::try_from_init(&*self.graph, init)
            .map_err(|e| ErrorState::from(e))?;

        // Emit: Split phase complete — include delta with split info
        {
            let mut ops = Vec::new();
            ops.push(DeltaOp::UpdateNode {
                index: root_idx,
                detail: "Split phase completed".to_string(),
            });
            emit_insert_node_with_delta(
                Transition::SplitComplete {
                    original_node: root_idx,
                    left_fragment: None,
                    right_fragment: None,
                },
                format!("Split phase complete for root {root_idx}"),
                root_idx,
                GraphMutation::new(ops),
            );
        }

        // Emit: Join phase starting
        let leaf_count = interval.states.leaves.len();
        emit_insert_node(
            Transition::JoinStart {
                nodes: interval
                    .states
                    .leaves
                    .iter()
                    .map(|k| k.index.index.0)
                    .collect(),
            },
            format!("Starting join phase with {leaf_count} leaves"),
            root_idx,
        );

        Ok(FrontierSplitIterator::from((self.graph.clone(), interval)).plan())
    }
    fn insert_impl(
        &mut self,
        searchable: impl Searchable<InsertTraversal>,
    ) -> Result<Result<R, R::Error>, ErrorState> {
        match searchable.search(self.graph.clone()) {
            Ok(result) => {
                // Check if result is a full token AND the query was exhausted
                // EntireRoot + query_exhausted means the query exactly matches an existing token.
                // In this case, no insertion needed - just return the token.
                //
                // If is_entire_root() but NOT query_exhausted(), it means we found a token
                // at the start of the query but there's more query remaining.
                if result.is_entire_root() && result.query_exhausted() {
                    // Query fully matched an existing token - just return it
                    let query_path = result.query_cursor().path().clone();
                    let root_token = result.root_token();
                    Ok(R::try_init(IndexWithPath {
                        index: root_token,
                        path: query_path,
                    }))
                } else if result.is_entire_root() && !result.query_exhausted() {
                    // EntireRoot + query not exhausted:
                    // Found a complete token at the start of the query, but there's more query.
                    // Return the matched token with cursor position indicating consumed portion.
                    let root_token = result.root_token();
                    let query_path = result.query_cursor().path().clone();

                    Ok(R::try_init(IndexWithPath {
                        index: root_token,
                        path: query_path,
                    }))
                } else {
                    // Partial match (Range/Prefix/Postfix) - need to insert to resolve
                    self.insert_init(
                        <R::Extract as ResultExtraction>::extract_from(&result),
                        InitInterval::from(result),
                    )
                    .map(Ok)
                }
            },
            Err(err) => Err(err),
        }
    }

    /// Core implementation for `insert_next_match`: search, then
    /// materialize the response.
    fn insert_next_match_impl(
        &mut self,
        searchable: impl Searchable<InsertTraversal>,
    ) -> Result<InsertOutcome, ErrorState> {
        let response = searchable.search(self.graph.clone())?;
        self.materialize(response).map(|m| m.outcome)
    }

    /// Materialize an already accepted search response without searching
    /// its query again.
    ///
    /// - `Complete`: the query exactly matches an existing token.
    /// - `NoExpansion`: an existing token matches a proper prefix of the
    ///   query.
    /// - `Created`: the matched range is split and joined into a new token.
    ///
    /// The returned [`Publication`] is empty unless a token was created.
    /// `IndexWithPath::path` is the *query* cursor path, not a graph path.
    pub fn materialize(
        &self,
        response: Response,
    ) -> Result<Materialization, ErrorState> {
        let query_path = response.query_cursor().path().clone();
        if response.is_entire_root() {
            let result = IndexWithPath {
                index: response.root_token(),
                path: query_path,
            };
            let outcome = if response.query_exhausted() {
                InsertOutcome::Complete { result, response }
            } else {
                InsertOutcome::NoExpansion { result, response }
            };
            return Ok(Materialization {
                outcome,
                publication: Publication::default(),
            });
        }
        // InitInterval consumes the response; keep it for the outcome.
        let (token, publication) =
            self.insert_init_published(InitInterval::from(response.clone()))?;
        Ok(Materialization {
            outcome: InsertOutcome::Created {
                result: IndexWithPath {
                    index: token,
                    path: query_path,
                },
                response,
            },
            publication,
        })
    }
}

// With interior mutability, HypergraphRef (Arc<Hypergraph>) just derefs to &Hypergraph
impl_has_graph! {
    impl<R: InsertResult> for InsertCtx<R>,
    self => &*self.graph;
    <'a> &'a Hypergraph
}
