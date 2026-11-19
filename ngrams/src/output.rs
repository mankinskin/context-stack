//! Content-keyed graph interchange, independent of vertex and pattern IDs.
//!
//! The current format (v3, [`PositionedGraphSnapshot`]) stores, for every
//! retained vertex, its maximal positioned child occurrences. The row-based v2
//! format produced by the legacy pipeline lives in [`legacy`].

use serde::{
    de::{
        MapAccess,
        Visitor,
    },
    Deserialize,
    Deserializer,
    Serialize,
};
use std::{
    collections::BTreeMap,
    fmt,
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

/// Version of the positioned, content-keyed graph format.
pub const POSITIONED_FORMAT_VERSION: u32 = 3;

/// Positioned child occurrences `(atom offset in the owner, child content)`.
pub type PositionedCover = Vec<(usize, String)>;

/// Versioned positioned graph document with the input corpus in supplied order.
///
/// Serialized as
/// `{"format_version": 3, "corpus": [...], "vertices": {"<content>": [[<offset>, "<child>"], ...]}}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PositionedGraphSnapshot {
    /// Must equal [`POSITIONED_FORMAT_VERSION`].
    pub format_version: u32,
    /// Original texts in input order, including repeats; each must be a vertex.
    pub corpus: Vec<String>,
    /// Content-keyed covers flattened into this document's top level.
    #[serde(flatten)]
    pub graph: PositionedGraphData,
}

impl PositionedGraphSnapshot {
    /// Wrap a graph for `corpus` with the current format version.
    pub fn new(
        corpus: Vec<String>,
        graph: PositionedGraphData,
    ) -> Self {
        Self {
            format_version: POSITIONED_FORMAT_VERSION,
            corpus,
            graph,
        }
    }

    /// Check the version, corpus, and graph structure; see
    /// [`PositionedGraphData::validate_for_corpus`].
    pub fn validate(&self) -> Result<(), String> {
        if self.format_version != POSITIONED_FORMAT_VERSION {
            return Err(format!(
                "unsupported format version: {} (expected {})",
                self.format_version, POSITIONED_FORMAT_VERSION
            ));
        }
        self.graph.validate_for_corpus(&self.corpus)
    }
}

/// ID-free positioned covers of every retained vertex.
///
/// Each cover lists every maximal proper retained occurrence in the owner as
/// `(atom offset, child)`, ordered by `(offset, child width)`. Atoms (one
/// `char`) have empty covers. Occurrences are a `Vec` so that invalid
/// duplicates or ordering remain visible to [`Self::validate`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct PositionedGraphData {
    /// Owner content to its positioned cover; duplicate keys are rejected.
    #[serde(deserialize_with = "unique_keys")]
    pub vertices: BTreeMap<String, PositionedCover>,
}

impl PositionedGraphData {
    /// Structural validation plus a nonempty corpus whose texts are all vertices.
    pub fn validate_for_corpus(
        &self,
        corpus: &[String],
    ) -> Result<(), String> {
        if corpus.is_empty() {
            return Err("corpus must contain at least one text".into());
        }
        if let Some(index) = corpus.iter().position(String::is_empty) {
            return Err(format!("corpus text {index} is empty"));
        }
        self.validate()?;
        for (index, text) in corpus.iter().enumerate() {
            if !self.vertices.contains_key(text) {
                return Err(format!("missing corpus root {index}: {text:?}"));
            }
        }
        Ok(())
    }

    /// Check every cover's structure, independent of how it was produced.
    ///
    /// Requires nonempty content keys; empty covers exactly for atoms; known,
    /// strictly narrower (in chars) children; in-range offsets (checked without
    /// overflow) whose span in the owner equals the child; strictly increasing
    /// `(offset, child width)` order; no occurrence contained in another of the
    /// same owner (including equal spans); and coverage of every owner position.
    ///
    /// This is structural only. It cannot detect an omitted maximal occurrence
    /// when the remaining entries still cover the owner (e.g. `ababa` as
    /// `[aba@0, b@3, a@4]` while `aba@2` exists); semantic verification against
    /// the retained set or an independent enumeration is a separate step.
    pub fn validate(&self) -> Result<(), String> {
        for (owner, cover) in &self.vertices {
            Self::validate_cover(&self.vertices, owner, cover)?;
        }
        Ok(())
    }

    fn validate_cover(
        vertices: &BTreeMap<String, PositionedCover>,
        owner: &str,
        cover: &[(usize, String)],
    ) -> Result<(), String> {
        let chars: Vec<char> = owner.chars().collect();
        let width = chars.len();
        if width == 0 {
            return Err("empty vertex content".into());
        }
        if width == 1 {
            return match cover.first() {
                None => Ok(()),
                Some((offset, child)) => Err(format!(
                    "atom {owner:?} has entry {child:?}@{offset}"
                )),
            };
        }
        if cover.is_empty() {
            return Err(format!("composite {owner:?} has an empty cover"));
        }
        let mut covered = vec![false; width];
        let mut previous: Option<(usize, usize, &str)> = None;
        for (index, (offset, child)) in cover.iter().enumerate() {
            let offset = *offset;
            let entry = || format!("{owner:?} entry {index} ({child:?}@{offset})");
            if !vertices.contains_key(child) {
                return Err(format!("{}: unknown child", entry()));
            }
            let child_width = child.chars().count();
            if child_width >= width {
                return Err(format!("{}: child is not narrower than owner", entry()));
            }
            let end = offset
                .checked_add(child_width)
                .filter(|&end| end <= width)
                .ok_or_else(|| format!("{}: span out of range", entry()))?;
            if !chars[offset..end].iter().copied().eq(child.chars()) {
                return Err(format!("{}: span does not match owner content", entry()));
            }
            if let Some((p_offset, p_end, p_child)) = previous {
                let p_width = p_end - p_offset;
                if (p_offset, p_width) == (offset, child_width) {
                    return Err(format!("{}: duplicate entry", entry()));
                }
                if (p_offset, p_width) > (offset, child_width) {
                    return Err(format!(
                        "{}: not ordered after {p_child:?}@{p_offset}",
                        entry()
                    ));
                }
                // Sorted by start; a later span is contained iff it ends no
                // later, and an earlier one iff both share the start.
                if end <= p_end || offset == p_offset {
                    return Err(format!(
                        "{}: contained occurrence overlapping {p_child:?}@{p_offset}",
                        entry()
                    ));
                }
            }
            covered[offset..end].iter_mut().for_each(|c| *c = true);
            previous = Some((offset, end, child));
        }
        match covered.iter().position(|c| !c) {
            Some(position) => Err(format!(
                "{owner:?}: position {position} is not covered"
            )),
            None => Ok(()),
        }
    }
}

/// Decode a content map, rejecting duplicate keys instead of overwriting.
fn unique_keys<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, PositionedCover>, D::Error>
where
    D: Deserializer<'de>,
{
    struct UniqueKeys;
    impl<'de> Visitor<'de> for UniqueKeys {
        type Value = BTreeMap<String, PositionedCover>;

        fn expecting(
            &self,
            f: &mut fmt::Formatter,
        ) -> fmt::Result {
            f.write_str("a map from vertex content to positioned covers")
        }

        fn visit_map<A: MapAccess<'de>>(
            self,
            mut map: A,
        ) -> Result<Self::Value, A::Error> {
            let mut vertices = BTreeMap::new();
            while let Some((key, cover)) =
                map.next_entry::<String, PositionedCover>()?
            {
                if vertices.contains_key(&key) {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate vertex {key:?}"
                    )));
                }
                vertices.insert(key, cover);
            }
            Ok(vertices)
        }
    }
    deserializer.deserialize_map(UniqueKeys)
}

/// Row-based v2 interchange produced by the legacy pipeline.
///
/// Kept for existing row consumers and fixtures until they migrate to v3.
pub mod legacy {
    use context_trace::legacy::*;
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
