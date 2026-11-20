//! Smoke sweep over every v3 fixture: every substring of every corpus text
//! (and each extended by every atom) finds the root and span of the
//! brute-force expectation; tie orders give identical responses.

use std::collections::BTreeSet;

use context_trace::positioned::fixtures::v3_graphs;
use pretty_assertions::assert_eq;

use super::search::check;
use crate::{
    r#match::{
        with_tie_order,
        TieOrder,
    },
    search::stats::last_search_stats,
    Find,
};

#[test]
fn every_substring_finds_expected_root_and_span() {
    let (mut queries, mut compared, mut probed, mut unique) = (0, 0, 0, 0);
    for f in v3_graphs() {
        let alphabet: Vec<String> = f
            .names
            .keys()
            .filter(|key| key.chars().count() == 1)
            .cloned()
            .collect();
        let mut texts = BTreeSet::new();
        for text in &f.corpus {
            let atoms: Vec<char> = text.chars().collect();
            for start in 0..atoms.len() {
                for end in start + 2..=atoms.len() {
                    let sub: String = atoms[start..end].iter().collect();
                    for atom in &alphabet {
                        texts.insert(format!("{sub}{atom}"));
                    }
                    texts.insert(sub);
                }
            }
        }
        for query in &texts {
            let response = check(&f, query);
            let stats = last_search_stats();
            queries += 1;
            compared += stats.compared_positions.len();
            probed += stats.equal_width_candidates;
            unique += stats.equal_width_first_matches;
            for order in [TieOrder::Ascending, TieOrder::Descending] {
                let other = with_tie_order(order, || f.graph.find_ancestor(query.chars()).unwrap());
                assert_eq!(other, response, "{}: {query} ({order:?})", f.name);
            }
        }
    }
    eprintln!(
        "sweep: {queries} queries, {compared} leaf comparisons, {probed} equal-width \
         candidates probed, {unique} of them matching at the first comparison"
    );
    assert_eq!(unique, 0);
}

/// Chained searches over pairs of corpus texts: each continuation from the
/// previous checkpoint equals a fresh search of the rest, shifted by the
/// checkpoint, and compares only positions after it (plan 2.5).
#[test]
fn chained_searches_equal_fresh_searches() {
    let mut chains = 0;
    for f in v3_graphs() {
        for (a, b) in f
            .corpus
            .iter()
            .flat_map(|a| f.corpus.iter().map(move |b| (a, b)))
        {
            let text: Vec<char> = format!("{a}{b}").chars().collect();
            let mut response = f
                .graph
                .find_ancestor(text.iter().copied().collect::<String>().chars())
                .unwrap();
            while !response.query_exhausted() {
                let offset = *response.checkpoint_position();
                let rest: String = text[offset..].iter().collect();
                let context = format!("{}: {a}{b} at {offset}", f.name);
                let fresh = f.graph.find_ancestor(rest.chars());
                let fresh_compared = last_search_stats().compared_positions;
                let next = f.graph.find_ancestor(response.query_cursor());
                let compared = last_search_stats().compared_positions;
                let (next, fresh) = match (next, fresh) {
                    (Ok(next), Ok(fresh)) => (next, fresh),
                    (next, fresh) => {
                        // A single remaining atom: both reject it.
                        assert_eq!(next, fresh, "{context}");
                        break;
                    },
                };
                assert_eq!(next.root_token(), fresh.root_token(), "{context}");
                assert_eq!(next.span(), fresh.span(), "{context}");
                assert_eq!(*next.query_start(), offset, "{context}");
                assert_eq!(
                    *next.checkpoint_position(),
                    offset + *fresh.checkpoint_position(),
                    "{context}"
                );
                assert_eq!(
                    next.path().range().map(|r| &r.path),
                    fresh.path().range().map(|r| &r.path),
                    "{context}"
                );
                assert_eq!(
                    compared.iter().map(|&p| *p - offset).collect::<Vec<_>>(),
                    fresh_compared.iter().map(|&p| *p).collect::<Vec<_>>(),
                    "{context}"
                );
                chains += 1;
                response = next;
            }
        }
    }
    eprintln!("chained: {chains} continuations");
}

/// Composite queries `[t1, t2]` of every pair of fixture tokens (Decision
/// 6 b), chained to the query end: each search finds the root, span and
/// match end of the equivalent atom query of the rest (paths too when both
/// matched beyond the first token), including continuations that start
/// inside a composite query token.
#[test]
fn composite_pairs_equal_atom_queries() {
    let (mut searches, mut inside) = (0, 0);
    for f in v3_graphs() {
        for (k1, &t1) in &f.names {
            for (k2, &t2) in &f.names {
                let text: Vec<char> = format!("{k1}{k2}").chars().collect();
                let context = format!("{}: [{k1}, {k2}]", f.name);
                let mut composite = f.graph.find_ancestor(vec![t1, t2]);
                let mut offset = 0;
                loop {
                    let rest: String = text[offset..].iter().collect();
                    let atoms = f.graph.find_ancestor(rest.chars());
                    let context = format!("{context} at {offset}");
                    let response = match (composite, atoms) {
                        (Ok(c), Ok(a)) => {
                            assert_eq!(c.root_token(), a.root_token(), "{context}");
                            assert_eq!(c.span(), a.span(), "{context}");
                            assert_eq!(*c.query_start(), offset, "{context}");
                            assert_eq!(
                                *c.checkpoint_position(),
                                offset + *a.checkpoint_position(),
                                "{context}"
                            );
                            if let (Some(cr), Some(ar)) = (c.path().range(), a.path().range()) {
                                assert_eq!(cr.path, ar.path, "{context}");
                            }
                            c
                        },
                        // The rest is one query token (atom or composite).
                        (Err(crate::SearchError::SingleIndex(t)), a) => {
                            if t.width.0 == 1 {
                                assert_eq!(a, Err(crate::SearchError::SingleIndex(t)), "{context}");
                            } else {
                                let a = a.unwrap();
                                assert_eq!(a.root_token(), t, "{context}");
                                assert!(a.query_exhausted() && a.is_entire_root(), "{context}");
                            }
                            break;
                        },
                        (c, a) => panic!("{context}: {c:?} vs {a:?}"),
                    };
                    searches += 1;
                    if response.query_exhausted() {
                        break;
                    }
                    offset = *response.checkpoint_position();
                    inside += usize::from(!response.query_cursor().path().end().is_empty());
                    composite = f.graph.find_ancestor(response.query_cursor());
                }
            }
        }
    }
    eprintln!("composite: {searches} searches, {inside} continuations inside a query token");
}
