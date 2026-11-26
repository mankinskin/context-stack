//! Content-keyed graph interchange, independent of vertex and pattern IDs.
//!
//! The current format (v3, [`PositionedGraphSnapshot`]) stores, for every
//! retained vertex, its maximal positioned child occurrences. The row-based v2
//! format produced by the legacy pipeline lives in [`legacy`].

use serde::Deserialize;

/// The v3 interchange types live in `context_trace::positioned::interchange`;
/// re-exported here so that `ngrams::output` keeps its API.
pub use context_trace::positioned::interchange::{
    PositionedCover,
    PositionedGraphData,
    PositionedGraphSnapshot,
    POSITIONED_FORMAT_VERSION,
};

/// One named corpus accepted by CLI file mode.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// Output filename stem, validated by the CLI before generation.
    pub name: String,
    /// Texts processed together as one corpus.
    pub inputs: Vec<String>,
}

/// Row-based v2 interchange produced by the legacy pipeline.
///
/// Kept for existing row consumers and fixtures until they migrate to v3.
pub mod legacy {
    use context_trace_legacy::*;
    use serde::{
        Deserialize,
        Serialize,
    };
    use std::collections::{
        BTreeMap,
        BTreeSet,
    };

    /// Version of the legacy row-based, content-keyed graph format.
    pub const FORMAT_VERSION: u32 = 2;

    /// Versioned graph document with the input corpus retained for root checks.
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct GraphSnapshot {
        /// Must equal `FORMAT_VERSION` for this validator.
        pub format_version: u32,
        /// Original texts, each of which must have an output vertex.
        pub corpus: Vec<String>,
        /// Content-keyed graph fields flattened into this document's top level.
        #[serde(flatten)]
        pub graph: GraphData,
    }

    impl GraphSnapshot {
        /// Check the version, nonempty inputs, graph structure, and corpus-root presence.
        ///
        /// This does not establish semantic completeness or schedule independence.
        pub fn validate(&self) -> Result<(), String> {
            if self.format_version != FORMAT_VERSION {
                return Err(format!(
                    "unsupported format version: {}",
                    self.format_version
                ));
            }
            if self.corpus.is_empty() || self.corpus.iter().any(String::is_empty) {
                return Err("a corpus must contain nonempty inputs".into());
            }
            self.graph.validate()?;
            for text in &self.corpus {
                if !self.graph.vertices.contains_key(text) {
                    return Err(format!("missing corpus root: {text:?}"));
                }
            }
            Ok(())
        }
    }

    /// ID-free representation of vertices and their alternative decompositions.
    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
    pub struct GraphData {
        /// Content to distinct ordered child sequences; atoms have an empty set.
        ///
        /// Keys and alternatives are sorted, but child order within a pattern matters.
        pub vertices: BTreeMap<String, BTreeSet<Vec<String>>>,
    }

    impl GraphData {
        /// Convert a hypergraph, checking widths, content uniqueness, and reciprocal links.
        ///
        /// Duplicate decompositions are rejected before insertion could hide them.
        /// Structural lookup methods can still panic for malformed graph references.
        pub fn from_graph(graph: &Hypergraph) -> Result<Self, String> {
            let mut vertices = BTreeMap::new();
            for key in graph.vertex_keys() {
                let text = graph.vertex_key_string(&key);
                let token = graph.expect_vertex_data(key).to_token();
                if token.width().0 != text.chars().count() {
                    return Err(format!("stale width for {text:?}"));
                }
                let mut patterns = BTreeSet::new();
                for (pattern_id, pattern) in &graph.expect_child_patterns(key) {
                    let mut children = Vec::new();
                    for (offset, child) in pattern.iter().enumerate() {
                        let data = graph.expect_vertex_data(child.vertex_index());
                        if child.width() != data.width() {
                            return Err(format!("stale child width in {text:?}"));
                        }
                        let backlink = data
                            .parents()
                            .get(&token.vertex_index())
                            .ok_or_else(|| format!("missing parent link in {text:?}"))?;
                        if backlink.width() != token.width()
                            || !backlink.pattern_indices().contains(
                                &graph::vertex::parent::PatternIndex::new(*pattern_id, offset),
                            )
                        {
                            return Err(format!("invalid parent link in {text:?}"));
                        }
                        children.push(graph.index_string(*child));
                    }
                    if !patterns.insert(children) {
                        return Err(format!("duplicate decomposition of {text:?}"));
                    }
                }
                if vertices.insert(text.clone(), patterns).is_some() {
                    return Err(format!("duplicate token content: {text:?}"));
                }
                // Check the reverse direction too: every recorded parent occurrence
                // must point back to this exact child token.
                for (parent_index, parent) in graph.expect_vertex_data(key).parents() {
                    let data = graph.expect_vertex_data(parent_index);
                    if parent.width() != data.width() || parent.pattern_indices().is_empty() {
                        return Err(format!("stale parent metadata for {text:?}"));
                    }
                    for position in parent.pattern_indices() {
                        let child = data
                            .child_patterns()
                            .get(&position.pattern_id())
                            .and_then(|pattern| pattern.get(position.sub_index()));
                        if child != Some(&token) {
                            return Err(format!("stale parent occurrence for {text:?}"));
                        }
                    }
                }
            }
            let data = Self { vertices };
            data.validate()?;
            Ok(data)
        }

        /// Check atom/composite shape, exact concatenation, and strictly smaller children.
        ///
        /// Strictly decreasing character widths exclude cycles. This checks only
        /// supplied patterns, not whether the oracle omitted a meaningful alternative.
        pub fn validate(&self) -> Result<(), String> {
            for (text, patterns) in &self.vertices {
                let width = text.chars().count();
                if width == 0 || (width == 1) != patterns.is_empty() {
                    return Err(format!("invalid atom/composite {text:?}"));
                }
                for pattern in patterns {
                    if pattern.len() < 2 || pattern.concat() != *text {
                        return Err(format!("invalid decomposition of {text:?}: {pattern:?}"));
                    }
                    for child in pattern {
                        if child.chars().count() >= width || !self.vertices.contains_key(child) {
                            return Err(format!("invalid child {child:?} of {text:?}"));
                        }
                    }
                }
            }
            Ok(())
        }
    }
}
