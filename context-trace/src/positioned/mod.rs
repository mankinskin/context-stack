//! Positioned composition model.
//!
//! Every retained vertex lists its maximal retained proper child occurrences
//! by atom offset (see the oracle plan in
//! `ngrams/agents/plans/20261118_PLAN_POSITIONED_COMPOSITION_ORACLE.md`).
//! Independent of the row model in `context-trace-legacy`; shares only the ID and
//! atom types ([`crate::VertexIndex`], [`crate::Token`], [`crate::GraphKind`]).
//!
//! Layout:
//! - `model/`: data types of the cover model (`Occurrence`,
//!   `Composition`, `OccurrenceLocation`, `VertexNode`,
//!   `PositionedVertex`, `PositionedError`);
//! - `store/`: store abstraction traits (staging plan §3): [`GraphRead`]
//!   hands out [`VertexRead`] vertex objects (`store/read.rs`),
//!   `CompositionRead` (`store/composition.rs`), the sealed writes behind
//!   `GraphWrite` (`store/write.rs`), [`HasGraph`] / [`HasGraphMut`]
//!   abstract how a store is reached (`store/has_graph.rs`), `Navigate`
//!   (`store/navigate.rs`);
//! - `committed/`: the committed store ([`PositionedGraph`], its lock guards
//!   and [`PositionedGraphRef`]) implements them (`committed/access.rs`);
//! - mutation (`GraphEdit`, the `Stage` contract, dissolution and
//!   stabilization) lives in `context_insert::edit`, the overlay stage of a
//!   read in `context_read::stage`; both write through the sealed raw
//!   primitives (`store/write.rs`, hidden feature `edit`);
//! - `interchange`: content-keyed snapshots, with conversion (feature
//!   `interchange`), `diff` and `fixtures` in `test-api` builds;
//! - `committed/serde.rs` (feature `serde`): validated (de)serialization of
//!   [`PositionedGraph`], `committed/table.rs`: table construction;
//! - `check`: store-generic structural checks (hidden; shared with the
//!   higher crates).
//!
//! Read-only navigation (reader plan Phase 2) is split into separate APIs:
//! - content reading (debug and `test-api` builds):
//!   [`Navigate::read_interval`];
//! - local navigation for search, on vertex objects ([`Navigate`],
//!   `store/navigate.rs`): `start_at`, `prefix_child`, `postfix_child`,
//!   `prefixes`, `postfixes`, `parent_occurrences`; atoms through
//!   [`GraphRead::atom`]; end paths (`end_at`, `advance_end_path`) are
//!   `context_search::path::PathNavigate`;
//! - search paths, trace states, trace cache and cache keys live in
//!   `context_search::{path, trace}` (search plan 2.2).

#[doc(hidden)]
pub mod check;
mod committed;
pub mod interchange;
mod model;
mod store;

#[cfg(any(test, feature = "test-api"))]
pub use self::interchange::{
    diff,
    fixtures,
};

#[cfg(feature = "edit")]
#[doc(hidden)]
pub use self::store::raw;
#[cfg(debug_assertions)]
pub use self::model::PositionedLabels;
#[cfg(any(debug_assertions, test, feature = "test-api"))]
pub use self::model::IntervalView;
#[cfg(any(test, feature = "test-api"))]
pub use self::check::validate_store;
#[cfg(any(test, feature = "test-api"))]
pub use self::interchange::diff::{
    GraphDiff,
    diff,
};
pub use self::{
    committed::{
        PositionedGraph,
        PositionedGraphRef,
        PositionedVertexRef,
    },
    model::{
        Composition,
        CompositionError,
        Occurrence,
        OccurrenceLocation,
        PositionedError,
        PositionedVertex,
        VertexNode,
        maximal,
        slice,
    },
    store::{
        CompositionRead,
        Generation,
        GraphAtom,
        GraphRead,
        GraphWrite,
        HasGraph,
        HasGraphMut,
        IndexOf,
        KindOf,
        Located,
        Navigate,
        descend,
        location,
        NodeRef,
        VertexRead,
        VertexState,
        string_repr,
    },
};

#[cfg(test)]
mod tests;
