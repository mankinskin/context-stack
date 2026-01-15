use parent::Parent;
use pattern::Pattern;

use crate::{
    HashMap,
    graph::vertex::{
        data::VertexData,
        key::VertexKey,
        pattern::id::PatternId,
    },
};

pub mod atom;
pub mod data;
pub mod entry;
pub mod has_vertex_data;
pub mod has_vertex_index;
pub mod has_vertex_key;
pub mod key;
pub mod location;
pub mod parent;
pub mod pattern;
pub mod token;
pub mod vertex_index;
pub mod wide;

pub use entry::VertexEntry;
pub use vertex_index::VertexIndex;

/// Type alias for indexmap's entry API (for vertex map operations)
pub type VertexMapEntry<'x> = indexmap::map::Entry<'x, VertexKey, VertexEntry>;
pub type IndexedVertexMapEntry<'x> =
    indexmap::map::IndexedEntry<'x, VertexKey, VertexEntry>;
pub type VertexParents = HashMap<VertexIndex, Parent>;
pub type ChildPatterns = HashMap<PatternId, Pattern>;
pub type IndexPosition = usize;
pub type IndexPattern = Vec<VertexIndex>;
pub type VertexPatternView<'a> = Vec<&'a VertexData>;
