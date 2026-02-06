pub mod band;
pub mod expand;
pub mod link;

use std::collections::BTreeSet;

use band::Band;
use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
};
use tracing::debug;

use crate::expansion::chain::{
    band::BandCtx,
    link::ChainOp,
};

#[derive(Default, Clone, Debug, Deref, DerefMut)]
pub struct BandChain {
    #[deref]
    #[deref_mut]
    pub bands: BTreeSet<Band>,
    // todo: use map for links
    //pub links: VecDeque<OverlapLink>,
}
impl BandChain {
    pub fn new(index: Token) -> Self {
        let band = Band {
            pattern: Pattern::from(vec![index]),
            start_bound: 0.into(),
            end_bound: index.width().0.into(),
        };
        debug!(initial_band = ?band, "New BandChain");
        Self {
            bands: Some(band).into_iter().collect(),
            //links: Default::default(),
        }
    }
    pub fn ends_at(
        &self,
        bound: AtomPosition,
    ) -> Option<BandCtx<'_>> {
        let band = self.bands.get(&bound);
        debug!(
            bound = ?bound,
            found = ?band.is_some(),
            band = ?band,
            "ends_at check"
        );
        band.map(|band| BandCtx {
            band,
            //back_link: self.links.iter().last(),
            //front_link: None,
        })
    }
    pub fn last(&self) -> Option<BandCtx<'_>> {
        self.bands.iter().last().map(|band| BandCtx {
            band,
            //back_link: self.links.iter().last(),
            //front_link: None,
        })
    }
    pub fn append(
        &mut self,
        band: impl Into<Band>,
    ) {
        self.bands.insert(band.into());
    }
    pub fn append_front_complement(
        &mut self,
        complement: Token,
        exp: Token,
    ) {
        let pattern = Pattern::from(vec![complement, exp]);
        let band = Band::from((0.into(), pattern));
        debug!(
            complement = ?complement,
            expansion = ?exp,
            result_band = ?band,
            "append_front_complement"
        );
        self.append(band);
    }
    pub fn pop_first(&mut self) -> Option<Band> {
        //self.links.pop_front();
        self.bands.pop_first()
    }
}
