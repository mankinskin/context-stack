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
    link::{ChainOp, OverlapLink},
};

#[derive(Default, Clone, Debug, Deref, DerefMut)]
pub(crate) struct BandChain {
    #[deref]
    #[deref_mut]
    pub(crate) bands: BTreeSet<Band>,
    /// Links representing overlaps between tokens in decompositions.
    /// Each link corresponds to an expansion that created an overlap band.
    pub(crate) links: Vec<OverlapLink>,
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
            links: Vec::new(),
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
        band.map(|band| BandCtx { band })
    }
    pub(crate) fn start_token(&self) -> Token {
        self.first().unwrap().last_token()
    }
    pub(crate) fn last(&self) -> Option<BandCtx<'_>> {
        self.bands.iter().last().map(|band| BandCtx { band })
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
    
    /// Add an overlap link representing the overlap between tokens in a decomposition.
    pub(crate) fn append_overlap_link(&mut self, link: OverlapLink) {
        debug!(
            child_path = ?link.child_path,
            search_path = ?link.search_path,
            start_bound = ?link.start_bound,
            "append_overlap_link"
        );
        self.links.push(link);
    }
    pub(crate) fn pop_first(&mut self) -> Option<Band> {
        self.bands.pop_first()
    }

    /// Get the final bundled token from the first band (main sequential bundle).
    /// The first band contains the sequential expansion result.
    /// Overlap bands (after the first) contain alternate decompositions.
    pub(crate) fn final_token(&self) -> Token {
        self.first().unwrap().last_token()
    }

    /// Iterate over overlap bands (all bands after the first one).
    /// These contain decompositions `[complement, expansion]`.
    pub(crate) fn overlap_bands(&self) -> impl Iterator<Item = &Band> {
        self.bands.iter().skip(1)
    }
}
