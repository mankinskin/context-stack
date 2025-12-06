//! Core VertexData type and construction.
//!
//! Contains the primary VertexData struct definition, builder pattern,
//! and basic accessor methods for vertex properties.

use crate::{
    TokenWidth,
    graph::vertex::{
        ChildPatterns,
        VertexParents,
        key::VertexKey,
        token::Token,
    },
};
use serde::{
    Deserialize,
    Serialize,
};

/// Central vertex data structure for hypergraph.
///
/// Contains vertex metadata, parent relationships, and child token patterns.
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(not(any(test, feature = "test-api")), derive(PartialEq, Eq, Clone))]
pub struct VertexData {
    /// Total token width of this vertex
    pub(crate) token: Token,

    /// Vertex key (optional metadata)
    pub(crate) key: VertexKey,

    /// Parent vertices and their pattern indices
    pub(crate) parents: VertexParents,

    /// Child token patterns by PatternId
    pub(crate) children: ChildPatterns,

    /// Cached string representation (test-only)
    #[cfg(any(test, feature = "test-api"))]
    #[serde(skip)]
    pub(crate) cached_string: std::sync::RwLock<Option<String>>,
}

// Custom PartialEq for test builds that ignores cached_string
#[cfg(any(test, feature = "test-api"))]
impl PartialEq for VertexData {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.token.width == other.token.width
            && self.token.index == other.token.index
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
            token: self.token,
            key: self.key,
            parents: self.parents.clone(),
            children: self.children.clone(),
            cached_string: std::sync::RwLock::new(None), // Don't clone cache
        }
    }
}

impl VertexData {
    /// Create a new VertexData with the given index and width
    pub fn new(token: Token) -> Self {
        Self {
            token,
            key: VertexKey::default(),
            parents: VertexParents::default(),
            children: ChildPatterns::default(),
            #[cfg(any(test, feature = "test-api"))]
            cached_string: std::sync::RwLock::new(None),
        }
    }

    /// Convert to ChildLocation for this vertex
    pub(crate) fn to_child(&self) -> Token {
        self.token
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
                assert!(
                    !p.is_empty(),
                    "Empty pattern in index {:#?}",
                    self.token
                );
                let pattern_width = pattern_width(p);
                assert_eq!(
                    pattern_width,
                    self.token.width,
                    "Pattern width mismatch in index {} token pattern:\n {:#?}",
                    self.token,
                    (pid, format!("{}", self.children.get(pid).unwrap()))
                );
                let mut p = p.iter().fold(Vec::new(), |mut pa, c| {
                    offset += c.width.0;
                    assert!(
                    !acc.iter().any(|pr| pr.contains(&offset)),
                    "Duplicate border in index {:#?} token patterns:\n {:#?}",
                        self.token,
                        self.children
                    );
                    pa.push(offset);
                    pa
                });
                p.pop().unwrap();
                assert!(
                    !p.is_empty(),
                    "Single index pattern in index {:#?}:\n {:#?}",
                    self.token,
                    (pid, self.children.get(pid))
                );
                acc.push(p);
                acc
            },
        );
    }
}

/// Builder for VertexData that defers token index assignment
#[derive(Debug, Default)]
pub struct VertexDataBuilder {
    /// Token width (to be combined with index later)
    pub(crate) width: Option<TokenWidth>,
    /// Vertex key
    pub(crate) key: Option<VertexKey>,
    /// Parent vertices
    pub(crate) parents: Option<VertexParents>,
    /// Child patterns
    pub(crate) children: Option<ChildPatterns>,
}

impl VertexDataBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the token width
    pub fn width(
        mut self,
        width: impl Into<TokenWidth>,
    ) -> Self {
        self.width = Some(width.into());
        self
    }

    /// Set the vertex key
    pub fn key(
        mut self,
        key: VertexKey,
    ) -> Self {
        self.key = Some(key);
        self
    }

    /// Set the parents
    pub fn parents(
        mut self,
        parents: VertexParents,
    ) -> Self {
        self.parents = Some(parents);
        self
    }

    /// Set the children
    pub fn children(
        mut self,
        children: ChildPatterns,
    ) -> Self {
        self.children = Some(children);
        self
    }

    /// Build VertexData with the given token (index + width)
    pub fn build(
        self,
        token: Token,
    ) -> VertexData {
        VertexData {
            token,
            key: self.key.unwrap_or_default(),
            parents: self.parents.unwrap_or_default(),
            children: self.children.unwrap_or_default(),
            #[cfg(any(test, feature = "test-api"))]
            cached_string: std::sync::RwLock::new(None),
        }
    }
}
