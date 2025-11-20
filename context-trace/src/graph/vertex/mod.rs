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

pub use vertex_index::VertexIndex;

pub type VertexEntry<'x> = indexmap::map::Entry<'x, VertexKey, VertexData>;
pub type IndexedVertexEntry<'x> =
    indexmap::map::IndexedEntry<'x, VertexKey, VertexData>;
pub type VertexParents = HashMap<VertexIndex, Parent>;
pub type ChildPatterns = HashMap<PatternId, Pattern>;
pub type IndexPosition = usize;
pub type IndexPattern = Vec<VertexIndex>;
pub type VertexPatternView<'a> = Vec<&'a VertexData>;
