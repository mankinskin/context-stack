use crate::{
    AtomPosition,
    direction::pattern::PatternDirection,
    graph::vertex::{
        location::{
            child::ChildLocation,
            pattern::IntoPatternLocation,
        },
        pattern::pattern_width,
    },
    path::{
        accessors::{
            child::root::GraphRootChild,
            root::{
                GraphRoot,
                RootPattern,
            },
        },
        mutators::{
            move_path::key::AdvanceKey,
            raise::PathRaise,
        },
        structs::{
            rooted::{
                role_path::IndexStartPath,
                root::RootedPath,
            },
            sub_path::PositionAnnotated,
        },
    },
    trace::{
        cache::key::{
            directed::{
                DirectedKey,
                up::UpKey,
            },
            props::{
                RootKey,
                TargetKey,
            },
        },
        child::state::{
            ChildState,
            RootChildState,
        },
        has_graph::{
            HasGraph,
            TravDir,
        },
        state::{
            BaseState,
            StateAdvance,
        },
    },
};
use derive_more::{
    Deref,
    DerefMut,
};
use std::{
    borrow::Borrow,
    cmp::Ordering,
    collections::VecDeque,
};

#[derive(Debug, Clone, Deref, DerefMut, Default)]
pub struct ParentBatch {
    pub parents: VecDeque<ParentState>,
}

/// State representing a position at the parent level (Start role).
/// Tracks positions for path raising and traversal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParentState {
    pub prev_pos: AtomPosition,
    pub root_pos: AtomPosition,
    pub path: IndexStartPath,
}

//impl_cursor_pos! {
//    CursorPosition for ParentState, self => self.cursor.relative_pos
//}

#[crate::instrument_trait_impl]
impl StateAdvance for ParentState {
    type Next = RootChildState;
    fn advance_state<G: HasGraph>(
        self,
        trav: &G,
    ) -> Result<Self::Next, Self> {
        let entry = self.path.graph_root_child_location();
        let graph = trav.graph();
        let pattern = self.path.root_pattern::<G>(&graph).clone();
        if let Some(next_i) =
            TravDir::<G>::pattern_index_next(pattern.borrow(), entry.sub_index)
        {
            tracing::debug!(next_i = next_i, "Found next child in pattern");
            let root_parent = self.clone();
            let ParentState {
                path,
                root_pos,
                prev_pos,
            } = self;
            Ok(RootChildState {
                child_state: ChildState {
                    entry_pos: root_pos,
                    start_pos: prev_pos, // Use prev_pos for start path tracing
                    path: path.into_range(next_i),
                },
                root_parent,
            })
        } else {
            tracing::debug!(
                entry_sub_index = entry.sub_index,
                pattern_len = pattern.len(),
                "No next child in pattern - at end of pattern"
            );
            Err(self)
        }
    }
}
impl PathRaise for ParentState {
    fn path_raise<G: HasGraph>(
        &mut self,
        trav: &G,
        parent_entry: ChildLocation,
    ) {
        // new root
        let path = &mut self.path.role_path.sub_path;

        let graph = trav.graph();
        let prev_pattern = graph.expect_pattern_at(self.path.root.location);

        self.prev_pos = self.root_pos;
        self.root_pos
            .advance_key(pattern_width(&prev_pattern[path.root_entry + 1..]).0);

        let prev = self.path.root.location.to_child_location(path.root_entry);
        path.root_entry = parent_entry.sub_index;
        self.path.root.location = parent_entry.into_pattern_location();

        // path raise is only called when path matches until end
        // avoid pointing path to the first token
        if !path.is_empty()
            || TravDir::<G>::pattern_index_prev(prev_pattern, prev.sub_index)
                .is_some()
        {
            path.path.push(prev);
        }
    }
}

impl<P: RootedPath + GraphRoot> TargetKey for BaseState<P> {
    fn target_key(&self) -> DirectedKey {
        self.root_key().into()
    }
}
impl<P: RootedPath + GraphRoot> RootKey for BaseState<P> {
    fn root_key(&self) -> UpKey {
        UpKey::new(self.path.root_parent(), self.root_pos.into())
    }
}

impl TargetKey for ParentState {
    fn target_key(&self) -> DirectedKey {
        self.root_key().into()
    }
}
impl RootKey for ParentState {
    fn root_key(&self) -> UpKey {
        UpKey::new(self.path.root_parent(), self.root_pos.into())
    }
}

impl Ord for ParentState {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.path.root_parent().cmp(&other.path.root_parent())
    }
}
impl PartialOrd for ParentState {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
