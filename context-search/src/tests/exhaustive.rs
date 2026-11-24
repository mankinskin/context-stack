//! Plan 2.6: exhaustive validation of positioned search on every
//! fixture. Queries and expectations come only from the fixture strings
//! (corpus texts and keys of the name map) and from the fixture graph
//! (spans with deep start / end descents); every response is checked by
//! [`validate`]. Suite-level depth floors ([`Depth`]) make sure the deep
//! fixtures keep exercising long paths, raises and decompositions.

use context_trace::positioned::{
        GraphRead,
        Navigate,
    };
use std::collections::{BTreeMap, BTreeSet};

use context_trace::{
    positioned::fixtures::{fixture_graphs, FixtureGraph},
    Token,
};
use pretty_assertions::assert_eq;

use super::{
    util::chars,
    validate::{cache_edges, validate, Oracle},
};
use crate::{
    r#match::{with_tie_order, TieOrder},
    search::stats::{last_search_stats, SearchStats},
    Find, Response, SearchError,
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

fn alphabet(f: &FixtureGraph) -> Vec<char> {
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

/// Number of spans per fixture in [`deep_spans`].
const DEEP_SPANS: usize = 16;

/// Graph-derived spans `root[s..e]` (texts) of every composite fixture key
/// with the longest descents `start_at(root, s)` + `end_at(root, e)`, the
/// `DEEP_SPANS` deepest per fixture (ties by text). These are the queries
/// whose canonical start and end paths can be longest.
fn deep_spans(f: &FixtureGraph) -> Vec<Vec<char>> {
    let mut spans = BTreeMap::new();
    for (key, &root) in &f.names {
        let key = chars(key);
        for s in 0..key.len() {
            for e in s + 2..=key.len() {
                let vertex = f.graph.vertex(root).unwrap();
                let start = vertex.start_at(s).map_or(0, |(_, steps)| steps.len());
                let end = vertex.end_at(e).map_or(0, |(_, steps)| steps.len());
                let depth = spans.entry(key[s..e].to_vec()).or_insert(0);
                *depth = (*depth).max(start + end);
            }
        }
    }
    let mut spans: Vec<_> = spans.into_iter().collect();
    spans.sort_by(|(a, x), (b, y)| y.cmp(x).then(a.cmp(b)));
    spans
        .into_iter()
        .take(DEEP_SPANS)
        .map(|(text, _)| text)
        .collect()
}

/// Query sets of 2.6: every substring of every corpus text and of every
/// vertex key (keys are corpus substrings, so this documents rather than
/// extends the set), each extended by every alphabet atom, graph-derived
/// deep spans ([`deep_spans`]) followed by every atom and by every deep
/// span, and seeded random strings over the alphabet.
fn atom_queries(f: &FixtureGraph) -> [(&'static str, Vec<String>); 4] {
    let alphabet = alphabet(f);
    let mut subs = BTreeSet::new();
    let mut extended = BTreeSet::new();
    for text in f.corpus.iter().chain(f.names.keys()) {
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
    let spans = deep_spans(f);
    let mut deep = BTreeSet::new();
    for span in &spans {
        let span: String = span.iter().collect();
        deep.insert(span.clone());
        for atom in &alphabet {
            deep.insert(format!("{span}{atom}"));
        }
        for other in &spans {
            deep.insert(format!("{span}{}", other.iter().collect::<String>()));
        }
    }
    [
        ("substrings", subs.into_iter().collect()),
        ("extended", extended.into_iter().collect()),
        ("deep", deep.into_iter().collect()),
        ("random", random),
    ]
}

/// Depth reached over a set of searches: response path lengths (with a
/// `(start, end)` histogram) and the maxima of the [`SearchStats`] depth
/// counters.
#[derive(Default)]
struct Depth {
    histogram: BTreeMap<(usize, usize), usize>,
    start: usize,
    end: usize,
    stats: SearchStats,
}

impl Depth {
    fn record(&mut self, response: &Response, stats: &SearchStats) {
        let start = response
            .path()
            .start_path()
            .map_or(0, |p| p.len());
        let end = response.path().end_path().map_or(0, |p| p.len());
        *self.histogram.entry((start, end)).or_default() += 1;
        self.start = self.start.max(start);
        self.end = self.end.max(end);
        let max = &mut self.stats;
        max.parent_raises = max.parent_raises.max(stats.parent_raises);
        max.graph_prefix_descents = max.graph_prefix_descents.max(stats.graph_prefix_descents);
        max.query_prefix_descents = max.query_prefix_descents.max(stats.query_prefix_descents);
        max.max_start_path_len = max.max_start_path_len.max(stats.max_start_path_len);
        max.max_end_path_len = max.max_end_path_len.max(stats.max_end_path_len);
        assert!(stats.max_start_path_len >= start && stats.max_end_path_len >= end);
    }
    /// Suite-level coverage floors.
    fn check(&self, floor: &Floor) {
        let s = &self.stats;
        assert!(
            self.start >= floor.start,
            "response start path length {}",
            self.start
        );
        assert!(
            self.end >= floor.end,
            "response end path length {}",
            self.end
        );
        assert!(
            self.histogram
                .keys()
                .any(|&(start, end)| start >= floor.both && end >= floor.both),
            "response with start and end paths of length {}",
            floor.both
        );
        assert!(
            s.parent_raises >= floor.raises,
            "raises {}",
            s.parent_raises
        );
        assert!(
            s.graph_prefix_descents >= floor.graph_descents,
            "graph prefix descents {}",
            s.graph_prefix_descents
        );
        assert!(
            s.query_prefix_descents >= floor.query_descents,
            "query prefix descents {}",
            s.query_prefix_descents
        );
        assert!(s.max_start_path_len >= floor.start && s.max_end_path_len >= floor.end);
    }
    fn report(&self, suite: &str) {
        let s = &self.stats;
        eprintln!(
            "{suite}: response (start, end) path lengths {:?}; max response start {}, end {}; \
             max per search: {} raises, {} graph / {} query prefix descents, \
             state start path {}, end path {}",
            self.histogram,
            self.start,
            self.end,
            s.parent_raises,
            s.graph_prefix_descents,
            s.query_prefix_descents,
            s.max_start_path_len,
            s.max_end_path_len,
        );
    }
}

/// Every atom query of 2.6 matches the brute-force expectation; ascending
/// and descending tie orders give identical responses (paths and trace
/// caches included); the uniqueness counter is 0 on every fixture.
#[test]
fn atom_queries_match_brute_force() {
    let mut counts = [0usize; 4];
    let (mut probed, mut fixtures) = (0, 0);
    let mut depth = Depth::default();
    for f in fixture_graphs() {
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
                depth.record(&response, &stats);
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
        "atom queries on {fixtures} fixtures: {} substrings, {} extended, {} deep, {} random; \
         {probed} equal-width candidates probed, none matching at the first comparison",
        counts[0], counts[1], counts[2], counts[3]
    );
    depth.report("atom queries");
    depth.check(&ATOM_FLOOR);
}

/// Minimal depth some search of a suite must reach.
struct Floor {
    start: usize,
    end: usize,
    /// Some response has start and end paths of at least this length.
    both: usize,
    raises: usize,
    graph_descents: usize,
    query_descents: usize,
}

/// Path length floors are exact maxima of the canonical paths on the deep
/// fixtures (`deep_chains`, `deep_prefix_chain`, `deep_suffix_chain`);
/// counter floors leave some room below the measured maxima (20 raises, 20
/// graph and 36 query prefix descents) for search changes.
const ATOM_FLOOR: Floor = Floor {
    start: 7,
    end: 7,
    both: 6,
    raises: 15,
    graph_descents: 15,
    query_descents: 0,
};

const COMPOSITE_FLOOR: Floor = Floor {
    query_descents: 25,
    ..ATOM_FLOOR
};

/// Longest fixture key at the start of `text`.
fn longest_key<'k>(f: &'k FixtureGraph, text: &[char]) -> (&'k String, Token) {
    f.names
        .iter()
        .filter(|(key, _)| text.starts_with(&chars(key)))
        .max_by_key(|(key, _)| key.chars().count())
        .map(|(key, &token)| (key, token))
        .unwrap()
}

/// `text` split greedily into the longest fixture keys.
fn greedy_split(f: &FixtureGraph, mut text: &[char]) -> Vec<Token> {
    let mut tokens = vec![];
    while !text.is_empty() {
        let (key, token) = longest_key(f, text);
        tokens.push(token);
        text = &text[key.chars().count()..];
    }
    tokens
}

/// `text` split greedily from the right into the longest fixture keys.
fn greedy_split_rev(f: &FixtureGraph, mut text: &[char]) -> Vec<Token> {
    let mut tokens = vec![];
    while !text.is_empty() {
        let (key, token) = f
            .names
            .iter()
            .filter(|(key, _)| text.ends_with(&chars(key)))
            .max_by_key(|(key, _)| key.chars().count())
            .unwrap();
        tokens.push(token);
        text = &text[..text.len() - key.chars().count()];
    }
    tokens.reverse();
    tokens.into_iter().copied().collect()
}

fn atoms(f: &FixtureGraph, text: &[char]) -> Vec<Token> {
    text.iter().map(|c| f.names[&c.to_string()]).collect()
}

/// Composite query sequences of `f` with their strings: each corpus text
/// split into covering fixture tokens (one chosen key at every position,
/// greedy longest keys around it), mixed atom/token sequences (one
/// composite key at every position of every corpus substring, atoms
/// around it), both extended by every alphabet atom (mismatches after or
/// inside composite tokens), every pair of fixture keys, and misaligned
/// cuts of the deep spans ([`deep_spans`], alone and followed by each
/// other): at every cut position, the left part split greedily from the
/// right and the right part from the left, so that query token boundaries
/// fall inside deep graph tokens and both sides decompose over several
/// levels.
fn composite_queries(f: &FixtureGraph) -> [(&'static str, BTreeSet<(String, Vec<Token>)>); 5] {
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
    let spans = deep_spans(f);
    let mut cuts = BTreeSet::new();
    for (i, span) in spans.iter().enumerate() {
        for text in std::iter::once(span.clone()).chain(
            spans[i..]
                .iter()
                .take(4)
                .map(|other| span.iter().chain(other).copied().collect()),
        ) {
            for cut in 1..text.len() {
                let (left, right) = text.split_at(cut);
                let mut query = greedy_split_rev(f, left);
                query.extend(greedy_split(f, right));
                cuts.insert((text.iter().collect(), query));
                let mut query = greedy_split(f, left);
                query.extend(greedy_split_rev(f, right));
                cuts.insert((text.iter().collect(), query));
            }
        }
    }
    [
        ("splits", splits),
        ("mixed", mixed),
        ("extended", extended),
        ("pairs", pairs),
        ("cuts", cuts),
    ]
}

/// Composite queries (Decision 6 b) give the same root, span, coverage,
/// paths and trace-cache edges as the equivalent atom query, chained to the
/// query end (continuations may start inside a composite query token).
/// Includes an unmatched composite first token (2.6 finding: it used to be
/// `EntireRoot` without paths).
#[test]
fn composite_queries_equal_atom_queries() {
    let mut counts = [0usize; 5];
    let (mut searches, mut inside, mut unmatched_first) = (0, 0, 0);
    let mut depth = Depth::default();
    for f in fixture_graphs() {
        let oracle = Oracle::new(&f);
        for (i, (category, queries)) in composite_queries(&f).into_iter().enumerate() {
            for (text, query) in queries {
                counts[i] += 1;
                let text = chars(&text);
                let context = format!("{} ({category}): {query:?}", f.name);
                let mut composite = f.graph.find_ancestor(query.clone());
                let mut stats = last_search_stats();
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
                    depth.record(&c, &stats);
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
                    inside += usize::from(!c.query_cursor().path().is_empty());
                    composite = f.graph.find_ancestor(c.query_cursor());
                    stats = last_search_stats();
                    atom = f.graph.find_ancestor(a.query_cursor());
                }
            }
        }
    }
    eprintln!(
        "composite queries: {} splits, {} mixed, {} extended, {} key pairs, {} deep cuts; \
         {searches} searches, {inside} continuations inside a query token, {unmatched_first} \
         with an unmatched composite first token",
        counts[0], counts[1], counts[2], counts[3], counts[4]
    );
    depth.report("composite queries");
    assert!(unmatched_first > 0);
    depth.check(&COMPOSITE_FLOOR);
}

/// Continuation (2.5): chained searches from each checkpoint over every
/// corpus text (and every concatenation of two texts or of two deep spans,
/// [`deep_spans`], so that continuations start inside deep tokens) equal
/// fresh searches
/// of the rest at the same positions, and compare only positions after the
/// checkpoint, exactly those of the fresh search.
#[test]
fn chained_searches_equal_fresh_searches() {
    let (mut texts, mut chains) = (0, 0);
    for f in fixture_graphs() {
        let oracle = Oracle::new(&f);
        let pairs = f
            .corpus
            .iter()
            .flat_map(|a| f.corpus.iter().map(move |b| format!("{a}{b}")));
        let spans: Vec<String> = deep_spans(&f)
            .into_iter()
            .take(8)
            .map(|span| span.into_iter().collect())
            .collect();
        let deep: BTreeSet<String> = spans
            .iter()
            .flat_map(|a| spans.iter().map(move |b| format!("{a}{b}")))
            .collect();
        for text in f.corpus.clone().into_iter().chain(pairs).chain(deep) {
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
