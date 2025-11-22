use crate::{
    AtomPosition,
    graph::vertex::wide::Wide,
    path::{
        RolePathUtils,
        accessors::role::{
            End,
            Start,
        },
        structs::rooted::{
            index_range::IndexRangePath,
            role_path::{
                IndexEndPath,
                IndexStartPath,
            },
        },
    },
    trace::{
        BottomUp,
        RoleTraceKey,
        TopDown,
        TraceCtx,
        TraceRole,
        cache::{
            key::directed::{
                down::DownKey,
                up::{
                    UpKey,
                    UpPosition,
                },
            },
            new::NewTraceEdge,
        },
        has_graph::HasGraph,
        traceable::Traceable,
    },
};
#[derive(Debug)]
pub enum TraceCommand {
    Postfix(PostfixCommand),
    Prefix(PrefixCommand),
    Range(RangeCommand),
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

#[derive(Debug)]
pub struct PostfixCommand {
    pub path: IndexStartPath,
    pub add_edges: bool,
    pub root_up_key: RoleTraceKey<Start>,
}
impl Traceable for PostfixCommand {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        tracing::debug!(
            "PostfixCommand::trace called with root_up_key.pos={}",
            self.root_up_key.pos.0
        );
        let first = self.path.role_leaf_token_location::<Start>().unwrap();
        let start_index = *ctx.trav.graph().expect_child_at(first);
        tracing::debug!(
            "PostfixCommand: first={:?}, start_index={}, start_index.width()={}",
            first,
            start_index,
            start_index.width()
        );
        tracing::trace!(
            ?first,
            ?start_index,
            "PostfixCommand: leaf and start_index"
        );
        let initial_prev = UpKey {
            index: start_index,
            pos: start_index.width().0.into(),
        };
        tracing::debug!(
            "PostfixCommand: calling trace_sub_path with initial_prev.pos={}",
            initial_prev.pos.0
        );
        let sub_path_prev = TraceRole::<Start>::trace_sub_path(
            ctx,
            &self.path,
            initial_prev,
            self.add_edges,
        );
        tracing::debug!(
            "PostfixCommand: trace_sub_path returned prev.pos={}",
            sub_path_prev.pos.0
        );
        tracing::trace!(?sub_path_prev, "PostfixCommand: trace_sub_path returned");
        let location = self.path.role_root_child_location::<Start>();
        // For cache consistency, use the root position (from root_up_key) for prev
        // The prev points to the child token at the parent's position
        let prev = UpKey {
            index: sub_path_prev.index,
            pos: self.root_up_key.pos,
        };
        tracing::debug!(
            "Creating bottom-up edge with position={}",
            self.root_up_key.pos.0
        );
        let new = NewTraceEdge::<BottomUp> {
            target: self.root_up_key,
            prev,
            location,
        };
        tracing::trace!(?new, "PostfixCommand: creating NewTraceEdge");
        ctx.cache.add_state(new, self.add_edges);
    }
}
#[derive(Debug)]
pub struct PrefixCommand {
    pub path: IndexEndPath,
    pub add_edges: bool,
    pub end_pos: AtomPosition,
}
impl Traceable for PrefixCommand {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        let root_exit = self.path.role_root_child_location::<End>();
        let exit_key = DownKey {
            pos: self.end_pos.into(),
            index: root_exit.parent,
        };
        let target = DownKey {
            index: *ctx.trav.graph().expect_child_at(root_exit),
            pos: exit_key.pos,
        };
        let new = NewTraceEdge::<TopDown> {
            target,
            prev: exit_key,
            location: root_exit,
        };
        ctx.cache.add_state(new, self.add_edges);

        TraceRole::<End>::trace_sub_path(
            ctx,
            &self.path,
            target,
            self.add_edges,
        );
    }
}

#[derive(Debug)]
pub struct RangeCommand {
    pub path: IndexRangePath,
    pub add_edges: bool,
    pub root_pos: UpPosition,
    pub end_pos: AtomPosition,
}
impl Traceable for RangeCommand {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        tracing::debug!(
            "RangeCommand::trace called with root_pos={}, end_pos={}",
            self.root_pos.0,
            usize::from(self.end_pos)
        );
        let first = self.path.role_leaf_token_location::<Start>().unwrap();
        let start_index = *ctx.trav.graph().expect_child_at(first);
        let sub_path_prev = TraceRole::<Start>::trace_sub_path(
            ctx,
            &self.path,
            UpKey {
                index: start_index,
                pos: start_index.width().0.into(),
            },
            self.add_edges,
        );
        // For cache consistency, prev should use the parent's current position (root_pos)
        // The prev points to the child token, but at the parent's position
        let root_entry = self.path.role_root_child_location::<Start>();
        let prev = UpKey {
            index: sub_path_prev.index,  // Use the child token from trace_sub_path
            pos: self.root_pos,  // But with the parent's position
        };
        let root_up_key = UpKey {
            index: root_entry.parent,
            pos: self.root_pos,
        };
        tracing::debug!(
            "Creating bottom-up edge: parent={}, pos={}",
            root_entry.parent,
            self.root_pos.0
        );
        let new = NewTraceEdge::<BottomUp> {
            target: root_up_key,
            prev,
            location: root_entry,
        };
        ctx.cache.add_state(new, self.add_edges);

        let root_exit = self.path.role_root_child_location::<End>();

        let exit_key = DownKey {
            pos: self.end_pos.into(),
            index: root_exit.parent,
        };
        let target = DownKey {
            pos: exit_key.pos,
            index: *ctx.trav.graph().expect_child_at(root_exit),
        };
        let new = NewTraceEdge::<TopDown> {
            target,
            prev: exit_key,
            location: root_exit,
        };
        ctx.cache.add_state(new, self.add_edges);

        TraceRole::<End>::trace_sub_path(
            ctx,
            &self.path,
            target,
            self.add_edges,
        );
    }
}
