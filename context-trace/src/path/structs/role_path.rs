use std::fmt;

use derive_more::{
    Deref,
    DerefMut,
};

use crate::*;

#[derive(Clone, Debug, PartialEq, Eq, Deref, DerefMut)]
pub struct RolePath<R: PathRole, N = ChildLocation> {
    #[deref]
    #[deref_mut]
    pub(crate) sub_path: SubPath<N>,
    pub(crate) _ty: std::marker::PhantomData<R>,
}

impl<R: PathRole, N> Default for RolePath<R, N> {
    fn default() -> Self {
        Self {
            sub_path: SubPath::default(),
            _ty: Default::default(),
        }
    }
}

impl<R: PathRole, N> RolePath<R, N> {
    pub fn new_empty(entry: usize) -> Self {
        Self {
            sub_path: SubPath::new(entry, Vec::new()),
            _ty: Default::default(),
        }
    }
    pub fn new(
        entry: usize,
        path: Vec<N>,
    ) -> Self {
        Self {
            sub_path: SubPath::new(entry, path),
            _ty: Default::default(),
        }
    }
    pub(crate) fn path(&self) -> &Vec<N> {
        &self.sub_path.path
    }
    pub(crate) fn path_mut(&mut self) -> &mut Vec<N> {
        &mut self.sub_path.path
    }
    pub(crate) fn into_rooted<Root: PathRoot>(
        self,
        root: Root,
    ) -> RootedRolePath<R, Root, N> {
        RootedRolePath::new(root, self)
    }
}

impl<R: PathRole> RolePath<R, PositionAnnotated<ChildLocation>> {
    /// Get the entry position (position when first node was added)
    pub fn entry_position(&self) -> Option<AtomPosition> {
        self.sub_path.entry_position()
    }
}

impl<R: PathRole, N> HasRootChildIndex<R> for RolePath<R, N> {
    fn root_child_index(&self) -> usize {
        self.sub_path.root_entry
    }
}
impl<R: PathRole, N> HasRootChildIndexMut<R> for RolePath<R, N> {
    fn root_child_index_mut(&mut self) -> &mut usize {
        &mut self.sub_path.root_entry
    }
}

//impl<R: PathRole> HasSinglePath for RolePath<R> {
//    fn single_path(&self) -> &[ChildLocation] {
//        self.path().borrow()
//    }
//}

impl<R: PathRole> HasPath<R> for RolePath<R, ChildLocation> {
    type Node = ChildLocation;
    fn path(&self) -> &Vec<ChildLocation> {
        &self.sub_path.path
    }
    fn path_mut(&mut self) -> &mut Vec<ChildLocation> {
        &mut self.sub_path.path
    }
}

impl<R: PathRole> HasRolePath<R> for RolePath<R, ChildLocation> {
    type Node = ChildLocation;
    fn role_path(&self) -> &RolePath<R, ChildLocation> {
        self
    }
    fn role_path_mut(&mut self) -> &mut RolePath<R, ChildLocation> {
        self
    }
}

// New PathAccessor trait implementation
impl<R: PathRole> crate::path::accessors::path_accessor::PathAccessor
    for RolePath<R, ChildLocation>
{
    type Role = R;
    type Node = ChildLocation;

    fn path(&self) -> &Vec<ChildLocation> {
        &self.sub_path.path
    }

    fn path_mut(&mut self) -> &mut Vec<ChildLocation> {
        &mut self.sub_path.path
    }
}

impl<R: PathRole> PathSimplify for RolePath<R, ChildLocation> {
    fn into_simplified<G: HasGraph>(
        mut self,
        trav: &G,
    ) -> Self {
        let graph = trav.graph();
        while let Some(loc) = self.path_mut().pop() {
            if !<R as PathBorder>::is_at_border(graph.graph(), loc) {
                self.path_mut().push(loc);
                break;
            }
        }
        self
    }
}

impl<R: PathRole> PathSimplify
    for RolePath<R, PositionAnnotated<ChildLocation>>
{
    fn into_simplified<G: HasGraph>(
        mut self,
        trav: &G,
    ) -> Self {
        let graph = trav.graph();
        while let Some(annotated) = self.path_mut().pop() {
            if !<R as PathBorder>::is_at_border(graph.graph(), annotated.node) {
                self.path_mut().push(annotated);
                break;
            }
        }
        self
    }
}

impl<R: PathRole, N> From<SubPath<N>> for RolePath<R, N> {
    fn from(sub_path: SubPath<N>) -> Self {
        Self {
            sub_path,
            _ty: Default::default(),
        }
    }
}

impl<R: PathRole, N: fmt::Display> fmt::Display for RolePath<R, N> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(
            f,
            "entry={}, path={}",
            self.sub_path.root_entry, self.sub_path
        )
    }
}
