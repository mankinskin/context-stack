#![deny(clippy::disallowed_methods)]
#![feature(test)]
#![feature(assert_matches)]
#![feature(try_blocks)]
//#![feature(hash_drain_filter)]
#![feature(slice_pattern)]
//#![feature(pin_macro)]
#![feature(exact_size_is_empty)]
#![feature(associated_type_defaults)]
//#![feature(return_position_impl_trait_in_trait)]
#![feature(type_changing_struct_update)]

pub mod direction;
pub mod path;

pub mod graph;
pub mod trace;

// Logging utilities (tracing and formatting)
pub mod logging;

// Re-export proc macros
pub use context_trace_macros::{
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
pub use tests::{
    assert_parents,
    env::{
        Env1,
        TestEnv,
    },
};

// Essential public re-exports for context-search
pub(crate) use crate::path::{
    accessors::{
        border::PathBorder,
        child::root::PatternRootChild,
        has_path::{
            HasRolePath,
            IntoRolePath,
        },
        root::GraphRootPattern,
    },
    structs::rooted::root::PathRoot,
};
pub use crate::{
    direction::{
        Direction,
        Left,
        Right,
    },
    graph::{
        Hypergraph,
        HypergraphRef,
        getters::{
            ErrorReason,
            IndexWithPath,
            vertex::VertexSet,
        },
        kind::{
            AtomOf,
            BaseGraphKind,
            GraphKind,
        },
        vertex::{
            ChildPatterns,
            VertexIndex,
            atom::{
                AsAtom,
                Atom,
                atomizing_iter,
            },
            data::VertexData,
            has_vertex_data::HasVertexData,
            has_vertex_index::{
                HasVertexIndex,
                ToToken,
            },
            location::{
                SubLocation,
                child::{
                    ChildLocation,
                    HasSubIndex,
                    HasSubIndexMut,
                },
                pattern::{
                    HasPatternLocation,
                    IntoPatternLocation,
                    PatternLocation,
                },
            },
            parent::{
                HasPatternId,
                Parent,
            },
            pattern::{
                IntoPattern,
                Pattern,
                id::PatternId,
                pattern_range::PatternRangeIndex,
                pattern_width,
            },
            token::{
                HasToken,
                Token,
                TokenWidth,
            },
            wide::Wide,
        },
    },
    path::{
        RolePathUtils,
        accessors::{
            calc::{
                CalcOffset,
                CalcWidth,
            },
            child::{
                HasLeafToken,
                HasRootedLeafToken,
                HasRootedLeafTokenLocation,
                root::GraphRootChild,
            },
            has_path::{
                HasPath,
                IntoRootedPath,
                IntoRootedRolePath,
            },
            role::{
                End,
                PathRole,
                Start,
            },
            root::{
                GraphRoot,
                PatternRoot,
                RootPattern,
            },
        },
        mutators::{
            append::PathAppend,
            lower::PathLower,
            move_path::{
                advance::{
                    Advance,
                    CanAdvance,
                },
                key::{
                    AtomPosition,
                    MoveKey,
                },
                path::MovePath,
                root::MoveRootIndex,
            },
            pop::PathPop,
            simplify::PathSimplify,
        },
        structs::{
            role_path::RolePath,
            rooted::{
                IntoChildLocation,
                PathNode,
                RootedRangePath,
                index_range::IndexRangePath,
                pattern_range::{
                    PatternPostfixPath,
                    PatternPrefixPath,
                    PatternRangePath,
                },
                role_path::{
                    HasRootChildIndex,
                    HasRootChildIndexMut,
                    HasRootChildToken,
                    IndexEndPath,
                    IndexStartPath,
                    PatternEndPath,
                    PatternStartPath,
                    RootedRolePath,
                    range::{
                        EndPath,
                        HasEndPath,
                        HasStartPath,
                        StartPath,
                    },
                },
                root::{
                    IndexRoot,
                    RootedPath,
                },
            },
            sub_path::{
                PositionAnnotated,
                SubPath,
            },
        },
    },
    trace::{
        TraceCtx,
        cache::{
            TraceCache,
            key::{
                directed::{
                    DirectedKey,
                    DirectedPosition,
                    HasAtomPosition,
                    down::DownKey,
                    up::UpKey,
                },
                props::{
                    CursorPosition,
                    LeafKey,
                    RootKey,
                    TargetKey,
                },
            },
            position::PositionCache,
            vertex::{
                VertexCache,
                positions::DirectedPositions,
            },
        },
        child::{
            iterator::{
                ChildIterator,
                ChildQueue,
                TraceKind,
            },
            state::{
                ChildState,
                RootChildState,
            },
        },
        has_graph::{
            HasGraph,
            HasGraphMut,
            TravKind,
        },
        state::{
            BaseState,
            IntoParentState,
            StateAdvance,
            parent::{
                ParentBatch,
                ParentState,
            },
        },
        traceable::{
            PostfixCommand,
            PrefixCommand,
            RangeCommand,
            Traceable,
        },
    },
};

// Re-export new consolidated accessor traits
pub use path::accessors::{
    path_accessor::{
        PathAccessor,
        RootedPathAccessor,
        StatePosition,
    },
    range_accessor::{
        EndPathAccessor,
        RangePathAccessor,
        RootedEndPathAccessor,
        RootedRolePathAccessor,
        RootedStartPathAccessor,
        StartPathAccessor,
    },
};
