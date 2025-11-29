//! Core VertexData type and construction.
//!
//! Contains the primary VertexData struct definition, builder pattern,
//! and basic accessor methods for vertex properties.

use crate::{
    graph::vertex::{
        key::VertexKey,
        token::Token,
        ChildPatterns,
        VertexIndex,
        VertexParents,
    },
    TokenWidth,
};
use derive_builder::Builder;
use serde::{
    Deserialize,
    Serialize,
};

/// Central vertex data structure for hypergraph.
///
/// Contains vertex metadata, parent relationships, and child token patterns.
#[derive(Debug, Builder, Serialize, Deserialize)]
#[cfg_attr(not(any(test, feature = "test-api")), derive(PartialEq, Eq, Clone))]
#[builder(pattern = "owned")]
pub struct VertexData {
    /// Total token width of this vertex
    pub(crate) width: TokenWidth,

    /// Unique identifier for this vertex
    pub(crate) index: VertexIndex,

    /// Vertex key (optional metadata)
    #[builder(default)]
    pub(crate) key: VertexKey,

    /// Parent vertices and their pattern indices
    #[builder(default)]
    pub(crate) parents: VertexParents,

    /// Child token patterns by PatternId
    #[builder(default)]
    pub(crate) children: ChildPatterns,

    /// Cached string representation (test-only)
    #[cfg(any(test, feature = "test-api"))]
    #[serde(skip)]
    #[builder(setter(skip), default = "std::sync::RwLock::new(None)")]
    pub(crate) cached_string: std::sync::RwLock<Option<String>>,
}

// Custom PartialEq for test builds that ignores cached_string
#[cfg(any(test, feature = "test-api"))]
impl PartialEq for VertexData {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.width == other.width
            && self.index == other.index
            && self.key == other.key
            && self.parents == other.parents
            && self.children == other.children
        // cached_string is not compared
    }
}

#[cfg(any(test, feature = "test-api"))]
impl Eq for VertexData {}

// Custom Clone for test builds that resets cached_string
#[cfg(any(test, feature = "test-api"))]
impl Clone for VertexData {
    fn clone(&self) -> Self {
        Self {
            width: self.width,
            index: self.index,
            key: self.key,
            parents: self.parents.clone(),
            children: self.children.clone(),
            cached_string: std::sync::RwLock::new(None), // Don't clone cache
        }
    }
}

impl VertexData {
    /// Create a new VertexData with the given index and width
    pub fn new(
        index: VertexIndex,
        width: TokenWidth,
    ) -> Self {
        Self {
            width,
            key: VertexKey::default(),
            index,
            parents: VertexParents::default(),
            children: ChildPatterns::default(),
            #[cfg(any(test, feature = "test-api"))]
            cached_string: std::sync::RwLock::new(None),
        }
    }

    /// Get this vertex's index
    pub fn vertex_index(&self) -> VertexIndex {
        self.index
    }

    /// Get this vertex's width
    pub fn width(&self) -> TokenWidth {
        self.width
    }

    /// Convert to ChildLocation for this vertex
    pub(crate) fn to_child(&self) -> Token {
        Token::new(self.index, self.width)
    }

    /// Get immutable reference to parent relationships
    pub fn parents(&self) -> &VertexParents {
        &self.parents
    }

    /// Get mutable reference to parent relationships
    #[allow(dead_code)]
    pub(crate) fn parents_mut(&mut self) -> &mut VertexParents {
        &mut self.parents
    }

    /// Get immutable reference to child patterns
    pub fn child_patterns(&self) -> &ChildPatterns {
        &self.children
    }

    /// Get mutable reference to child patterns
    pub fn child_patterns_mut(&mut self) -> &mut ChildPatterns {
        &mut self.children
    }

    /// Invalidate cached string representation (test-only)
    #[cfg(any(test, feature = "test-api"))]
    pub(crate) fn invalidate_string_cache(&self) {
        if let Ok(mut cache) = self.cached_string.write() {
            *cache = None;
        }
    }

    /// Validate vertex invariants
    #[track_caller]
    pub(crate) fn validate(&self) {
        //self.validate_links();
        if !self.children.is_empty() {
            self.validate_patterns();
        }
    }

    /// Validate that vertex doesn't have only one parent and one child
    #[track_caller]
    #[allow(dead_code)]
    pub(crate) fn validate_links(&self) {
        assert!(self.children.len() != 1 || self.parents.len() != 1);
    }

    /// Validate all child patterns meet invariants
    #[track_caller]
    pub(crate) fn validate_patterns(&self) {
        use crate::graph::vertex::pattern::pattern_width;

        self.children.iter().fold(
            Vec::new(),
            |mut acc: Vec<Vec<usize>>, (pid, p)| {
                let mut offset = 0;
                assert!(!p.is_empty(), "Empty pattern in index {:#?}", self.index);
                let pattern_width = pattern_width(p);
                assert_eq!(pattern_width, self.width, "Pattern width mismatch in index {:#?} token pattern:\n {:#?}", self.index, (pid, self.children.get(pid)));
                let mut p = p.iter().fold(Vec::new(), |mut pa, c| {
                    offset += c.width.0;
                    assert!(
                    !acc.iter().any(|pr| pr.contains(&offset)),
                    "Duplicate border in index {:#?} token patterns:\n {:#?}",
                        self.index,
                        self.children
                    );
                    pa.push(offset);
                    pa
                });
                p.pop().unwrap();
                assert!(!p.is_empty(), "Single index pattern in index {:#?}:\n {:#?}", self.index, (pid, self.children.get(pid)));
                acc.push(p);
                acc
            },
        );
    }
}
