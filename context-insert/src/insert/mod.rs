use context::*;
use result::InsertResult;

use crate::interval::init::InitInterval;
use context_search::*;
use context_trace::*;
pub mod context;
pub(crate) mod direction;
pub mod outcome;
pub mod result;

pub use outcome::InsertOutcome;

/// Trait for types that can create an InsertCtx for graph insertions.
///
/// With interior mutability, we only need `HasGraph` - mutations happen
/// through `&self` methods on `Hypergraph` using per-vertex locks.
pub trait ToInsertCtx<R: InsertResult = Token>: HasGraph {
    fn insert_context(&self) -> InsertCtx<R>;

    fn insert(
        &self,
        searchable: impl Searchable<InsertTraversal>,
    ) -> Result<R, ErrorState> {
        self.insert_context().insert(searchable)
    }
    fn insert_init(
        &self,
        ext: R::Extract,
        init: InitInterval,
    ) -> Result<R, ErrorState> {
        self.insert_context().insert_init(ext, init)
    }
    #[deprecated(
        since = "0.2.0",
        note = "Use `insert_next_match` instead. Returns `Result<InsertOutcome, ErrorReason>` — a flat enum replacing the confusing nested Result."
    )]
    #[allow(deprecated)]
    fn insert_or_get_complete(
        &self,
        searchable: impl Searchable<InsertTraversal>,
    ) -> Result<Result<R, R::Error>, ErrorReason> {
        self.insert_context().insert_or_get_complete(searchable)
    }
    fn insert_next_match(
        &self,
        searchable: impl Searchable<InsertTraversal>,
    ) -> Result<
        outcome::InsertOutcome,
        context_trace::graph::getters::ErrorReason,
    > {
        self.insert_context().insert_next_match(searchable)
    }
}

impl<R: InsertResult> ToInsertCtx<R> for HypergraphRef {
    fn insert_context(&self) -> InsertCtx<R> {
        InsertCtx::<R>::from(self.clone())
    }
}

// Implement for references - with interior mutability, we don't need &mut
impl<R: InsertResult, T: ToInsertCtx<R>> ToInsertCtx<R> for &'_ T {
    fn insert_context(&self) -> InsertCtx<R> {
        (**self).insert_context()
    }
}
