//! Content-keyed interchange of the positioned model, independent of
//! vertex IDs.
//!
//! A [`PositionedGraphData`] stores, for every retained vertex, its maximal
//! positioned child occurrences. It is produced by the `ngrams` oracle
//! (re-exported as `ngrams::output`) and converted to and from
//! [`super::PositionedGraph`].
//!
//! `test-api` adds that conversion (`convert.rs`), graph comparison
//! ([`diff`]) and the reader fixtures ([`fixtures`]).

#[cfg(any(test, feature = "test-api"))]
mod convert;
#[cfg(any(test, feature = "test-api"))]
pub mod diff;
#[cfg(any(test, feature = "test-api"))]
pub mod fixtures;

use serde::{
    Deserialize,
    Deserializer,
    Serialize,
    de::{
        MapAccess,
        Visitor,
    },
};
use std::{
    collections::BTreeMap,
    fmt,
};

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
