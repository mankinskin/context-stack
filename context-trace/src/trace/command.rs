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
        role::TraceRole,
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
    pub root_up_key: RoleTraceKey<Start>,
}
impl Traceable for PostfixCommand {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        let first = self.path.role_leaf_token_location::<Start>().unwrap();
        let start_index = *ctx.trav.graph().expect_child_at(first);
        let initial_prev = UpKey {
            index: start_index,
            pos: start_index.width().0.into(),
        };
        let sub_path_prev =
            TraceRole::<Start>::trace_sub_path(ctx, &self.path, initial_prev);
        let location = self.path.role_root_child_location::<Start>();
        // For cache consistency, use the root position (from root_up_key) for prev
        // The prev points to the child token at the parent's position
        let prev = UpKey {
            index: sub_path_prev.index,
            pos: self.root_up_key.pos,
        };
        let new = NewTraceEdge::<BottomUp> {
            target: self.root_up_key,
            prev,
            location,
        };
        ctx.cache.add_state(new);
    }
}
#[derive(Debug)]
pub struct PrefixCommand {
    pub path: IndexEndPath,
    pub root_pos: AtomPosition,
    pub end_pos: AtomPosition,
}
impl Traceable for PrefixCommand {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        let root_exit = self.path.role_root_child_location::<End>();
        let exit_pos = self.end_pos;

        let exit_key = DownKey {
            pos: exit_pos.into(),
            index: root_exit.parent,
        };
        let target_index = self.path.role_rooted_leaf_token(&ctx.trav);
        let target = DownKey {
            index: target_index,
            pos: exit_key.pos,
        };
        let new = NewTraceEdge::<TopDown> {
            target,
            prev: exit_key,
            location: root_exit,
        };
        ctx.cache.add_state(new);

        TraceRole::<End>::trace_sub_path(ctx, &self.path, target);
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
        let first = self.path.role_leaf_token_location::<Start>().unwrap();
        let start_index = *ctx.trav.graph().expect_child_at(first);
        let sub_path_prev = TraceRole::<Start>::trace_sub_path(
            ctx,
            &self.path,
            UpKey {
                index: start_index,
                pos: start_index.width().0.into(),
            },
        );
        // For cache consistency, prev should use the parent's current position (root_pos)
        // The prev points to the child token, but at the parent's position
        let root_entry = self.path.role_root_child_location::<Start>();
        let prev = UpKey {
            index: sub_path_prev.index, // Use the child token from trace_sub_path
            pos: self.root_pos,         // But with the parent's position
        };
        let root_up_key = UpKey {
            index: root_entry.parent,
            pos: self.root_pos,
        };
        let new = NewTraceEdge::<BottomUp> {
            target: root_up_key,
            prev,
            location: root_entry,
        };
        ctx.cache.add_state(new);

        let root_exit = self.path.role_root_child_location::<End>();

        // Calculate exit position for top-down tracing:
        // exit_pos = entry_pos (root_pos) + atom_offset within the pattern
        // The atom_offset is the sum of widths of all children before root_exit
        let (exit_pos, target_index) = {
            let graph = ctx.trav.graph();
            let pattern = graph.expect_pattern_at(root_exit);
            let atom_offset: usize = pattern
                .iter()
                .take(root_exit.sub_index)
                .map(|token| token.width().0)
                .sum();
            let exit_pos = AtomPosition::from(self.root_pos.0 + atom_offset);
            let target_index = *graph.expect_child_at(root_exit);
            (exit_pos, target_index)
        };

        let exit_key = DownKey {
            pos: exit_pos.into(),
            index: root_exit.parent,
        };
        let target = DownKey {
            pos: exit_key.pos,
            index: target_index,
        };
        let new = NewTraceEdge::<TopDown> {
            target,
            prev: exit_key,
            location: root_exit,
        };
        ctx.cache.add_state(new);

        TraceRole::<End>::trace_sub_path(ctx, &self.path, target);
    }
}
