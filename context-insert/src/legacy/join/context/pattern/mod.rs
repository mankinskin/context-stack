use derivative::Derivative;

use crate::legacy::{
    join::plan::JoinRef,
    split::{
        SplitMap,
        vertex::pattern::PatternElems,
    },
};
use context_trace::legacy::*;

pub(crate) mod borders;

/// Materialized child intervals, keyed by host and half-open atom offsets.
pub(crate) type IntervalMap = HashMap<(JoinRef, usize, usize), JoinRef>;

/// Pattern join context that owns its data.
///
/// `pattern` is the node decomposition as seen through the operation-local
/// overlay (original children with proposed replacements applied).
#[derive(Debug, Clone, Derivative)]
#[derivative(Hash, PartialEq, Eq)]
pub(crate) struct PatternJoinCtx {
    pub(crate) loc: PatternLocation,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    pub(crate) pattern: Vec<JoinRef>,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    pub(crate) splits: SplitMap,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    pub(crate) intervals: IntervalMap,
}

impl PatternElems for PatternJoinCtx {
    type Elem = JoinRef;
    fn elems(&self) -> &[JoinRef] {
        &self.pattern
    }
}
impl HasPatternId for PatternJoinCtx {
    fn pattern_id(&self) -> PatternId {
        self.loc.pattern_id
    }
}

impl From<PatternJoinCtx> for PatternId {
    fn from(value: PatternJoinCtx) -> Self {
        value.loc.pattern_id
    }
}
