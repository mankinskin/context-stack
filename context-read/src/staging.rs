//! How a read stages its edits (staging plan §3.7, D-S1).

use context_trace::{
    positioned::{
        OverlayHandle,
        PositionedGraphRef,
        StageHandle,
    },
    BaseGraphKind,
};

/// Opens the stage of one read.
pub trait StagingStrategy {
    type Handle<'g>: StageHandle<Kind = BaseGraphKind>;
    fn open(graph: &PositionedGraphRef) -> Self::Handle<'_>;
}

/// (a): a physical overlay next to the read-only committed graph
/// (staging plan §4).
#[derive(Clone, Copy, Debug, Default)]
pub struct OverlayStaging;

impl StagingStrategy for OverlayStaging {
    type Handle<'g> = OverlayHandle<'g>;
    fn open(graph: &PositionedGraphRef) -> OverlayHandle<'_> {
        OverlayHandle::open(graph)
    }
}

/// The strategy of [`read`](crate::read) (D-S1).
pub type DefaultStaging = OverlayStaging;
