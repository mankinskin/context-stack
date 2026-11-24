//! Data types of the cover model: compositions of occurrences, their
//! locations, stored vertices and edit/validation errors.

mod composition;
mod error;
mod location;
mod vertex;

pub(crate) use self::composition::slice;
#[cfg(any(debug_assertions, test, feature = "test-api"))]
pub use self::location::IntervalView;
#[cfg(debug_assertions)]
pub use self::vertex::PositionedLabels;
pub use self::{
    composition::{
        Composition,
        CompositionError,
        Occurrence,
        maximal,
    },
    error::PositionedError,
    location::OccurrenceLocation,
    vertex::{
        PositionedVertex,
        VertexNode,
    },
};
