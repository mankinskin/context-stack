use std::fmt;

use derive_more::{
    Deref,
    DerefMut,
};

use crate::*;

#[derive(Clone, Debug, PartialEq, Eq, Default, Deref, DerefMut)]
pub struct RolePath<R: PathRole> {
    #[deref]
    #[deref_mut]
    pub(crate) sub_path: SubPath,
    pub(crate) _ty: std::marker::PhantomData<R>,
}

impl<R: PathRole> RolePath<R> {
    pub fn new_empty(entry: usize) -> Self {
        Self {
            sub_path: SubPath::new(entry, Default::default()),
            _ty: Default::default(),
        }
    }
    pub fn new(
        entry: usize,
        path: Vec<ChildLocation>,
    ) -> Self {
        Self {
            sub_path: SubPath::new(entry, path),
            _ty: Default::default(),
        }
    }
    pub(crate) fn path(&self) -> &Vec<ChildLocation> {
        &self.sub_path.path
    }
    pub(crate) fn path_mut(&mut self) -> &mut Vec<ChildLocation> {
        &mut self.sub_path.path
    }
    pub(crate) fn into_rooted<Root: PathRoot>(
        self,
        root: Root,
    ) -> RootedRolePath<R, Root> {
        RootedRolePath::from((root, self))
    }
}

impl<R: PathRole> RootChildIndex<R> for RolePath<R> {
    fn root_child_index(&self) -> usize {
        self.sub_path.root_entry
    }
}
impl<R: PathRole> RootChildIndexMut<R> for RolePath<R> {
    fn root_child_index_mut(&mut self) -> &mut usize {
        &mut self.sub_path.root_entry
    }
}

//impl<R: PathRole> HasSinglePath for RolePath<R> {
//    fn single_path(&self) -> &[ChildLocation] {
//        self.path().borrow()
//    }
//}

impl<R: PathRole> HasPath<R> for RolePath<R> {
    fn path(&self) -> &Vec<ChildLocation> {
        &self.path
    }
    fn path_mut(&mut self) -> &mut Vec<ChildLocation> {
        &mut self.sub_path.path
    }
}

impl<R: PathRole> HasRolePath<R> for RolePath<R> {
    fn role_path(&self) -> &RolePath<R> {
        self
    }
    fn role_path_mut(&mut self) -> &mut RolePath<R> {
        self
    }
}

impl<R: PathRole> PathSimplify for RolePath<R> {
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

impl<R: PathRole> From<SubPath> for RolePath<R> {
    fn from(sub_path: SubPath) -> Self {
        Self {
            sub_path,
            _ty: Default::default(),
        }
    }
}

impl<R: PathRole> fmt::Display for RolePath<R> {
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
