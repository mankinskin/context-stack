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

/// Generic role-parameterized path access trait
///
/// This trait provides role-generic access to path vectors. It's parameterized by
/// the path role (R: PathRole) as a generic parameter, allowing code to be generic
/// over Start/End roles.
///
/// **When to use:**
/// - Generic code that works with either Start or End roles
/// - Trait bounds on generic functions/implementations
/// - Types that need to implement path access for multiple roles separately
///
/// **When NOT to use:**
/// - Simple non-generic path access → Use [`PathAccessor`](super::path_accessor::PathAccessor)
/// - Need RolePath struct fields → Use [`HasRolePath`] or role-specific traits
///
/// # Design Note
/// This trait is kept alongside PathAccessor because it allows role-generic code
/// via generic parameters (HasPath<R>) rather than associated types (PathAccessor::Role = R).
/// Some types (like RootedRangePath) cannot implement PathAccessor due to having two roles,
/// but can implement HasPath<Start> and HasPath<End> separately.
#[auto_impl(& mut)]
pub trait HasPath<R> {
    type Node;
    fn path(&self) -> &Vec<Self::Node>;
    fn path_mut(&mut self) -> &mut Vec<Self::Node>;
}

/// access to a rooted path pointing to a descendant
pub trait IntoRootedRolePath<R: PathRole>:
    IntoRolePath<R> + RootedPath
{
    fn into_rooted_role_path(
        self
    ) -> RootedRolePath<R, Self::Root, ChildLocation>;
    fn get_rooted_role_path(
        &self
    ) -> RootedRolePath<R, Self::Root, ChildLocation>
    where
        Self: HasRolePath<R, Node = ChildLocation>,
    {
        let root = self.path_root();
        self.role_path().clone().into_rooted(root)
    }
}

pub trait IntoRootedPath<P: RootedPath> {
    fn into_rooted_path(self) -> P;
}
pub trait HasRootedRolePath<Root: PathRoot, R: PathRole> {
    fn rooted_role_path(&self) -> &RootedRolePath<R, Root, ChildLocation>;
    fn rooted_role_path_mut(
        &mut self
    ) -> &mut RootedRolePath<R, Root, ChildLocation>;
}
pub trait HasRootedPath<P: RootedPath> {
    fn rooted_path(&self) -> &P;
    fn rooted_path_mut(&mut self) -> &mut P;
}

/// Access to RolePath structure for role-generic code
///
/// This trait provides access to the complete RolePath structure (including root_entry field).
/// It's used for role-generic code that needs to work with both Start and End roles.
///
/// **When to use:**
/// - Role-generic implementations (generic over R: PathRole)
/// - Need access to RolePath struct fields (root_entry, etc.)
/// - Types that implement HasRolePath<Start> and HasRolePath<End> separately (like RootedRangePath)
///
/// **When NOT to use:**
/// - Simple path vector access → Use [`PathAccessor`](super::path_accessor::PathAccessor)
/// - Concrete Start role access → Use [`StartPathAccessor`](super::range_accessor::StartPathAccessor)
/// - Concrete End role access → Use [`EndPathAccessor`](super::range_accessor::EndPathAccessor)
///
/// # Design Note
/// This trait is kept (not deprecated) because RootedRangePath contains two roles (Start + End)
/// and cannot implement PathAccessor twice due to Rust's trait coherence rules (E0119).
/// HasRolePath allows role-generic code via trait bounds like `where T: HasRolePath<R>`.
pub trait HasRolePath<R: PathRole> {
    type Node;
    fn role_path(&self) -> &RolePath<R, Self::Node>;
    fn role_path_mut(&mut self) -> &mut RolePath<R, Self::Node>;
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
