use crate::*;
use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
    From,
};
use linked_hash_set::LinkedHashSet;

#[derive(Default, Debug, Deref, DerefMut, From, Clone, PartialEq, Eq)]
pub(crate) struct Leaves(LinkedHashSet<PosKey>);

impl Leaves {
    pub(crate) fn collect_leaves(
        &mut self,
        index: &Token,
        offsets: CompleteLocations,
    ) -> HashMap<Offset, Vec<SubSplitLocation>> {
        offsets
            .into_iter()
            .filter_map(|(parent_offset, res)| match res {
                Ok(locs) => Some((parent_offset, locs)),
                Err(_) => {
                    self.insert(PosKey::new(*index, parent_offset));
                    None
                },
            })
            .collect()
    }
}
