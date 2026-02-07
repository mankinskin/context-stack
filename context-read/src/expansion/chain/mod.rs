pub(crate) mod band;
pub(crate) mod expand;
pub(crate) mod link;

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
pub(crate) struct BandChain {
    #[deref]
    #[deref_mut]
    pub(crate) bands: BTreeSet<Band>,
    // todo: use map for links
    //pub(crate) links: VecDeque<OverlapLink>,
}
impl BandChain {
    pub(crate) fn new(index: Token) -> Self {
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
    pub(crate) fn ends_at(
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
    pub(crate) fn start_token(&self) -> Token {
        self.first().unwrap().last_token()
    }
    pub(crate) fn last(&self) -> Option<BandCtx<'_>> {
        self.bands.iter().last().map(|band| BandCtx {
            band,
            //back_link: self.links.iter().last(),
            //front_link: None,
        })
    }
    pub(crate) fn append(
        &mut self,
        band: impl Into<Band>,
    ) {
        self.bands.insert(band.into());
    }
    pub(crate) fn append_front_complement(
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
    pub(crate) fn pop_first(&mut self) -> Option<Band> {
        //self.links.pop_front();
        self.bands.pop_first()
    }

    /// Get the final bundled token from the last band.
    pub(crate) fn final_token(&self) -> Token {
        self.last().unwrap().band.last_token()
    }

    /// Iterate over overlap bands (all bands after the first one).
    /// These contain decompositions `[complement, expansion]`.
    pub(crate) fn overlap_bands(&self) -> impl Iterator<Item = &Band> {
        self.bands.iter().skip(1)
    }
}
