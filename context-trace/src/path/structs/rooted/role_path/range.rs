use crate::*;

pub type StartPath = RolePath<Start>;
pub type EndPath = RolePath<End>;

//impl LeafTokenPosMut<End> for EndPath {
//    fn leaf_token_pos_mut(&mut self) -> &mut usize {
//        if !self.path().is_empty() {
//            &mut self.leaf_token_location_mut().unwrap().sub_index
//        } else {
//            self.root_child_index_mut()
//        }
//    }
//}
use crate::path::accessors::has_path::HasPath;
use crate::path::accessors::role::{Start, End};

pub trait HasStartPath: HasPath<Start> {
    fn start_path(&self) -> &StartPath;
    fn start_path_mut(&mut self) -> &mut StartPath;
}

pub trait HasEndPath: HasPath<End> {
    fn end_path(&self) -> &EndPath;
    fn end_path_mut(&mut self) -> &mut EndPath;
}

impl From<IndexRangePath> for StartPath {
    fn from(p: IndexRangePath) -> Self {
        p.start
    }
}

impl From<IndexRangePath> for EndPath {
    fn from(p: IndexRangePath) -> Self {
        p.end
    }
}
//impl<R> WideMut for RolePath<R> {
//    fn width_mut(&mut self) -> &mut usize {
//        &mut self.width
//    }
//}
