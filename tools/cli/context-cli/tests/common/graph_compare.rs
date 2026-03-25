//! Label-indexed graph comparison utilities for ngrams oracle validation.
//!
//! The core problem: the ngrams and context-read algorithms both produce
//! hypergraphs for the same input string, but assign vertex indices in
//! different orders.  Comparison must be label-based, not index-based.
//!
//! # Comparison Model
//!
//! Both graphs are normalised into a [`LabelMap`] — a `HashMap<label,
//! TokenEntry>` where each entry records the token's width and its set of
//! child patterns.  Patterns are stored as `BTreeSet<Vec<String>>` so the
//! *set* of patterns is unordered (a token may decompose in multiple ways),
//! but the *sequence within* each pattern is preserved (order matters).
//!
//! See `20260315_PLAN_NGRAMS_ORACLE_VALIDATION.md` §Comparison Strategy for
//! the full rationale (Design Decisions D17–D20).

use std::{
    collections::{
        BTreeSet,
        HashMap,
        HashSet,
    },
    fmt,
};

use context_trace::graph::snapshot::GraphSnapshot;

// ---------------------------------------------------------------------------
// Core data structures
// ---------------------------------------------------------------------------

/// Canonical, label-indexed view of a hypergraph for comparison purposes.
///
/// Built from a [`GraphSnapshot`] via [`label_map_from_snapshot`].
#[derive(Debug, Clone, Default)]
pub struct LabelMap {
    /// `label → width + set of child patterns`.
    /// Atoms are also represented here (width == 1, patterns empty).
    pub tokens: HashMap<String, TokenEntry>,
    /// Labels of atom vertices (width == 1, no children).
    pub atoms: HashSet<String>,
}

/// Per-token data stored in a [`LabelMap`].
#[derive(Debug, Clone)]
pub struct TokenEntry {
    /// Character width of the token (1 for atoms, >1 for compound tokens).
    pub width: usize,
    /// Set of child patterns.  Each inner `Vec<String>` is one pattern
    /// (an ordered sequence of child labels).  The outer `BTreeSet` is
    /// unordered — a token may have multiple valid decompositions.
    pub patterns: BTreeSet<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Building a LabelMap from a GraphSnapshot
// ---------------------------------------------------------------------------

/// Build a [`LabelMap`] from a [`GraphSnapshot`].
///
/// Steps:
/// 1. Build `index → label` map from `SnapshotNode` list.
/// 2. Populate every node as a `TokenEntry` (patterns initially empty).
/// 3. Group edges by `(parent_index, pattern_idx)`, sort children by
///    `sub_index`, resolve indices to labels, and insert the resulting
///    `Vec<String>` into the parent's pattern set.
pub fn label_map_from_snapshot(snap: &GraphSnapshot) -> LabelMap {
    // Step 1: index → label
    let index_to_label: HashMap<usize, String> = snap
        .nodes
        .iter()
        .map(|n| (n.index, n.label.clone()))
        .collect();

    // Step 2: seed every node
    let mut tokens: HashMap<String, TokenEntry> = HashMap::new();
    let mut atoms: HashSet<String> = HashSet::new();

    for node in &snap.nodes {
        if node.width == 1 {
            atoms.insert(node.label.clone());
        }
        tokens
            .entry(node.label.clone())
            .or_insert_with(|| TokenEntry {
                width: node.width,
                patterns: BTreeSet::new(),
            });
    }

    // Step 3: group edges → patterns.
    // Key: (parent_index, pattern_idx)   Value: [(sub_index, child_index)]
    let mut by_parent_pattern: HashMap<(usize, usize), Vec<(usize, usize)>> =
        HashMap::new();

    for edge in &snap.edges {
        by_parent_pattern
            .entry((edge.from, edge.pattern_idx))
            .or_default()
            .push((edge.sub_index, edge.to));
    }

    for ((parent_idx, _pattern_idx), mut children) in by_parent_pattern {
        let parent_label = match index_to_label.get(&parent_idx) {
            Some(l) => l.clone(),
            None => continue,
        };

        // Sort by sub_index so children are in left-to-right order.
        children.sort_by_key(|(sub_idx, _)| *sub_idx);

        let pattern: Vec<String> = children
            .iter()
            .filter_map(|(_, child_idx)| index_to_label.get(child_idx))
            .cloned()
            .collect();

        if pattern.is_empty() {
            continue;
        }

        if let Some(entry) = tokens.get_mut(&parent_label) {
            entry.patterns.insert(pattern);
        }
    }

    LabelMap { tokens, atoms }
}

// ---------------------------------------------------------------------------
// Comparison report types
// ---------------------------------------------------------------------------

/// A width disagreement between oracle and candidate for the same label.
#[derive(Debug, Clone)]
pub struct WidthMismatch {
    pub label: String,
    pub oracle_width: usize,
    pub candidate_width: usize,
}

/// A child pattern whose concatenation does not equal the parent label.
///
/// This is always a bug in the candidate graph (or snapshot extraction).
#[derive(Debug, Clone)]
pub struct ConcatenationViolation {
    pub label: String,
    pub pattern: Vec<String>,
    /// What the children actually concatenate to (should equal `label`).
    pub actual_concat: String,
}

/// A candidate pattern for a label that has no matching pattern in the oracle.
///
/// This is a *warning*, not an error.  Context-read may legitimately produce
/// a decomposition that the ngrams frequency/wrapper passes did not create.
#[derive(Debug, Clone)]
pub struct PatternMismatch {
    pub label: String,
    pub candidate_pattern: Vec<String>,
}

/// Result of comparing a candidate (context-read) graph against an ngrams
/// oracle graph.
///
/// ## Severity levels
///
/// | Severity | Condition |
/// |----------|-----------|
/// | **Fatal** | `root_present_in_oracle == false`, any `width_mismatches`, any `concatenation_violations` |
/// | **Error** | `atom_match == false` |
/// | **Warning** | entries in `pattern_mismatches` |
/// | **Info** | entries in `unverified_by_oracle` |
///
/// Use [`report_is_ok`] to gate test pass/fail on Fatal + Error findings.
#[derive(Debug, Clone, Default)]
pub struct ComparisonReport {
    /// The root label (full input string) must be present in the oracle graph.
    pub root_present_in_oracle: bool,
    /// `candidate.atoms == oracle.atoms` (same set of distinct characters).
    pub atom_match: bool,
    /// Tokens present in both maps with different widths — always a bug.
    pub width_mismatches: Vec<WidthMismatch>,
    /// Patterns whose child-label concatenation ≠ parent label — always a bug.
    pub concatenation_violations: Vec<ConcatenationViolation>,
    /// Candidate labels absent from the oracle (not necessarily wrong).
    pub unverified_by_oracle: Vec<String>,
    /// Candidate patterns with no corresponding pattern in the oracle.
    pub pattern_mismatches: Vec<PatternMismatch>,
}

// ---------------------------------------------------------------------------
// Comparison logic
// ---------------------------------------------------------------------------

/// Returns `true` if the report has no Fatal or Error findings.
///
/// Fatal: `root_present_in_oracle == false`, any `width_mismatches`,
///        any `concatenation_violations`.
/// Error: `atom_match == false`.
///
/// Warnings (`pattern_mismatches`) and Info (`unverified_by_oracle`) do **not**
/// cause this function to return `false`.
pub fn report_is_ok(report: &ComparisonReport) -> bool {
    report.root_present_in_oracle
        && report.atom_match
        && report.width_mismatches.is_empty()
        && report.concatenation_violations.is_empty()
}

/// Compare a candidate (context-read) graph against an ngrams oracle graph.
///
/// The `root_label` should be the full input string — the token that is
/// expected to be the root in both graphs.
///
/// # Checks performed
///
/// 1. **Root present in oracle** (Fatal) — `root_label` must exist in oracle.
/// 2. **Atom equality** (Error) — `candidate.atoms == oracle.atoms`.
/// 3. For every non-atom token in the candidate:
///    a. **Width agreement** (Fatal) — if the label also exists in the oracle,
///       both widths must agree.
///    b. **Concatenation invariant** (Fatal) — for every child pattern,
///       concatenation of child labels must equal the parent label.
///    c. **Oracle presence** (Info) — labels absent from oracle are recorded
///       in `unverified_by_oracle` but do not fail the report.
///    d. **Pattern subsumption** (Warning) — for each candidate pattern,
///       check whether that exact pattern exists in the oracle.  If not,
///       it's logged in `pattern_mismatches`.
pub fn compare_against_oracle(
    oracle: &LabelMap,
    candidate: &LabelMap,
    root_label: &str,
) -> ComparisonReport {
    let mut report = ComparisonReport::default();

    // 1. Root present in oracle
    report.root_present_in_oracle = oracle.tokens.contains_key(root_label);

    // 2. Atom equality
    report.atom_match = candidate.atoms == oracle.atoms;

    // 3. Walk every non-atom token in the candidate
    let mut unverified: Vec<String> = Vec::new();
    let mut width_mismatches: Vec<WidthMismatch> = Vec::new();
    let mut concat_violations: Vec<ConcatenationViolation> = Vec::new();
    let mut pattern_mismatches: Vec<PatternMismatch> = Vec::new();

    for (label, entry) in &candidate.tokens {
        // Skip atoms — they are checked in bulk via atom_match above.
        if candidate.atoms.contains(label) {
            continue;
        }

        let oracle_entry = oracle.tokens.get(label);

        // 3a. Width agreement
        if let Some(oe) = oracle_entry {
            if oe.width != entry.width {
                width_mismatches.push(WidthMismatch {
                    label: label.clone(),
                    oracle_width: oe.width,
                    candidate_width: entry.width,
                });
            }
        } else {
            // 3c. Label absent from oracle
            unverified.push(label.clone());
        }

        // 3b + 3d. Per-pattern checks
        for pattern in &entry.patterns {
            // Concatenation invariant
            let actual_concat: String =
                pattern.iter().map(|s| s.as_str()).collect();
            if actual_concat != *label {
                concat_violations.push(ConcatenationViolation {
                    label: label.clone(),
                    pattern: pattern.clone(),
                    actual_concat,
                });
            }

            // Pattern subsumption (warning only)
            if let Some(oe) = oracle_entry {
                if !oe.patterns.contains(pattern) {
                    pattern_mismatches.push(PatternMismatch {
                        label: label.clone(),
                        candidate_pattern: pattern.clone(),
                    });
                }
            }
        }
    }

    // Sort all lists for deterministic output in test failure messages.
    unverified.sort();
    width_mismatches.sort_by(|a, b| a.label.cmp(&b.label));
    concat_violations.sort_by(|a, b| a.label.cmp(&b.label));
    pattern_mismatches.sort_by(|a, b| a.label.cmp(&b.label));

    report.unverified_by_oracle = unverified;
    report.width_mismatches = width_mismatches;
    report.concatenation_violations = concat_violations;
    report.pattern_mismatches = pattern_mismatches;

    report
}

// ---------------------------------------------------------------------------
// Human-readable display for test failure messages
// ---------------------------------------------------------------------------

impl fmt::Display for ComparisonReport {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        writeln!(f, "=== Oracle Comparison Report ===")?;

        // Fatal findings
        if !self.root_present_in_oracle {
            writeln!(f, "[FATAL] Root label NOT present in oracle graph")?;
        }
        if !self.atom_match {
            writeln!(f, "[ERROR] Atom sets do not match")?;
        }
        for wm in &self.width_mismatches {
            writeln!(
                f,
                "[FATAL] Width mismatch for {:?}: oracle={} candidate={}",
                wm.label, wm.oracle_width, wm.candidate_width
            )?;
        }
        for cv in &self.concatenation_violations {
            writeln!(
                f,
                "[FATAL] Concatenation violation for {:?}: pattern={} concat_to={:?}",
                cv.label,
                format_pattern(&cv.pattern),
                cv.actual_concat
            )?;
        }

        // Warnings
        for pm in &self.pattern_mismatches {
            writeln!(
                f,
                "[WARN]  Pattern not in oracle for {:?}: {}",
                pm.label,
                format_pattern(&pm.candidate_pattern)
            )?;
        }

        // Info
        if !self.unverified_by_oracle.is_empty() {
            writeln!(
                f,
                "[INFO]  Unverified by oracle (not necessarily wrong): {:?}",
                self.unverified_by_oracle
            )?;
        }

        if report_is_ok(self) {
            writeln!(f, "Result: OK (no fatal or error findings)")?;
        } else {
            writeln!(f, "Result: FAILED")?;
        }

        Ok(())
    }
}

fn format_pattern(pattern: &[String]) -> String {
    let inner = pattern
        .iter()
        .map(|s| format!("{:?}", s))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{}]", inner)
}

// ---------------------------------------------------------------------------
// Unit tests for the comparison helpers themselves
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use context_trace::graph::snapshot::{
        SnapshotEdge,
        SnapshotNode,
    };

    /// Build a minimal snapshot for testing.
    fn make_snap(
        nodes: Vec<(usize, &str, usize)>,
        edges: Vec<(usize, usize, usize, usize)>,
    ) -> GraphSnapshot {
        GraphSnapshot {
            nodes: nodes
                .into_iter()
                .map(|(index, label, width)| SnapshotNode {
                    index,
                    label: label.to_string(),
                    width,
                })
                .collect(),
            edges: edges
                .into_iter()
                .map(|(from, to, pattern_idx, sub_index)| SnapshotEdge {
                    from,
                    to,
                    pattern_idx,
                    sub_index,
                })
                .collect(),
        }
    }

    #[test]
    fn label_map_atoms_only() {
        // "ab": two atoms, no compound token
        let snap = make_snap(vec![(0, "a", 1), (1, "b", 1)], vec![]);
        let map = label_map_from_snapshot(&snap);

        assert_eq!(map.atoms.len(), 2);
        assert!(map.atoms.contains("a"));
        assert!(map.atoms.contains("b"));
        assert!(map.tokens["a"].patterns.is_empty());
    }

    #[test]
    fn label_map_compound_token() {
        // "ab" as root with children "a" and "b"
        // index 2 = "ab", index 0 = "a", index 1 = "b"
        let snap = make_snap(
            vec![(0, "a", 1), (1, "b", 1), (2, "ab", 2)],
            vec![
                (2, 0, 0, 0), // ab → a, pattern 0, pos 0
                (2, 1, 0, 1), // ab → b, pattern 0, pos 1
            ],
        );
        let map = label_map_from_snapshot(&snap);

        assert_eq!(map.atoms.len(), 2);
        let ab = &map.tokens["ab"];
        assert_eq!(ab.width, 2);
        assert_eq!(ab.patterns.len(), 1);
        let pattern = ab.patterns.iter().next().unwrap();
        assert_eq!(pattern, &vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn label_map_multiple_patterns() {
        // "abc" with two patterns: [ab, c] and [a, bc]
        let snap = make_snap(
            vec![
                (0, "a", 1),
                (1, "b", 1),
                (2, "c", 1),
                (3, "ab", 2),
                (4, "bc", 2),
                (5, "abc", 3),
            ],
            vec![
                // pattern 0: abc → [ab, c]
                (5, 3, 0, 0),
                (5, 2, 0, 1),
                // pattern 1: abc → [a, bc]
                (5, 0, 1, 0),
                (5, 4, 1, 1),
            ],
        );
        let map = label_map_from_snapshot(&snap);

        let abc = &map.tokens["abc"];
        assert_eq!(abc.width, 3);
        assert_eq!(abc.patterns.len(), 2);
        assert!(
            abc.patterns
                .contains(&vec!["ab".to_string(), "c".to_string()])
        );
        assert!(
            abc.patterns
                .contains(&vec!["a".to_string(), "bc".to_string()])
        );
    }

    #[test]
    fn compare_identical_maps_is_ok() {
        let snap = make_snap(
            vec![(0, "a", 1), (1, "b", 1), (2, "ab", 2)],
            vec![(2, 0, 0, 0), (2, 1, 0, 1)],
        );
        let map = label_map_from_snapshot(&snap);
        let report = compare_against_oracle(&map, &map, "ab");
        assert!(report_is_ok(&report), "{}", report);
    }

    #[test]
    fn compare_root_missing_from_oracle() {
        let oracle_snap = make_snap(vec![(0, "a", 1), (1, "b", 1)], vec![]);
        let candidate_snap = make_snap(
            vec![(0, "a", 1), (1, "b", 1), (2, "ab", 2)],
            vec![(2, 0, 0, 0), (2, 1, 0, 1)],
        );
        let oracle = label_map_from_snapshot(&oracle_snap);
        let candidate = label_map_from_snapshot(&candidate_snap);
        let report = compare_against_oracle(&oracle, &candidate, "ab");

        assert!(!report.root_present_in_oracle);
        assert!(!report_is_ok(&report));
    }

    #[test]
    fn compare_concatenation_violation() {
        // "ab" token with a broken pattern [b, a] (concat = "ba" ≠ "ab")
        let oracle_snap = make_snap(
            vec![(0, "a", 1), (1, "b", 1), (2, "ab", 2)],
            vec![(2, 0, 0, 0), (2, 1, 0, 1)],
        );
        let bad_snap = make_snap(
            vec![(0, "a", 1), (1, "b", 1), (2, "ab", 2)],
            vec![
                (2, 1, 0, 0), // b first
                (2, 0, 0, 1), // a second → concat = "ba" ≠ "ab"
            ],
        );
        let oracle = label_map_from_snapshot(&oracle_snap);
        let bad = label_map_from_snapshot(&bad_snap);
        let report = compare_against_oracle(&oracle, &bad, "ab");

        assert!(!report.concatenation_violations.is_empty());
        assert!(!report_is_ok(&report));
    }

    #[test]
    fn compare_width_mismatch() {
        let oracle_snap = make_snap(
            vec![(0, "a", 1), (1, "b", 1), (2, "ab", 2)],
            vec![(2, 0, 0, 0), (2, 1, 0, 1)],
        );
        // Same label "ab" but wrong width (3 instead of 2)
        let bad_snap = make_snap(
            vec![(0, "a", 1), (1, "b", 1), (2, "ab", 3)],
            vec![(2, 0, 0, 0), (2, 1, 0, 1)],
        );
        let oracle = label_map_from_snapshot(&oracle_snap);
        let bad = label_map_from_snapshot(&bad_snap);
        let report = compare_against_oracle(&oracle, &bad, "ab");

        assert!(!report.width_mismatches.is_empty());
        assert!(!report_is_ok(&report));
    }

    #[test]
    fn compare_unverified_label_is_only_info() {
        // Candidate has "ab" and "abc"; oracle only has "ab".
        // "abc" is unverified but should not fail the report.
        let oracle_snap = make_snap(
            vec![(0, "a", 1), (1, "b", 1), (2, "c", 1), (3, "ab", 2)],
            vec![(3, 0, 0, 0), (3, 1, 0, 1)],
        );
        let candidate_snap = make_snap(
            vec![
                (0, "a", 1),
                (1, "b", 1),
                (2, "c", 1),
                (3, "ab", 2),
                (4, "abc", 3),
            ],
            vec![(3, 0, 0, 0), (3, 1, 0, 1), (4, 3, 0, 0), (4, 2, 0, 1)],
        );
        let oracle = label_map_from_snapshot(&oracle_snap);
        let candidate = label_map_from_snapshot(&candidate_snap);
        let report = compare_against_oracle(&oracle, &candidate, "abc");

        // root "abc" is absent from oracle → fatal
        assert!(!report.root_present_in_oracle);
        // "abc" should appear in unverified
        assert!(report.unverified_by_oracle.contains(&"abc".to_string()));
    }

    #[test]
    fn report_display_smoke_test() {
        // Just ensure display doesn't panic.
        let report = ComparisonReport::default();
        let s = format!("{}", report);
        assert!(s.contains("Report"));
    }
}
