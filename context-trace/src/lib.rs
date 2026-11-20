#![deny(clippy::disallowed_methods)]
#![cfg_attr(feature = "legacy", feature(type_changing_struct_update))]

pub mod direction;

pub mod graph;
/// Trace-cache keys (token + directed atom position), shared by both models.
pub mod key;
pub mod position;
/// Positioned composition model (v3), independent of `legacy`.
pub mod positioned;

// Logging utilities (tracing and formatting)
pub mod logging;

/// Row-model implementation, fenced off from the model-agnostic root.
#[cfg(feature = "legacy")]
pub mod legacy;

// Hidden re-export for use in macros only (not part of public API)
#[doc(hidden)]
pub use tracing as __tracing;

// Re-export proc macros
pub use context_trace_macros::{
    TypedDebug,
    instrument_sig,
    instrument_trait_impl,
};

#[cfg(any(test, feature = "test-api"))]
pub mod tests;

#[cfg(not(any(test, feature = "test-api")))]
pub use std::collections::{
    HashMap,
    HashSet,
};
#[cfg(any(test, feature = "test-api"))]
pub use {
    ::charify,
    std::hash::{
        BuildHasherDefault,
        DefaultHasher,
    },
};
#[cfg(any(test, feature = "test-api"))]
pub type HashSet<T> =
    std::collections::HashSet<T, BuildHasherDefault<DefaultHasher>>;
#[cfg(any(test, feature = "test-api"))]
pub type HashMap<K, V> =
    std::collections::HashMap<K, V, BuildHasherDefault<DefaultHasher>>;

#[cfg(any(test, feature = "test-api"))]
pub use tests::test_case::TestEnv;

pub use crate::{
    direction::{
        Direction,
        Left,
        Right,
    },
    graph::{
        kind::{
            AtomOf,
            BaseGraphKind,
            GraphKind,
        },
        vertex::{
            VertexIndex,
            atom::{
                AsAtom,
                Atom,
                atomizing_iter,
            },
            has_vertex_index::{
                HasVertexIndex,
                ToToken,
            },
            has_vertex_key::HasVertexKey,
            key::VertexKey,
            token::{
                Token,
                TokenWidth,
            },
            wide::Wide,
        },
    },
    logging::{
        CompactFormat,
        format_utils::pretty,
        write_indent,
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
    position::AtomPosition,
};
