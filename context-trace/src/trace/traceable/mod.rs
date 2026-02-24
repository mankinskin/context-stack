pub mod role;
pub mod root;
use crate::{
    trace::{
        BottomUp,
        cache::{
            key::directed::{
                down::DownPosition,
                up::UpPosition,
            },
            new::NewTraceEdge,
        },
        traceable::{
            role::{
                TracePathUtils,
                TraceRoleSubPath,
            },
            root::{
                PostfixRootCommand,
                PrefixRootCommand,
                RangeRootCommand,
                TraceRoot,
            },
        },
    },
    *,
};
#[derive(Debug)]
pub enum TraceCommand {
    Postfix(PostfixCommand),
    Prefix(PrefixCommand),
    Range(RangeCommand),
}

#[derive(Debug, Clone)]
pub struct PostfixCommand {
    pub(crate) path: IndexStartPath,
    //pub root_up_key: RoleTraceKey<Start>,
    pub(crate) entry_pos: UpPosition,
}

impl PostfixCommand {
    pub fn new(
        path: IndexStartPath,
        _root_parent: ChildLocation,
        entry_pos: UpPosition,
    ) -> Self {
        Self { path, entry_pos }
    }
}
#[derive(Debug, Clone)]
pub struct PrefixCommand {
    pub(crate) path: IndexEndPath,
    pub(crate) exit_pos: DownPosition,
    //pub(crate) end_pos: AtomPosition,
}

impl PrefixCommand {
    pub fn new(
        path: IndexEndPath,
        exit_pos: DownPosition,
        //end_pos: AtomPosition,
    ) -> Self {
        Self {
            path,
            exit_pos,
            //end_pos,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RangeCommand {
    pub(crate) path: IndexRangePath,
    pub(crate) entry_pos: UpPosition,
    pub(crate) exit_pos: DownPosition,
    //pub(crate) end_pos: AtomPosition,
}

impl RangeCommand {
    pub fn new(
        path: IndexRangePath,
        entry_pos: UpPosition,
        exit_pos: DownPosition,
    ) -> Self {
        Self {
            path,
            entry_pos,
            exit_pos,
        }
    }
}

pub struct Range;

pub(crate) trait IntoRootCommand<Role> {
    type RootCommand: TraceRoot;
    fn root_command<G: HasGraph>(
        &self,
        ctx: &mut TraceCtx<G>,
    ) -> Self::RootCommand;
}
impl IntoRootCommand<Start> for PrefixCommand {
    type RootCommand = PrefixRootCommand;
    fn root_command<G: HasGraph>(
        &self,
        _ctx: &mut TraceCtx<G>,
    ) -> Self::RootCommand {
        let root_exit = self.path.role_root_child_location::<End>();
        PrefixRootCommand {
            root_exit,
            exit_pos: self.exit_pos,
        }
    }
}
impl IntoRootCommand<End> for PostfixCommand {
    type RootCommand = PostfixRootCommand;
    fn root_command<G: HasGraph>(
        &self,
        ctx: &mut TraceCtx<G>,
    ) -> Self::RootCommand {
        let first = self.path.role_leaf_token_location::<Start>().unwrap();
        let start_index = ctx.trav.graph().expect_child_at(first);
        let initial_prev = UpKey {
            index: start_index,
            pos: start_index.width().0.into(),
        };
        let mut sub_path_prev =
            self.path.trace_role_sub_path(ctx, initial_prev);
        // Update position to match entry_pos after tracing sub-path
        sub_path_prev.pos = self.entry_pos;

        let root_entry = self.path.role_root_child_location::<Start>();
        PostfixRootCommand {
            root_entry,
            prev: sub_path_prev,
            entry_pos: self.entry_pos,
        }
    }
}
impl IntoRootCommand<Range> for RangeCommand {
    type RootCommand = RangeRootCommand;
    fn root_command<G: HasGraph>(
        &self,
        ctx: &mut TraceCtx<G>,
    ) -> Self::RootCommand {
        let postfix = IntoRootCommand::<Start>::root_command(self, ctx);
        let prefix = IntoRootCommand::<End>::root_command(self, ctx);
        RangeRootCommand { postfix, prefix }
    }
}
impl IntoRootCommand<Start> for RangeCommand {
    type RootCommand = PostfixRootCommand;
    fn root_command<G: HasGraph>(
        &self,
        ctx: &mut TraceCtx<G>,
    ) -> Self::RootCommand {
        let first = self.path.role_leaf_token_location::<Start>().unwrap();
        let start_index = ctx.trav.graph().expect_child_at(first);
        let initial_prev = UpKey {
            index: start_index,
            pos: start_index.width().0.into(),
        };
        let mut sub_path_prev =
            self.path.trace_start_sub_path(ctx, initial_prev);

        let root_entry = self.path.role_root_child_location::<Start>();

        // Trace through all intermediate parents from sub_path_prev to root_entry
        // This ensures all vertices on the bottom-up path are added to the cache
        let mut current = sub_path_prev.index;
        let target_root = root_entry.parent;

        tracing::debug!(
            "Tracing intermediate parents: current={:?}, target_root={:?}, entry_pos={:?}",
            current,
            target_root,
            self.entry_pos
        );

        // Traverse upward from current through all parents until we reach target_root
        while current != target_root {
            let graph = ctx.trav.graph();
            let parents = graph.expect_parents(current);

            // Find the parent that is on the path to target_root
            let next_parent: Option<(VertexIndex, Token, &Parent)> =
                parents.iter().find_map(|(parent_idx, parent_data)| {
                    let parent_token = graph.to_token(*parent_idx);

                    // Check if this parent is target_root
                    if parent_token == target_root {
                        return Some((*parent_idx, parent_token, parent_data));
                    }

                    // Check if target_root is a parent of this parent
                    if graph
                        .expect_parents(parent_token)
                        .contains_key(&target_root.vertex_index())
                    {
                        return Some((*parent_idx, parent_token, parent_data));
                    }

                    None
                });

            if let Some((_parent_idx, parent_token, parent_data)) = next_parent
            {
                // If the next parent is the target root, don't add it here
                // Let PostfixRootCommand handle the final connection
                if parent_token == target_root {
                    // Just update sub_path_prev to point to the last intermediate parent
                    // at the current position, then break
                    break;
                }

                // Find the correct pattern index for this parent relationship
                let pattern_idx = parent_data
                    .pattern_indices()
                    .iter()
                    .find(|_pi| {
                        // For intermediate parents, find any pattern that leads upward
                        true
                    })
                    .expect("Should find pattern index");

                let location = ChildLocation {
                    parent: parent_token,
                    pattern_id: pattern_idx.pattern_id,
                    sub_index: pattern_idx.sub_index,
                };

                let target = UpKey {
                    index: parent_token,
                    pos: sub_path_prev.pos, // Use the current position, not entry_pos
                };

                tracing::debug!(
                    "Adding intermediate parent: prev={:?}, target={:?}, location={:?}",
                    sub_path_prev,
                    target,
                    location
                );

                let new_edge = NewTraceEdge::<BottomUp> {
                    target,
                    prev: sub_path_prev,
                    location,
                };
                ctx.cache.add_state(new_edge);

                current = parent_token;
                sub_path_prev = target;
            } else {
                tracing::error!(
                    "Could not find next parent from {:?} to {:?}",
                    current,
                    target_root
                );
                break;
            }
        }

        // Now update the position to entry_pos for the final connection to root
        sub_path_prev.pos = self.entry_pos;

        PostfixRootCommand {
            root_entry,
            prev: sub_path_prev,
            entry_pos: self.entry_pos,
        }
    }
}
impl IntoRootCommand<End> for RangeCommand {
    type RootCommand = PrefixRootCommand;
    fn root_command<G: HasGraph>(
        &self,
        _ctx: &mut TraceCtx<G>,
    ) -> Self::RootCommand {
        let root_exit = self.path.role_root_child_location::<End>();
        PrefixRootCommand {
            root_exit,
            exit_pos: self.exit_pos,
        }
    }
}

pub trait Traceable {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    );
}
impl Traceable for PostfixCommand {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        tracing::debug!(
            "PostfixCommand::trace - entry_pos={:?}",
            self.entry_pos
        );
        self.root_command(ctx).trace_root(ctx);
        tracing::debug!("PostfixCommand::trace - complete");
    }
}
impl Traceable for PrefixCommand {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        tracing::debug!("PrefixCommand::trace - exit_pos={:?}", self.exit_pos);
        let prev = self.root_command(ctx).trace_root(ctx);
        tracing::debug!("PrefixCommand::trace - after root, prev={:?}", prev);
        self.path.trace_role_sub_path(ctx, prev);
        tracing::debug!("PrefixCommand::trace - complete");
    }
}

impl Traceable for RangeCommand {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        tracing::debug!(
            "RangeCommand::trace - entry_pos={:?}, exit_pos={:?}",
            self.entry_pos,
            self.exit_pos
        );
        let exit_key =
            IntoRootCommand::<Range>::root_command(&self, ctx).trace_root(ctx);
        tracing::debug!(
            "RangeCommand::trace - after root, exit_key={:?}",
            exit_key
        );
        self.path.trace_end_sub_path(ctx, exit_key);
        tracing::debug!("RangeCommand::trace - complete");
    }
}

impl Traceable for TraceCommand {
    fn trace<G: super::has_graph::HasGraph>(
        self,
        ctx: &mut super::TraceCtx<G>,
    ) {
        match self {
            Self::Postfix(cmd) => cmd.trace(ctx),
            Self::Prefix(cmd) => cmd.trace(ctx),
            Self::Range(cmd) => cmd.trace(ctx),
        }
    }
}
