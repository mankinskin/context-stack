//! Staging implementation (a), the physical overlay (staging plan §4): a
//! [`StagingGraph`] next to the committed graph. The base is read-only while
//! the stage is open; [`StageHandle::commit`] writes everything at once.
//!
//! - Staged vertices live in the stage, addressed by [`StagedIndex`]; atoms
//!   new in the read are staged as well (atom lookup checks the stage first).
//! - Changes of committed vertices (insertion into committed hosts) are a
//!   copy-on-write composition and a parent-link delta per vertex.
//! - [`OverlayView`] (base read guard and stage guard) is the store the
//!   read works on; index type [`StageIndex`], vertex object
//!   [`OverlayVertex`]. Compositions are read as [`Lifted`] slices in the
//!   stage's index space; unchanged committed ones in place, without
//!   copying.
//!
//! Layout: the stage itself (`staging.rs`), its compositions (`lifted.rs`),
//! vertex objects (`vertex.rs`), the store view (`view.rs`) and the read's
//! handle (`handle.rs`).
//!
//! [`StageHandle::commit`]: super::StageHandle::commit
//! [`StagedIndex`]: crate::graph::vertex::StagedIndex
//! [`StageIndex`]: crate::graph::vertex::StageIndex

mod handle;
mod lifted;
mod staging;
mod vertex;
mod view;

#[cfg(debug_assertions)]
pub use self::view::OverlayLabels;
pub use self::{
    handle::OverlayHandle,
    lifted::Lifted,
    staging::{
        StagedVertex,
        StagingGraph,
    },
    vertex::{
        OverlayVertex,
        OverlayVertexMut,
    },
    view::OverlayView,
};
