use border::{
    PartitionBorder,
    perfect::BoolPerfect,
};
use borders::PartitionBorders;

use range::{
    mode::PatternInfoOf,
    role::{
        ModePatternCtxOf,
        RangeRole,
    },
};
use std::hash::Hash;

use crate::{
    interval::partition::{
        ToPartition,
        info::{
            border::info::InfoBorder,
            range::role::ModeNodeCtxOf,
        },
    },
    split::{
        pattern::PatternSplits,
        vertex::pattern::{
            GetPatternCtx,
            HasPatternTraceCtx,
            PatternTraceCtx,
        },
    },
};
use context_trace::*;

pub(crate) mod border;
pub(crate) mod borders;
pub(crate) mod range;

#[derive(Debug, Default)]
pub(crate) struct PartitionInfo<R: RangeRole> {
    pub(crate) patterns: HashMap<PatternId, PatternInfoOf<R>>,
    pub(crate) perfect: R::Perfect,
}

/// Type alias for pattern contexts by pattern ID.
/// With interior mutability, pattern contexts own their data.
pub(crate) type PatternCtxs<R> = HashMap<PatternId, ModePatternCtxOf<R>>;

pub(crate) trait PartitionBorderKey: Hash + Eq {}

impl<T: Hash + Eq> PartitionBorderKey for T {}
pub(crate) trait InfoPartition<R: RangeRole>: Sized + Clone + ToPartition<R> {
    fn info_borders(
        &self,
        ctx: &PatternTraceCtx,
    ) -> R::Borders {
        let part = self.clone().to_partition();
        // todo detect if prev offset is in same index (to use inner partition as result)
        let pctx = ctx.pattern_trace_context();
        let splits = part.offsets.get(&pctx.loc.pattern_id).unwrap();
        // Get atom position for recalculating sub_index from current pattern
        // The AtomPos type is generic: NonZeroUsize for Pre/Post, (NonZeroUsize, NonZeroUsize) for In
        let atom_pos = part.offsets.atom_pos();

        R::Borders::info_border(&pctx.pattern, &splits, atom_pos)
    }

    fn pattern_ctxs<'a>(
        &self,
        ctx: &ModeNodeCtxOf<'a, R>,
    ) -> PatternCtxs<R> {
        let part = self.clone().to_partition();
        part.offsets
            .ids()
            .map(|id| (*id, ctx.get_pattern_context(id)))
            .collect()
    }

    /// bundle pattern range infos of each pattern
    /// or extract complete token for range
    fn partition_borders<C: PartitionBorderKey + From<ModePatternCtxOf<R>>>(
        &self,
        ctx: &ModeNodeCtxOf<'_, R>,
    ) -> PartitionBorders<R, C> {
        let ctxs = self.pattern_ctxs(ctx);
        let (perfect, borders): (R::Perfect, HashMap<_, _>) = ctxs
            .into_values()
            .map(|pctx| {
                let (perfect, borders) = {
                    let pctx = pctx.pattern_trace_context();
                    let borders = self.info_borders(&pctx);
                    (borders.perfect().then_some(pctx.loc.pattern_id), borders)
                };
                (perfect, (C::from(pctx), borders))
            })
            .unzip();
        PartitionBorders { borders, perfect }
    }
    fn info_partition<'a>(
        &self,
        ctx: &ModeNodeCtxOf<'a, R>,
    ) -> Result<PartitionInfo<R>, Token> {
        self.partition_borders(ctx).into_partition_info()
    }
}

impl<R: RangeRole, P: ToPartition<R>> InfoPartition<R> for P {}
