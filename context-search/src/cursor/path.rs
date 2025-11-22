use std::ops::ControlFlow;

use crate::{
    cursor::{
        CursorState,
        PathCursor,
    },
    state::start::StartFoldPath,
};
use context_trace::{
    IntoChildLocation,
    *,
};

pub(crate) trait MovablePath<D: Direction, R: PathRole>:
    MovePath<D, R> + RootChildIndex<R> + RootPattern
{
}
impl<
        D: Direction,
        R: PathRole,
        P: MovePath<D, R> + RootChildIndex<R> + RootPattern,
    > MovablePath<D, R> for P
{
}
impl<D: Direction, R: PathRole, P: MovablePath<D, R>, S: CursorState>
    MovePath<D, R> for PathCursor<P, S>
where
    Self: MoveKey<D>,
    P: MovePath<D, R>,
{
    type Node = P::Node;

    fn path_pop_node(&mut self) -> Option<Self::Node> {
        self.path.path_pop_node()
    }

    fn move_path_segment<G: HasGraph>(
        &mut self,
        node: &mut Self::Node,
        trav: &G::Guard<'_>,
    ) -> ControlFlow<()> {
        let flow = self.path.move_path_segment::<G>(node, trav);
        if let ControlFlow::Continue(()) = flow {
            let graph = trav.graph();
            let location = node.as_child_location();
            self.move_key(*graph.expect_child_at(location).width());
        }
        flow
    }
}

impl<D: Direction, R: PathRole, P: MovablePath<D, R>, S: CursorState>
    MoveRootIndex<D, R> for PathCursor<P, S>
where
    Self: MoveKey<D> + RootChildIndex<R>,
{
    fn move_root_index<G: HasGraph>(
        &mut self,
        trav: &G,
    ) -> ControlFlow<()> {
        let flow = self.path.move_root_index(trav);
        if let ControlFlow::Continue(()) = flow {
            let child_index = self.role_root_child_index();
            let graph = trav.graph();
            let pattern = self.path.root_pattern::<G>(&graph);

            // Only access pattern if child_index is valid
            // When matching is complete, child_index may equal pattern.len()
            if child_index < pattern.len() {
                let child_width = pattern[child_index].width();
                let old_pos = *self.atom_position.as_ref();
                tracing::debug!(
                    "PathCursor::move_root_index: child_index={}, child_width={}, old_atom_pos={}, will advance to {}",
                    child_index, child_width, old_pos, old_pos + child_width
                );
                self.move_key(*child_width);
            }
        }
        flow
    }
}

impl<P: PathPop, S: CursorState> PathPop for PathCursor<P, S> {
    fn path_pop(&mut self) -> Option<ChildLocation> {
        self.path.path_pop()
    }
}
impl<P: PathAppend, S: CursorState> PathAppend for PathCursor<P, S> {
    fn path_append(
        &mut self,
        parent_entry: ChildLocation,
    ) {
        self.path.path_append(parent_entry);
    }
}
impl<P: RootedPath, S: CursorState> HasRootedPath<P> for PathCursor<P, S> {
    fn rooted_path(&self) -> &P {
        &self.path
    }
    fn rooted_path_mut(&mut self) -> &mut P {
        &mut self.path
    }
}
impl<R: PathRole, P: StartFoldPath + HasPath<R>, S: CursorState> HasPath<R>
    for PathCursor<P, S>
{
    type Node = <P as HasPath<R>>::Node;
    fn path(&self) -> &Vec<Self::Node> {
        HasPath::<R>::path(&self.path)
    }
    fn path_mut(&mut self) -> &mut Vec<Self::Node> {
        HasPath::<R>::path_mut(&mut self.path)
    }
}

impl<R: PathRole, P: RootChildToken<R> + StartFoldPath, S: CursorState>
    RootChildToken<R> for PathCursor<P, S>
{
    fn root_child_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token {
        RootChildToken::<R>::root_child_token(&self.path, trav)
    }
}
impl<R: PathRole, P: StartFoldPath + LeafToken<R>, S: CursorState> LeafToken<R>
    for PathCursor<P, S>
{
    fn leaf_token_location(&self) -> Option<ChildLocation> {
        LeafToken::<R>::leaf_token_location(&self.path)
    }
    fn leaf_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Option<Token> {
        LeafToken::<R>::leaf_token(&self.path, trav)
    }
}
