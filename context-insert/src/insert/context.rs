use std::fmt::Debug;

use crate::{
    insert::result::ResultExtraction,
    interval::{
        IntervalGraph,
        init::InitInterval,
    },
    join::context::frontier::FrontierSplitIterator,
};
use context_search::{
    AncestorPolicy,
    BftQueue,
    ErrorState,
    SearchKind,
    Searchable,
};
use context_trace::{
    PatternRoot,
    *,
};

use crate::insert::result::InsertResult;

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
    #[context_trace::instrument_sig(level = "info", skip(self))]
    pub fn insert(
        &mut self,
        searchable: impl Searchable<InsertTraversal> + Debug,
    ) -> Result<R, ErrorState> {
        self.insert_impl(searchable)
            .and_then(|res| res.map_err(|root| root.into()))
    }
    #[context_trace::instrument_sig(level = "info", skip(self))]
    pub(crate) fn insert_or_get_complete(
        &mut self,
        searchable: impl Searchable<InsertTraversal> + Debug,
    ) -> Result<Result<R, R::Error>, ErrorReason> {
        self.insert_impl(searchable).map_err(|err| err.reason)
    }
    pub(crate) fn insert_init(
        &mut self,
        ext: R::Extract,
        init: InitInterval,
    ) -> Result<R, ErrorState> {
        use crate::visualization::{
            emit_insert_node,
            reset_step_counter,
        };
        use context_trace::graph::visualization::Transition;

        // Reset step counter for new insert operation
        reset_step_counter();

        // Validate end_bound is not zero
        if *init.end_bound.as_ref() == 0 {
            return Err(ErrorReason::InvalidEndBound.into());
        }

        let root_idx = init.root.index.0;

        // Emit: Split phase starting
        emit_insert_node(
            Transition::SplitStart {
                node: root_idx,
                split_position: *init.end_bound.as_ref(),
            },
            format!("Starting split phase on root {root_idx}"),
            root_idx,
        );

        // With interior mutability, we just pass a reference to the graph
        let interval = IntervalGraph::from((&*self.graph, init));

        // Emit: Split phase complete
        emit_insert_node(
            Transition::SplitComplete {
                original_node: root_idx,
                left_fragment: None,
                right_fragment: None,
            },
            format!("Split phase complete for root {root_idx}"),
            root_idx,
        );

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

        let mut ctx =
            FrontierSplitIterator::from((self.graph.clone(), interval));
        let joined = ctx.find_map(|joined| joined).unwrap();

        // Emit: Join complete
        emit_insert_node(
            Transition::JoinComplete {
                result_node: joined.index.0,
            },
            format!("Join complete — created token {}", joined.index.0),
            joined.index.0,
        );

        Ok(R::build_with_extract(joined, ext))
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
}

// With interior mutability, HypergraphRef (Arc<Hypergraph>) just derefs to &Hypergraph
impl_has_graph! {
    impl<R: InsertResult> for InsertCtx<R>,
    self => &*self.graph;
    <'a> &'a Hypergraph
}
