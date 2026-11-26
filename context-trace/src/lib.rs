#![deny(clippy::disallowed_methods)]


pub mod graph;
pub mod position;
/// Positioned composition model, independent of `legacy`.
pub mod positioned;

// Logging utilities (tracing and formatting)
pub mod logging;


// Hidden re-export for use in macros only (not part of public API)
#[doc(hidden)]
pub use tracing as __tracing;

// Re-export proc macros
pub use context_trace_macros::{
    TypedDebug,
    instrument_sig,
    instrument_trait_impl,
};


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


pub use crate::{
    graph::{
        kind::{
            AtomOf,
            BaseGraphKind,
            GraphKind,
        },
        vertex::{
            CommittedIndex,
            StageIndex,
            StagedIndex,
            VertexId,
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
    position::AtomPosition,
};
