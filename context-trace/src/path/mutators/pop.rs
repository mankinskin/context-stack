use crate::{
    graph::vertex::location::child::ChildLocation,
    path::{
        accessors::role::PathRole,
        structs::{
            role_path::RolePath,
            rooted::{
                role_path::RootedRolePath,
                root::PathRoot,
                PathNode,
            },
        },
    },
};

// pop path segments
pub trait PathPop<Node = ChildLocation> {
    fn path_pop(&mut self) -> Option<Node>;
}

impl<Role: PathRole, Root: PathRoot> PathPop<ChildLocation> for RootedRolePath<Role, Root> {
    fn path_pop(&mut self) -> Option<ChildLocation> {
        self.role_path.path_pop()
    }
}

impl<R: PathRole> PathPop<ChildLocation> for RolePath<R> {
    fn path_pop(&mut self) -> Option<ChildLocation> {
        self.sub_path.path.pop()
    }
}
