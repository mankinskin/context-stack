//! Content-keyed graph interchange, independent of vertex and pattern IDs.

use context_trace::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const FORMAT_VERSION: u32 = 2;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub name: String,
    pub inputs: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSnapshot {
    pub format_version: u32,
    pub corpus: Vec<String>,
    #[serde(flatten)]
    pub graph: GraphData,
}

impl GraphSnapshot {
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct GraphData {
    pub vertices: BTreeMap<String, BTreeSet<Vec<String>>>,
}

impl GraphData {
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
