//! Content-keyed comparison of positioned graphs for test failure output.

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    fmt,
};

use super::interchange::PositionedGraphData;

/// Occurrences `(offset, child)` of one owner present only on one side.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoverDiff {
    pub missing: Vec<(usize, String)>,
    pub extra: Vec<(usize, String)>,
}

/// Difference between an expected and an actual graph. Owners present on
/// only one side are listed as vertices, not as occurrence differences.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphDiff {
    pub missing_vertices: Vec<String>,
    pub extra_vertices: Vec<String>,
    /// Owner present on both sides → differing occurrences.
    pub covers: BTreeMap<String, CoverDiff>,
}

impl GraphDiff {
    pub fn is_empty(&self) -> bool {
        self.missing_vertices.is_empty()
            && self.extra_vertices.is_empty()
            && self.covers.is_empty()
    }
}

/// Compare `actual` against `expected` by content.
pub fn diff(
    expected: &PositionedGraphData,
    actual: &PositionedGraphData,
) -> GraphDiff {
    let only = |a: &PositionedGraphData, b: &PositionedGraphData| {
        a.vertices
            .keys()
            .filter(|key| !b.vertices.contains_key(*key))
            .cloned()
            .collect()
    };
    let covers = expected
        .vertices
        .iter()
        .filter_map(|(owner, expected_cover)| {
            let actual_cover = actual.vertices.get(owner)?;
            let e: BTreeSet<_> = expected_cover.iter().cloned().collect();
            let a: BTreeSet<_> = actual_cover.iter().cloned().collect();
            let cover = CoverDiff {
                missing: e.difference(&a).cloned().collect(),
                extra: a.difference(&e).cloned().collect(),
            };
            (cover != CoverDiff::default()).then(|| (owner.clone(), cover))
        })
        .collect();
    GraphDiff {
        missing_vertices: only(expected, actual),
        extra_vertices: only(actual, expected),
        covers,
    }
}

fn occurrences(list: &[(usize, String)]) -> String {
    list.iter()
        .map(|(offset, child)| format!("{child:?}@{offset}"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl fmt::Display for GraphDiff {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if self.is_empty() {
            return f.write_str("graphs are equal");
        }
        writeln!(f, "graph differs from expected:")?;
        let names = |list: &[String]| {
            list.iter()
                .map(|name| format!("{name:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        if !self.missing_vertices.is_empty() {
            writeln!(f, "  missing vertices: {}", names(&self.missing_vertices))?;
        }
        if !self.extra_vertices.is_empty() {
            writeln!(f, "  extra vertices: {}", names(&self.extra_vertices))?;
        }
        for (owner, cover) in &self.covers {
            write!(f, "  {owner:?}:")?;
            if !cover.missing.is_empty() {
                write!(f, " missing [{}]", occurrences(&cover.missing))?;
            }
            if !cover.extra.is_empty() {
                write!(f, " extra [{}]", occurrences(&cover.extra))?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
