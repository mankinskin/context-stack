#![deny(clippy::disallowed_methods)]
#![feature(test)]
//#![feature(hash_drain_filter)]
//#![feature(pin_macro)]
//#![feature(return_position_impl_trait_in_trait)]

extern crate test;

/// Cursor typestates and checkpoints shared with `legacy` (promoted, plan 2.0).
pub mod cursor;

/// Positioned search errors.
pub mod error;
pub use error::SearchError;

// Positioned ancestor search (plan 2.3–2.5). Module layout and names mirror
// `legacy`; only navigation (positions instead of rows) differs.
pub mod compare;
pub mod r#match;
/// Search paths (role-generic [`RolePath`], [`RangePath`]), search plan 2.2.
pub mod path;
pub mod policy;
pub mod search;
pub mod state;
/// Trace states, trace cache and cache keys, search plan 2.2.
pub mod trace;

pub use search::{
    Find,
    searchable::Searchable,
};
pub use state::response::Response;

pub use self::{
    path::{
        End,
        EndPath,
        HasRolePath,
        PathRole,
        PathRoot,
        QueryPath,
        QueryRoot,
        RangePath,
        RolePath,
        SpanCoverage,
        Start,
        StartPath,
    },
    trace::{
        QueryAnchor,
        TraceCtx,
        TraceDirection,
        cache::{
            TraceCache,
            position::PositionCache,
            vertex::VertexCache,
        },
        child::{
            ChildState,
            RootChildState,
        },
        key::{
            DirectedKey,
            DirectedPosition,
            DownKey,
            DownPosition,
            HasAtomPosition,
            UpKey,
            UpPosition,
        },
        state::{
            IntoParentState,
            StateAdvance,
            parent::ParentState,
        },
        traceable::{
            PostfixCommand,
            PrefixCommand,
            RangeCommand,
            TraceCommand,
            Traceable,
        },
    },
};

#[cfg(test)]
mod tests;

