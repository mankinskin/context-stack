use crate::{
    AtomPosition,
    ChildLocation,
    DownKey,
    GraphRootChild,
    HasAtomPosition,
    HasGraph,
    HasRolePath,
    PathRole,
    RolePathUtils,
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

pub trait TraceRolePath<Role: PathRole>:
    RolePathUtils + HasRolePath<Role, Node = ChildLocation> + GraphRootChild<Role>
{
}
impl<
    Role: PathRole,
    P: RolePathUtils
        + HasRolePath<Role, Node = ChildLocation>
        + GraphRootChild<Role>,
> TraceRolePath<Role> for P
{
}

pub trait TraceRole<Role: PathRole> {
    fn trace_sub_path<P: TraceRolePath<Role>>(
        &mut self,
        path: &P,
        prev: RoleTraceKey<Role>,
    ) -> RoleTraceKey<Role>;
}
pub type RoleEdit<R> = NewTraceEdge<<R as PathRole>::Direction>;

impl<G: HasGraph, Role: PathRole> TraceRole<Role> for TraceCtx<G>
where
    EditKind: From<NewTraceEdge<<Role as PathRole>::Direction>>,
{
    fn trace_sub_path<P: TraceRolePath<Role>>(
        &mut self,
        path: &P,
        prev_key: RoleTraceKey<Role>,
    ) -> RoleTraceKey<Role> {
        let graph = self.trav.graph();

        path.raw_child_path()
            .iter()
            .fold(prev_key, |prev, location| {
                let target =
                    Role::Direction::build_key(&graph, *prev.pos(), location);
                self.cache
                    .add_state(RoleEdit::<Role>::new(target, prev, *location));
                target
            })
    }
}

pub trait TraceDirection {
    type Opposite: TraceDirection;
    type Key: TraceKey;
    fn build_key<G: HasGraph>(
        trav: &G,
        last_pos: AtomPosition,
        location: &ChildLocation,
    ) -> Self::Key;
}

impl TraceDirection for BottomUp {
    type Opposite = TopDown;
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
    type Opposite = BottomUp;
    type Key = DownKey;
    fn build_key<G: HasGraph>(
        trav: &G,
        last_pos: AtomPosition,
        location: &ChildLocation,
    ) -> Self::Key {
        let graph = trav.graph();
        let index = *graph.expect_child_at(location);
        let delta = graph.expect_child_offset(location);
        DownKey {
            index,
            pos: DownPosition::from(last_pos + delta),
        }
    }
}
