//! Row-model implementation (legacy).
//!
//! Contains the pattern-row graph model (`Hypergraph` with `ChildPatterns`),
//! paths, tracing caches and their tests. Frozen: no new code here, only
//! fixes needed to keep the legacy tests green. New code lives in the crate
//! root modules and must not depend on this module.
//!
//! `pub use crate::*` (and the per-module `pub use crate::<module>::*`) mirror
//! the model-agnostic root items into this namespace, so legacy code and
//! legacy consumers can import everything from `context_trace::legacy`.
//! Local items shadow the mirrored root items.

pub use crate::*;

pub mod direction;
pub mod graph;
pub mod logging;
pub mod path;
pub mod trace;

#[cfg(any(test, feature = "test-api"))]
pub mod tests;

#[cfg(any(test, feature = "test-api"))]
pub use tests::{
    assert_parents,
    env::Env1,
};

// Essential public re-exports for context-search
pub(crate) use crate::legacy::{
    direction::pattern::PatternDirection,
    graph::vertex::{
        IndexPosition,
        location::SubRangeLocation,
        pattern::pattern_range::get_child_pattern_range,
        token::SubToken,
    },
    path::{
        accessors::{
            border::PathBorder,
            child::root::PatternRootChild,
            has_path::HasRolePath,
        },
        mutators::{
            move_path::key::AdvanceKey,
            raise::PathRaise,
        },
        structs::rooted::root::PathRoot,
    },
    trace::has_graph::TravDir,
};
pub use crate::legacy::{
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
            has_vertex_key::HasVertexKey,
            key::VertexKey,
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
                pattern_range::{
                    PatternRangeIndex,
                    RangeIndex,
                },
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
    logging::{
        CompactFormat,
        format_utils::pretty,
        write_indent,
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
                HasChildPath,
                IntoRolePath,
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
                GraphRootPattern,
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
                    down::{
                        DownKey,
                        DownPosition,
                    },
                    up::{
                        UpKey,
                        UpPosition,
                    },
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

pub use self::graph::vertex::traversal::{
    BandExpandingIterator,
    BandExpandingPolicy,
    BandIterator,
    HasTokenRoleIters,
    PostfixExpandingPolicy,
    PostfixIterator,
    PrefixExpandingPolicy,
    PrefixIterator,
    boundary_target,
};

// Re-export new consolidated accessor traits
pub use self::path::accessors::{
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
