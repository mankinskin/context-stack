use context::*;
use result::InsertResult;

use crate::legacy::interval::init::InitInterval;
use context_search::legacy::*;
use context_trace::legacy::*;
pub mod context;
pub(crate) mod direction;
pub mod outcome;
pub mod publication;
pub mod result;
pub mod scoped;

pub use outcome::{
    InsertOutcome,
    Materialization,
};
pub use publication::{
    Publication,
    PublicationStage,
    RangeReplacement,
};

/// Trait for types that can create an InsertCtx for graph insertions.
///
/// With interior mutability, we only need `HasGraph` - mutations happen
/// through `&self` methods on `Hypergraph` using per-vertex locks.
pub trait ToInsertCtx<R: InsertResult = Token>:
    HasGraph<Kind = BaseGraphKind>
{
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
    /// Canonical token for `scope`'s content before the postfix reached by
    /// `postfix_path`, represented inside `scope` (see [`scoped`]).
    fn insert_scope_prefix(
        &self,
        scope: Token,
        postfix_path: &[ChildLocation],
    ) -> Result<Token, ErrorState> {
        scoped::insert_scope_prefix(
            self.insert_context().graph_ref(),
            scope,
            postfix_path,
        )
    }
    /// Canonical token for `scope`'s content after the prefix reached by
    /// `prefix_path`, represented inside `scope` (see [`scoped`]).
    fn insert_scope_postfix(
        &self,
        scope: Token,
        prefix_path: &[ChildLocation],
    ) -> Result<Token, ErrorState> {
        scoped::insert_scope_postfix(
            self.insert_context().graph_ref(),
            scope,
            prefix_path,
        )
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
        context_trace::legacy::graph::getters::ErrorReason,
    > {
        self.insert_context().insert_next_match(searchable)
    }
    /// Materialize an accepted search response without searching again.
    fn materialize(
        &self,
        response: Response,
    ) -> Result<Materialization, ErrorReason> {
        self.insert_context()
            .materialize(response)
            .map_err(|err| err.reason)
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
