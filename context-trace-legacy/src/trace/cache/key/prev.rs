use crate::trace::cache::directed::{DirectedKey};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrevKey {
    pub(crate) prev_target: DirectedKey,
    pub(crate) delta: usize,
}

impl PrevKey {
    pub(crate) fn advanced(&self) -> DirectedKey {
        let mut target = self.prev_target.clone();
        target.pos += self.delta;
        target
    }
}

pub(crate) trait ToPrev {
    fn to_prev(
        self,
        delta: usize,
    ) -> PrevKey;
}

impl<T: Into<DirectedKey>> ToPrev for T {
    fn to_prev(
        self,
        delta: usize,
    ) -> PrevKey {
        PrevKey {
            prev_target: self.into(),
            delta,
        }
    }
}
