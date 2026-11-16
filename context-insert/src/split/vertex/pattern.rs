use derivative::Derivative;

use context_trace::*;
use derive_new::new;

use crate::split::vertex::node::NodeTraceCtx;

/// Pattern trace context that owns its data.
///
/// With interior mutability, we can't hold references across lock boundaries,
/// so this struct owns the pattern data.
#[derive(Debug, Clone, Derivative, new)]
#[derivative(Hash, PartialEq, Eq)]
pub(crate) struct PatternTraceCtx {
    pub(crate) loc: PatternLocation,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    pub(crate) pattern: Pattern,
}

impl From<PatternTraceCtx> for PatternId {
    fn from(value: PatternTraceCtx) -> Self {
        value.loc.pattern_id
    }
}

/// Pattern context giving access to the pattern id and its elements.
///
/// Elements are graph tokens during split tracing and join references
/// during join planning.
pub(crate) trait PatternElems: HasPatternId {
    type Elem: Wide;
    fn elems(&self) -> &[Self::Elem];
}
impl PatternElems for PatternTraceCtx {
    type Elem = Token;
    fn elems(&self) -> &[Token] {
        &self.pattern
    }
}
impl HasPatternId for PatternTraceCtx {
    fn pattern_id(&self) -> PatternId {
        self.loc.pattern_id
    }
}

pub(crate) trait GetPatternTraceCtx {
    fn get_pattern_trace_context(
        &self,
        pattern_id: &PatternId,
    ) -> PatternTraceCtx;
}
pub(crate) trait GetPatternCtx {
    type PatternCtx: PatternElems;
    fn get_pattern_context(
        &self,
        pattern_id: &PatternId,
    ) -> Self::PatternCtx;
}

impl GetPatternCtx for NodeTraceCtx {
    type PatternCtx = PatternTraceCtx;
    fn get_pattern_context(
        &self,
        pattern_id: &PatternId,
    ) -> Self::PatternCtx {
        self.get_pattern_trace_context(pattern_id)
    }
}
impl GetPatternTraceCtx for NodeTraceCtx {
    fn get_pattern_trace_context(
        &self,
        pattern_id: &PatternId,
    ) -> PatternTraceCtx {
        PatternTraceCtx {
            loc: self.index.to_pattern_location(*pattern_id),
            pattern: self.patterns.get(pattern_id).unwrap().clone(),
        }
    }
}
