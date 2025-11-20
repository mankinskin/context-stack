use crate::{
    graph::vertex::location::child::ChildLocation,
    path::{
        accessors::role::PathRole,
        structs::{
            role_path::RolePath,
            rooted::{
                role_path::RootedRolePath,
                root::{
                    PathRoot,
                    RootedPath,
                },
            },
        },
    },
};
use auto_impl::auto_impl;

/// access to a rooted path pointing to a descendant
#[auto_impl(& mut)]
pub trait HasPath<R> {
    fn path(&self) -> &Vec<ChildLocation>;
    fn path_mut(&mut self) -> &mut Vec<ChildLocation>;
}

/// access to a rooted path pointing to a descendant
pub trait IntoRootedRolePath<R: PathRole>:
    IntoRolePath<R> + RootedPath
{
    fn into_rooted_role_path(self) -> RootedRolePath<R, Self::Root>;
    fn get_rooted_role_path(&self) -> RootedRolePath<R, Self::Root>
    where
        Self: HasRolePath<R>,
    {
        let root = self.path_root();
        self.role_path().clone().into_rooted(root)
    }
}

pub trait IntoRootedPath<P: RootedPath> {
    fn into_rooted_path(self) -> P;
}
pub trait HasRootedRolePath<Root: PathRoot, R: PathRole> {
    fn rooted_role_path(&self) -> &RootedRolePath<R, Root>;
    fn rooted_role_path_mut(&mut self) -> &mut RootedRolePath<R, Root>;
}
pub trait HasRootedPath<P: RootedPath> {
    fn rooted_path(&self) -> &P;
    fn rooted_path_mut(&mut self) -> &mut P;
}
/// access to a rooted path pointing to a descendant
pub trait HasRolePath<R: PathRole> {
    fn role_path(&self) -> &RolePath<R>;
    fn role_path_mut(&mut self) -> &mut RolePath<R>;
}
pub trait IntoRolePath<R: PathRole> {
    fn into_role_path(self) -> RolePath<R>;
}

//pub(crate) trait HasMatchPaths:
//    HasRolePath<Start> + HasRolePath<End>
//{
//    fn into_paths(self) -> (RolePath<Start>, RolePath<End>);
//}
//
//pub(crate) trait HasSinglePath {
//    fn single_path(&self) -> &[ChildLocation];
//}
//
