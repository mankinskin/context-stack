use crate::{
    trace::cache::{
        key::directed::DirectedKey,
        new::EditKind,
    },
    *,
};
//pub(crate) enum AddTokenLocation {
//    Target(ChildLocation),
//    Prev(ChildLocation),
//}
pub type Bottom = HashMap<DirectedKey, SubLocation>;
pub type Top = HashSet<DirectedKey>;
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PositionCache {
    pub(crate) top: HashSet<DirectedKey>,
    pub(crate) bottom: Bottom,
}
impl PositionCache {
    pub fn with_top(top: Top) -> Self {
        Self {
            top,
            bottom: Default::default(),
        }
    }
    pub fn with_bottom(bottom: Bottom) -> Self {
        Self {
            top: Default::default(),
            bottom,
        }
    }
    pub fn bottom(&self) -> &Bottom {
        &self.bottom
    }
    pub fn new(
        top: HashSet<DirectedKey>,
        bottom: HashMap<DirectedKey, SubLocation>,
    ) -> Self {
        Self { top, bottom }
    }
    pub fn build_edge(
        cache: &mut TraceCache,
        state: EditKind,
        add_edges: bool,
    ) -> Self {
        // create all bottom edges (created upwards or downwards)
        let mut bottom = HashMap::default();
        match (add_edges, state) {
            (false, _) => {},
            (_, EditKind::Parent(edit)) => {
                // created by upwards traversal
                bottom
                    .insert(edit.prev.into(), edit.location.to_sub_location());
            },
            (_, EditKind::Child(edit)) => {
                // created by downwards traversal
                let prev = cache.force_mut(&(edit.prev.into()));
                prev.bottom.insert(
                    edit.target.into(),
                    edit.location.to_sub_location(),
                );
            },
        }
        Self {
            bottom,
            top: HashSet::default(),
        }
    }
    #[allow(dead_code)]
    pub(crate) fn num_parents(&self) -> usize {
        self.top.len()
    }
    #[allow(dead_code)]
    pub(crate) fn num_bu_edges(&self) -> usize {
        self.bottom.len()
    }
}
