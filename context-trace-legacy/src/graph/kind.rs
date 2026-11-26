//! Row-model graph kind: the shared kind plus the pattern direction.
pub use context_trace::graph::kind::{AtomOf, BaseGraphKind};
use crate::direction::{Right, pattern::PatternDirection};

pub trait GraphKind: context_trace::graph::kind::GraphKind {
    type Direction: PatternDirection;
}
impl GraphKind for BaseGraphKind {
    type Direction = Right;
}
