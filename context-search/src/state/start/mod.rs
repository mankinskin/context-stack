mod core;
mod search;

pub(crate) use core::{
    InputLocation,
    IntoCursor,
    StartCtx,
    StartFoldPath,
};
pub use search::Searchable;
