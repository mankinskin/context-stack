//! Positions of one direction (legacy `cache::vertex::positions`).

use derive_more::derive::{
    Deref,
    DerefMut,
    IntoIterator,
};

use crate::{
    AtomPosition,
    HashMap,
    positioned::trace::cache::position::PositionCache,
};

#[derive(Clone, Debug, PartialEq, Eq, Default, IntoIterator, Deref, DerefMut)]
pub struct DirectedPositions {
    entries: HashMap<AtomPosition, PositionCache>,
}

impl FromIterator<(AtomPosition, PositionCache)> for DirectedPositions {
    fn from_iter<T: IntoIterator<Item = (AtomPosition, PositionCache)>>(iter: T) -> Self {
        Self {
            entries: FromIterator::from_iter(iter),
        }
    }
}

impl Extend<(AtomPosition, PositionCache)> for DirectedPositions {
    fn extend<T: IntoIterator<Item = (AtomPosition, PositionCache)>>(
        &mut self,
        iter: T,
    ) {
        for (k, v) in iter {
            let entry = self.entries.entry(k).or_default();
            entry.top.extend(v.top);
            entry.bottom.extend(v.bottom);
        }
    }
}
