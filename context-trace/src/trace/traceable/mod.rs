pub mod role;
pub mod root;
use crate::{
    trace::{
        RoleTraceKey,
        cache::key::directed::{
            down::DownPosition,
            up::UpPosition,
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
        let start_index = *ctx.trav.graph().expect_child_at(first);
        let initial_prev = UpKey {
            index: start_index,
            pos: start_index.width().0.into(),
        };
        let sub_path_prev = self.path.trace_role_sub_path(ctx, initial_prev);

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
        let start_index = *ctx.trav.graph().expect_child_at(first);
        let initial_prev = UpKey {
            index: start_index,
            pos: start_index.width().0.into(),
        };
        let sub_path_prev = self.path.trace_start_sub_path(ctx, initial_prev);

        let root_entry = self.path.role_root_child_location::<Start>();
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
        self.root_command(ctx).trace_root(ctx);
    }
}
impl Traceable for PrefixCommand {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        let prev = self.root_command(ctx).trace_root(ctx);
        self.path.trace_role_sub_path(ctx, prev);
    }
}

impl Traceable for RangeCommand {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        let exit_key =
            IntoRootCommand::<Range>::root_command(&self, ctx).trace_root(ctx);
        self.path.trace_end_sub_path(ctx, exit_key);
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
