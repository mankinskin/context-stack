//! Plan 2.6: exhaustive validation of positioned search on every v3
//! fixture. Queries and expectations come only from the fixture strings
//! (corpus texts and keys of the name map); every response is checked by
//! [`validate`].

use std::collections::BTreeSet;

use context_trace::{
    positioned::fixtures::{v3_graphs, V3Graph},
    Token,
};
use pretty_assertions::assert_eq;

use super::{
    util::chars,
    validate::{cache_edges, validate, Oracle},
};
use crate::{
    r#match::{with_tie_order, TieOrder},
    search::stats::last_search_stats,
    Find, SearchError,
};

/// Deterministic SplitMix64 (no extra dependency).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const RANDOM_QUERIES: usize = 300;

fn alphabet(f: &V3Graph) -> Vec<char> {
    f.names
        .keys()
        .filter(|key| key.chars().count() == 1)
        .map(|key| key.chars().next().unwrap())
        .collect()
}

fn substrings(text: &[char]) -> impl Iterator<Item = &[char]> {
    (0..text.len())
        .flat_map(move |start| (start + 1..=text.len()).map(move |end| &text[start..end]))
}

/// Query sets of 2.6: every substring of every corpus text, each extended
/// by every alphabet atom, and seeded random strings over the alphabet.
fn atom_queries(f: &V3Graph) -> [(&'static str, Vec<String>); 3] {
    let alphabet = alphabet(f);
    let mut subs = BTreeSet::new();
    let mut extended = BTreeSet::new();
    for text in &f.corpus {
        for sub in substrings(&chars(text)) {
            let sub: String = sub.iter().collect();
            for atom in &alphabet {
                extended.insert(format!("{sub}{atom}"));
            }
            subs.insert(sub);
        }
    }
    let max = f.corpus.iter().map(|t| t.chars().count()).max().unwrap() + 2;
    let mut rng = Rng(0x2_6000 ^ f.name.len() as u64);
    let random = (0..RANDOM_QUERIES)
        .map(|_| {
            let len = 2 + rng.below(max - 1);
            (0..len)
                .map(|_| alphabet[rng.below(alphabet.len())])
                .collect()
        })
        .collect();
    [
        ("substrings", subs.into_iter().collect()),
        ("extended", extended.into_iter().collect()),
        ("random", random),
    ]
}

/// Every atom query of 2.6 matches the brute-force expectation; ascending
/// and descending tie orders give identical responses (paths and trace
/// caches included); the uniqueness counter is 0 on every fixture.
#[test]
fn atom_queries_match_brute_force() {
    let mut counts = [0usize; 3];
    let (mut probed, mut fixtures) = (0, 0);
    for f in v3_graphs() {
        let oracle = Oracle::new(&f);
        let mut unique = 0;
        for (i, (category, queries)) in atom_queries(&f).into_iter().enumerate() {
            for query in &queries {
                let context = format!("{} ({category}): {query}", f.name);
                let result = f.graph.find_ancestor(query.chars());
                let stats = last_search_stats();
                counts[i] += 1;
                if query.chars().count() == 1 {
                    assert_eq!(
                        result,
                        Err(SearchError::SingleIndex(f.names[query])),
                        "{context}"
                    );
                    continue;
                }
                let response = result.unwrap_or_else(|error| panic!("{context}: {error}"));
                validate(&oracle, query, 0, &response);
                probed += stats.equal_width_candidates;
                unique += stats.equal_width_first_matches;
                for order in [TieOrder::Ascending, TieOrder::Descending] {
                    let other = with_tie_order(order, || f.graph.find_ancestor(query.chars()));
                    assert_eq!(other.as_ref(), Ok(&response), "{context} ({order:?})");
                }
            }
        }
        assert_eq!(unique, 0, "{}: uniqueness counter", f.name);
        fixtures += 1;
    }
    eprintln!(
        "atom queries on {fixtures} fixtures: {} substrings, {} extended, {} random; \
         {probed} equal-width candidates probed, none matching at the first comparison",
        counts[0], counts[1], counts[2]
    );
}

/// Longest fixture key at the start of `text`.
fn longest_key<'k>(f: &'k V3Graph, text: &[char]) -> (&'k String, Token) {
    f.names
        .iter()
        .filter(|(key, _)| text.starts_with(&chars(key)))
        .max_by_key(|(key, _)| key.chars().count())
        .map(|(key, &token)| (key, token))
        .unwrap()
}

/// `text` split greedily into the longest fixture keys.
fn greedy_split(f: &V3Graph, mut text: &[char]) -> Vec<Token> {
    let mut tokens = vec![];
    while !text.is_empty() {
        let (key, token) = longest_key(f, text);
        tokens.push(token);
        text = &text[key.chars().count()..];
    }
    tokens
}

fn atoms(f: &V3Graph, text: &[char]) -> Vec<Token> {
    text.iter().map(|c| f.names[&c.to_string()]).collect()
}

/// Composite query sequences of `f` with their strings: each corpus text
/// split into covering fixture tokens (one chosen key at every position,
/// greedy longest keys around it), mixed atom/token sequences (one
/// composite key at every position of every corpus substring, atoms
/// around it), both extended by every alphabet atom (mismatches after or
/// inside composite tokens), and every pair of fixture keys.
fn composite_queries(f: &V3Graph) -> [(&'static str, BTreeSet<(String, Vec<Token>)>); 4] {
    let mut splits = BTreeSet::new();
    let mut mixed = BTreeSet::new();
    let keys: Vec<(Vec<char>, Token)> = f.names.iter().map(|(k, &t)| (chars(k), t)).collect();
    for text in &f.corpus {
        let text = chars(text);
        for i in 0..text.len() {
            for (key, token) in keys.iter().filter(|(key, _)| text[i..].starts_with(key)) {
                let mut query = greedy_split(f, &text[..i]);
                query.push(*token);
                query.extend(greedy_split(f, &text[i + key.len()..]));
                splits.insert((text.iter().collect(), query));
            }
        }
        for sub in substrings(&text) {
            for i in 0..sub.len() {
                for (key, token) in keys
                    .iter()
                    .filter(|(key, _)| key.len() > 1 && sub[i..].starts_with(key))
                {
                    let mut query = atoms(f, &sub[..i]);
                    query.push(*token);
                    query.extend(atoms(f, &sub[i + key.len()..]));
                    mixed.insert((sub.iter().collect(), query));
                }
            }
        }
    }
    let mut extended = BTreeSet::new();
    for (text, query) in splits.iter().chain(&mixed) {
        for atom in alphabet(f) {
            let mut query = query.clone();
            query.push(f.names[&atom.to_string()]);
            extended.insert((format!("{text}{atom}"), query));
        }
    }
    let pairs = keys
        .iter()
        .flat_map(|(k1, t1)| {
            keys.iter()
                .map(move |(k2, t2)| (k1.iter().chain(k2).collect(), vec![*t1, *t2]))
        })
        .collect();
    [
        ("splits", splits),
        ("mixed", mixed),
        ("extended", extended),
        ("pairs", pairs),
    ]
}

/// Composite queries (Decision 6 b) give the same root, span, coverage,
/// paths and trace-cache edges as the equivalent atom query, chained to the
/// query end (continuations may start inside a composite query token).
/// Includes an unmatched composite first token (2.6 finding: it used to be
/// `EntireRoot` without paths).
#[test]
fn composite_queries_equal_atom_queries() {
    let mut counts = [0usize; 4];
    let (mut searches, mut inside, mut unmatched_first) = (0, 0, 0);
    for f in v3_graphs() {
        let oracle = Oracle::new(&f);
        for (i, (category, queries)) in composite_queries(&f).into_iter().enumerate() {
            for (text, query) in queries {
                counts[i] += 1;
                let text = chars(&text);
                let context = format!("{} ({category}): {query:?}", f.name);
                let mut composite = f.graph.find_ancestor(query.clone());
                let mut atom = f
                    .graph
                    .find_ancestor(text.iter().copied().collect::<String>().chars());
                let mut offset = 0;
                loop {
                    let context = format!("{context} at {offset}");
                    let (c, a) = match (composite, atom) {
                        (Ok(c), Ok(a)) => (c, a),
                        // The rest is one query token: legacy rejects it.
                        (Err(SearchError::SingleIndex(t)), a) => {
                            if t.width.0 == 1 {
                                assert_eq!(a, Err(SearchError::SingleIndex(t)), "{context}");
                            } else {
                                let a = a.unwrap();
                                assert_eq!(a.root_token(), t, "{context}");
                                assert!(a.query_exhausted() && a.is_entire_root(), "{context}");
                            }
                            break;
                        }
                        (c, a) => panic!("{context}: {c:?} vs {a:?}"),
                    };
                    let rest: String = text[offset..].iter().collect();
                    validate(&oracle, &rest, offset, &c);
                    assert_eq!(c.path(), a.path(), "{context}: paths");
                    assert_eq!(
                        c.checkpoint_position(),
                        a.checkpoint_position(),
                        "{context}"
                    );
                    assert_eq!(cache_edges(&c, 0), cache_edges(&a, 0), "{context}: cache");
                    searches += 1;
                    unmatched_first += usize::from(
                        c.root_token().width.0 > 1
                            && c.is_entire_root()
                            && c.cache.edges().next().is_none()
                            && c.root_token() == c.query_cursor().path().pattern()[0]
                            && offset == 0,
                    );
                    if c.query_exhausted() {
                        break;
                    }
                    offset = *c.checkpoint_position();
                    inside += usize::from(!c.query_cursor().path().end().is_empty());
                    composite = f.graph.find_ancestor(c.query_cursor());
                    atom = f.graph.find_ancestor(a.query_cursor());
                }
            }
        }
    }
    eprintln!(
        "composite queries: {} splits, {} mixed, {} extended, {} key pairs; {searches} searches, \
         {inside} continuations inside a query token, {unmatched_first} with an unmatched \
         composite first token",
        counts[0], counts[1], counts[2], counts[3]
    );
    assert!(unmatched_first > 0);
}

/// Continuation (2.5): chained searches from each checkpoint over every
/// corpus text (and every concatenation of two texts) equal fresh searches
/// of the rest at the same positions, and compare only positions after the
/// checkpoint, exactly those of the fresh search.
#[test]
fn chained_searches_equal_fresh_searches() {
    let (mut texts, mut chains) = (0, 0);
    for f in v3_graphs() {
        let oracle = Oracle::new(&f);
        let pairs = f
            .corpus
            .iter()
            .flat_map(|a| f.corpus.iter().map(move |b| format!("{a}{b}")));
        for text in f.corpus.clone().into_iter().chain(pairs) {
            texts += 1;
            let atoms = chars(&text);
            let mut response = f.graph.find_ancestor(text.chars()).unwrap();
            validate(&oracle, &text, 0, &response);
            while !response.query_exhausted() {
                let offset = *response.checkpoint_position();
                let rest: String = atoms[offset..].iter().collect();
                let context = format!("{}: {text} at {offset}", f.name);
                let fresh = f.graph.find_ancestor(rest.chars());
                let fresh_compared = last_search_stats().compared_positions;
                let next = f.graph.find_ancestor(response.query_cursor());
                let stats = last_search_stats();
                let (next, fresh) = match (next, fresh) {
                    (Ok(next), Ok(fresh)) => (next, fresh),
                    (next, fresh) => {
                        // A single remaining atom: both reject it.
                        assert_eq!(next, fresh, "{context}");
                        assert_eq!(atoms.len() - offset, 1, "{context}");
                        break;
                    }
                };
                validate(&oracle, &rest, offset, &next);
                assert_eq!(stats.equal_width_first_matches, 0, "{context}");
                assert_eq!(next.path().coverage(), fresh.path().coverage(), "{context}");
                assert_eq!(
                    next.path().range().map(|r| &r.path),
                    fresh.path().range().map(|r| &r.path),
                    "{context}"
                );
                assert_eq!(
                    cache_edges(&next, offset),
                    cache_edges(&fresh, 0),
                    "{context}: cache"
                );
                assert!(
                    stats.compared_positions.iter().all(|&p| *p >= offset),
                    "{context}: re-compared {:?}",
                    stats.compared_positions
                );
                assert_eq!(
                    stats
                        .compared_positions
                        .iter()
                        .map(|&p| *p - offset)
                        .collect::<Vec<_>>(),
                    fresh_compared.iter().map(|&p| *p).collect::<Vec<_>>(),
                    "{context}: compared positions"
                );
                chains += 1;
                response = next;
            }
        }
    }
    eprintln!("chained: {texts} texts, {chains} continuations");
}
