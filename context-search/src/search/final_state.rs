use std::cmp::Ordering;

use crate::state::end::EndState;
use context_trace::{
    trace::cache::vertex::VertexCache,
    *,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct FoldState {
    pub(crate) cache: TraceCache,
    pub(crate) end_state: EndState,
    pub(crate) start: Token,
    pub(crate) root: Token,
}

impl FoldState {
    pub(crate) fn root_entry(&self) -> &VertexCache {
        self.cache.entries.get(&self.root().vertex_index()).unwrap()
    }
    pub(crate) fn start_key(&self) -> DirectedKey {
        DirectedKey::new(self.start, self.start.width())
    }
    pub(crate) fn root(&self) -> Token {
        self.root
    }
}

// get bottom up edge iterators
//  - use back edges for late path directly
//  - trace back edges for early path to gather bottom up edges
//    - build new cache for this or store forward edges directly in search
// edge: token location, position

// tabularize all splits bottom up
// table: location, position -> split
// breadth first bottom up traversal , merging splits
// - start walking edges up from leaf nodes
// - each edge has location in parent and position
//    - each edge defines a split in parent at location, possibly merged with nested splits from below path
//    - each node has a bottom edge n-tuple for each of its token patterns, where n is the number of splits

// - combine splits into an n+1-tuple of pieces for each split tuple and position
//    - each position needs a single n+1-tuple of pieces, built with respect to other positions
// - combine split context and all positions into pairs of halves for each position

// - continue walk up to parents, write split pieces to table for each position
//    - use table to pass finished splits upwards

// - at root, there are at least 2 splits for each token pattern and only one position
