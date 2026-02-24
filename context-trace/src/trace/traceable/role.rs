use crate::{
    AtomPosition,
    ChildLocation,
    DownKey,
    End,
    GraphRootChild,
    HasAtomPosition,
    HasGraph,
    HasRolePath,
    PathRole,
    RolePathUtils,
    Start,
    TraceCtx,
    UpKey,
    trace::{
        BottomUp,
        RoleTraceKey,
        TopDown,
        TraceKey,
        cache::{
            key::directed::{
                down::DownPosition,
                up::UpPosition,
            },
            new::{
                EditKind,
                NewTraceEdge,
            },
        },
    },
};

pub trait RoleTraceablePath<Role: PathRole>:
    RolePathUtils + HasRolePath<Role, Node = ChildLocation> + GraphRootChild<Role>
{
}
impl<
    Role: PathRole,
    P: RolePathUtils
        + HasRolePath<Role, Node = ChildLocation>
        + GraphRootChild<Role>,
> RoleTraceablePath<Role> for P
{
}
pub type RoleEdit<R> = NewTraceEdge<<R as PathRole>::Direction>;

pub trait TracePathUtils {
    fn trace_start_sub_path<G: HasGraph>(
        &self,
        ctx: &mut TraceCtx<G>,
        prev: RoleTraceKey<Start>,
    ) -> RoleTraceKey<Start>
    where
        Self: TraceRoleSubPath<Start>,
    {
        self.trace_role_sub_path(ctx, prev)
    }

    fn trace_end_sub_path<G: HasGraph>(
        &self,
        ctx: &mut TraceCtx<G>,
        prev: RoleTraceKey<End>,
    ) -> RoleTraceKey<End>
    where
        Self: TraceRoleSubPath<End>,
    {
        self.trace_role_sub_path(ctx, prev)
    }
}
impl<T: TraceRoleSubPath<Start> + TraceRoleSubPath<End>> TracePathUtils for T {}
pub trait TraceRoleSubPath<Role: PathRole>: RoleTraceablePath<Role> {
    fn trace_role_sub_path<G: HasGraph>(
        &self,
        ctx: &mut TraceCtx<G>,
        prev: RoleTraceKey<Role>,
    ) -> RoleTraceKey<Role>;
}

impl<Role: PathRole, P: RoleTraceablePath<Role>> TraceRoleSubPath<Role> for P
where
    EditKind: From<NewTraceEdge<<Role as PathRole>::Direction>>,
{
    fn trace_role_sub_path<G: HasGraph>(
        &self,
        ctx: &mut TraceCtx<G>,
        prev_key: RoleTraceKey<Role>,
    ) -> RoleTraceKey<Role> {
        let graph = ctx.trav.graph();
        let path_len = self.raw_child_path().len();
        tracing::debug!(
            "trace_role_sub_path - starting with prev={:?}, path_len={}",
            prev_key,
            path_len
        );

        let result = self.raw_child_path()
            .iter()
            .enumerate()
            .fold(prev_key, |prev, (i, location)| {
                let target =
                    Role::Direction::build_key(&graph, *prev.pos(), location);
                tracing::debug!("trace_role_sub_path - step {}: prev={:?}, location={:?}, target={:?}",
                               i, prev, location, target);
                let (key, was_new) = ctx.cache
                    .add_state(RoleEdit::<Role>::new(prev, target, *location));
                tracing::debug!("trace_role_sub_path - step {}: added key={:?}, was_new={}",
                               i, key, was_new);
                target
            });
        tracing::debug!("trace_role_sub_path - complete, result={:?}", result);
        result
    }
}

pub trait TraceDirection {
    type Key: TraceKey;
    fn build_key<G: HasGraph>(
        trav: &G,
        last_pos: AtomPosition,
        location: &ChildLocation,
    ) -> Self::Key;
}

impl TraceDirection for BottomUp {
    type Key = UpKey;
    fn build_key<G: HasGraph>(
        _trav: &G,
        last_pos: AtomPosition,
        location: &ChildLocation,
    ) -> Self::Key {
        UpKey {
            index: location.parent,
            pos: UpPosition::from(last_pos),
        }
    }
}

impl TraceDirection for TopDown {
    type Key = DownKey;
    fn build_key<G: HasGraph>(
        trav: &G,
        last_pos: AtomPosition,
        location: &ChildLocation,
    ) -> Self::Key {
        let graph = trav.graph();
        let index = graph.expect_child_at(location);
        let delta = graph.expect_child_offset(location);
        DownKey {
            index,
            pos: DownPosition::from(last_pos + delta),
        }
    }
}
