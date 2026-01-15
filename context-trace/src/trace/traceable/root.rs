use crate::{
    trace::{
        BottomUp,
        RoleTraceKey,
        TopDown,
        cache::{
            key::directed::{
                down::DownPosition,
                up::UpPosition,
            },
            new::NewTraceEdge,
        },
    },
    *,
};

pub trait TraceRoot {
    type Next;
    fn trace_root<G: HasGraph>(
        &self,
        ctx: &mut TraceCtx<G>,
    ) -> Self::Next;
}

#[derive(Debug, Clone)]
pub(crate) struct PrefixRootCommand {
    pub(crate) root_exit: ChildLocation,
    pub(crate) exit_pos: DownPosition,
    //pub(crate) end_pos: AtomPosition,
}
#[derive(Debug, Clone)]
pub(crate) struct PostfixRootCommand {
    pub(crate) root_entry: ChildLocation,
    pub(crate) entry_pos: UpPosition,
    pub(crate) prev: UpKey,
}
#[derive(Debug, Clone)]
pub(crate) struct RangeRootCommand {
    pub(crate) postfix: PostfixRootCommand,
    pub(crate) prefix: PrefixRootCommand,
}
impl RangeRootCommand {
    pub(crate) fn postfix_root_command(&self) -> &PostfixRootCommand {
        &self.postfix
    }
    pub(crate) fn prefix_root_command(&self) -> &PrefixRootCommand {
        &self.prefix
    }
}

impl TraceRoot for RangeRootCommand {
    type Next = RoleTraceKey<End>;

    fn trace_root<G: HasGraph>(
        &self,
        ctx: &mut TraceCtx<G>,
    ) -> Self::Next {
        tracing::debug!("RangeRootCommand::trace_root - starting postfix");
        self.postfix_root_command().trace_root(ctx);
        tracing::debug!("RangeRootCommand::trace_root - starting prefix");
        let result = self.prefix_root_command().trace_root(ctx);
        tracing::debug!(
            "RangeRootCommand::trace_root - complete, result={:?}",
            result
        );
        result
    }
}
impl TraceRoot for PostfixRootCommand {
    type Next = ();

    fn trace_root<G: HasGraph>(
        &self,
        ctx: &mut TraceCtx<G>,
    ) -> Self::Next {
        let entry_key = UpKey {
            index: self.root_entry.parent,
            pos: self.entry_pos,
        };
        tracing::debug!(
            "PostfixRootCommand::trace_root - entry_key={:?}, prev={:?}, location={:?}",
            entry_key,
            self.prev,
            self.root_entry
        );
        let new = NewTraceEdge::<BottomUp> {
            target: entry_key,
            prev: self.prev,
            location: self.root_entry,
        };
        let (key, was_new) = ctx.cache.add_state(new);
        tracing::debug!(
            "PostfixRootCommand::trace_root - added to cache: key={:?}, was_new={}",
            key,
            was_new
        );
    }
}
impl TraceRoot for PrefixRootCommand {
    type Next = RoleTraceKey<End>;

    fn trace_root<G: HasGraph>(
        &self,
        ctx: &mut TraceCtx<G>,
    ) -> Self::Next {
        // key for exit position in root
        let root_exit_key = DownKey {
            index: self.root_exit.parent,
            pos: self.exit_pos,
        };
        let exit_index = ctx.trav.graph().expect_child_at(self.root_exit);
        // key for exit index in root
        let exit = DownKey {
            index: exit_index,
            pos: root_exit_key.pos,
        };
        tracing::debug!(
            "PrefixRootCommand::trace_root - root_exit_key={:?}, exit={:?}, location={:?}",
            root_exit_key,
            exit,
            self.root_exit
        );
        // edit for first trace edge
        let new = NewTraceEdge::<TopDown> {
            target: exit,
            prev: root_exit_key,
            location: self.root_exit,
        };
        let (key, was_new) = ctx.cache.add_state(new);
        tracing::debug!(
            "PrefixRootCommand::trace_root - added to cache: key={:?}, was_new={}, returning exit={:?}",
            key,
            was_new,
            exit
        );
        exit // Changed from root_exit_key to exit
    }
}
//impl TraceRoot for TraceCommand {
//    type Next = ();
//    fn trace_root<G: HasGraph>(
//        &mut self,
//        ctx: &mut TraceCtx<G>,
//    ) -> Self::Next {
//        match *self {
//            Self::Postfix(cmd) => cmd.trace_root(ctx),
//            Self::Prefix(cmd) => cmd.trace_root(ctx),
//            Self::Range(cmd) => cmd.trace_root(ctx),
//        }
//    }
//}
