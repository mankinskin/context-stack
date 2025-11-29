use std::{
    fmt::Debug,
    sync::RwLockWriteGuard,
};

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
use context_trace::*;
use std::sync::RwLockReadGuard;

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
    ) -> R {
        let interval = IntervalGraph::from((&mut self.graph.graph_mut(), init));
        let mut ctx =
            FrontierSplitIterator::from((self.graph.clone(), interval));
        let joined = ctx.find_map(|joined| joined).unwrap();
        R::build_with_extract(joined, ext)
    }
    fn insert_result(
        &mut self,
        searchable: impl Searchable<InsertTraversal>,
    ) -> Result<Result<R, R::Error>, ErrorState> {
        match searchable.search(self.graph.clone()) {
            Ok(result) => {
                // Check if the query was exhausted and result is full token
                if result.query_exhausted() && result.is_full_token() {
                    // Extract the query pattern from the cursor and the root token from the complete path
                    let query_path = result.query_cursor().path().clone();
                    let root_token = result.root_token();
                    Ok(R::try_init(IndexWithPath {
                        index: root_token,
                        path: query_path,
                    }))
                } else {
                    // Query not exhausted - need to insert
                    Ok(Ok(self.insert_init(
                        <R::Extract as ResultExtraction>::extract_from(&result),
                        InitInterval::from(result),
                    )))
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
impl_has_graph! {
    impl<R: InsertResult> for InsertCtx<R>,
    self => self.graph.read().unwrap();
    <'a> RwLockReadGuard<'a, Hypergraph>
}
impl_has_graph_mut! {
    impl<R: InsertResult> for InsertCtx<R>,
    self => self.graph.write().unwrap();
    <'a> RwLockWriteGuard<'a, Hypergraph>
}
