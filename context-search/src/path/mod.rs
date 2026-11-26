//! Paths of positioned search (search plan 2.2, canonical rules 2.4).
//!
//! Positioned counterparts of the legacy role paths, generic over the role
//! and the root (legacy `RolePath<R>` / `RootedRolePath<R, Root>` /
//! `RootedRangePath<Root>`):
//!
//! - [`PathRole`] ([`Start`], [`End`], `role.rs`): storage order, span
//!   boundary, border predicate of the canonical rules, location from a
//!   root ([`Navigate::start_at`](super::Navigate::start_at) /
//!   [`Navigate::end_at`](super::Navigate::end_at)), trace
//!   direction and trace key positions;
//! - [`PathRoot`] (`root.rs`): a graph [`Token`](context_trace::Token) or a
//!   [`QueryRoot`] (token-sequence query; its next root child is the next
//!   pattern token);
//! - [`RolePath<R, Root>`] (`role_path.rs`): one path struct with the
//!   shared accessors and mutators (raise, border-child descent, advance,
//!   locate); aliases [`StartPath`] (climbs from the match start
//!   to a root), [`EndPath`] (descends from the root to the token at the
//!   match end) and [`QueryPath`] (a position inside a query);
//! - [`RangePath<Root>`] (`range.rs`): both role paths with the matched
//!   span in absolute atom offsets of the root, role-indexed through
//!   [`HasRolePath`], and [`SpanCoverage`].
//!
//! All steps are [`OccurrenceLocation`](super::OccurrenceLocation)s;
//! navigation uses only compositions, never content. Position arithmetic
//! is confined to `role.rs` (see its module docs) so that more dimensions
//! only touch the role trait and its helpers.

pub mod navigate;
pub mod range;
pub mod role;
pub mod role_path;
pub mod root;

pub use self::{
    navigate::PathNavigate,
    range::{
        HasRolePath,
        RangePath,
        SpanCoverage,
    },
    role::{
        BorderChild,
        End,
        PathRole,
        Start,
    },
    role_path::{
        EndPath,
        QueryPath,
        RolePath,
        StartPath,
    },
    root::{
        PathRoot,
        QueryRoot,
    },
};
