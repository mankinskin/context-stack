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
    pub fn insert(
        &mut self,
        searchable: impl Searchable<InsertTraversal>,
    ) -> Result<R, ErrorState> {
        self.insert_result(searchable)
            .and_then(|res| res.map_err(|root| root.into()))
    }
    pub fn insert_init(
        &mut self,
        ext: R::Extract,
        init: InitInterval,
    ) -> Result<R, ErrorState> {
        // Validate end_bound is not zero
        if *init.end_bound.as_ref() == 0 {
            return Err(ErrorReason::InvalidEndBound.into());
        }
        // With interior mutability, we just pass a reference to the graph
        let interval = IntervalGraph::from((&*self.graph, init));
        let mut ctx =
            FrontierSplitIterator::from((self.graph.clone(), interval));
        let joined = ctx.find_map(|joined| joined).unwrap();
        Ok(R::build_with_extract(joined, ext))
    }
    fn insert_result(
        &mut self,
        searchable: impl Searchable<InsertTraversal>,
    ) -> Result<Result<R, R::Error>, ErrorState> {
        match searchable.search(self.graph.clone()) {
            Ok(result) => {
                // Check if result is a full token AND the query was exhausted
                // EntireRoot + query_exhausted means the query exactly matches an existing token.
                // In this case, no insertion needed - just return the token.
                //
                // If is_full_token() but NOT query_exhausted(), it means we found a token
                // at the start of the query but there's more query remaining.
                if result.is_full_token() && result.query_exhausted() {
                    // Query fully matched an existing token - just return it
                    let query_path = result.query_cursor().path().clone();
                    let root_token = result.root_token();
                    Ok(R::try_init(IndexWithPath {
                        index: root_token,
                        path: query_path,
                    }))
                } else if result.is_full_token() && !result.query_exhausted() {
                    // EntireRoot + query not exhausted:
                    // Found a complete token at the start of the query, but there's more query.
                    // The cache won't have useful traversal data for splitting,
                    // so directly insert the query as a new pattern.
                    let query_path = result.query_cursor().path().clone();
                    let query_pattern: Vec<Token> = query_path
                        .pattern_root_pattern()
                        .iter()
                        .cloned()
                        .collect();
                    let new_token =
                        self.graph.insert_pattern(query_pattern.clone());

                    // Create a new path that indicates the entire query has been consumed
                    // (both start and end at the last index of the pattern)
                    let pattern_len = query_pattern.len();
                    let exhausted_path = RootedRangePath::new(
                        Pattern::from(query_pattern),
                        RolePath::new_empty(pattern_len - 1),
                        RolePath::new_empty(pattern_len - 1),
                    );

                    Ok(R::try_init(IndexWithPath {
                        index: new_token,
                        path: exhausted_path,
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
    pub fn insert_or_get_complete(
        &mut self,
        searchable: impl Searchable<InsertTraversal>,
    ) -> Result<Result<R, R::Error>, ErrorReason> {
        self.insert_result(searchable).map_err(|err| err.reason)
    }
}

// With interior mutability, HypergraphRef (Arc<Hypergraph>) just derefs to &Hypergraph
impl_has_graph! {
    impl<R: InsertResult> for InsertCtx<R>,
    self => &*self.graph;
    <'a> &'a Hypergraph
}
