//! Deterministic corpora for the oracle and search tests.

use std::collections::BTreeSet;

/// Every string over `alphabet` of length `1..=max`.
pub fn strings(
    alphabet: &[char],
    max: usize,
) -> Vec<String> {
    let mut all = Vec::new();
    let mut layer = vec![String::new()];
    for _ in 0..max {
        layer = layer
            .iter()
            .flat_map(|s| alphabet.iter().map(move |c| format!("{s}{c}")))
            .collect();
        all.extend(layer.iter().cloned());
    }
    all
}

/// xorshift64*, enough for a reproducible sample.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `lo..=hi`.
    pub fn range(
        &mut self,
        lo: usize,
        hi: usize,
    ) -> usize {
        lo + (self.next() % (hi - lo + 1) as u64) as usize
    }

    pub fn text(
        &mut self,
        alphabet: &[char],
        len: usize,
    ) -> String {
        (0..len)
            .map(|_| alphabet[self.range(0, alphabet.len() - 1)])
            .collect()
    }
}

/// Corpora of the oracle test: every text over `{a, b}` up to length 8 and
/// over `{a, b, c}` up to length 5, every pair of texts over `{a, b}` up to
/// length 4, and a seeded sample of 1–3 texts up to length 12. Every one of
/// them is read exactly like the oracle by the positioned reader (checked
/// 2026-11-26 with `context_read::read` against `parse_corpus`, no
/// deviations), so a difference is an API bug.
pub fn oracle_corpora() -> Vec<Vec<String>> {
    corpora(8, 5, 4, 300)
}

/// A smaller set for the per-query tests (search, contexts).
pub fn search_corpora() -> Vec<Vec<String>> {
    corpora(6, 4, 3, 60)
}

fn corpora(
    ab_max: usize,
    abc_max: usize,
    pair_max: usize,
    random: usize,
) -> Vec<Vec<String>> {
    let mut corpora = BTreeSet::new();
    let ab = ['a', 'b'];
    let abc = ['a', 'b', 'c'];
    for text in strings(&ab, ab_max).into_iter().chain(strings(&abc, abc_max)) {
        corpora.insert(vec![text]);
    }
    let short = strings(&ab, pair_max);
    for first in &short {
        for second in &short {
            corpora.insert(vec![first.clone(), second.clone()]);
        }
    }
    corpora.extend(random_corpora(0x5eed_0001, random));
    corpora.into_iter().collect()
}

/// `count` corpora of 1–3 texts over `{a, b}` or `{a, b, c}`, lengths
/// 1..=12.
pub fn random_corpora(
    seed: u64,
    count: usize,
) -> Vec<Vec<String>> {
    let mut rng = Rng::new(seed);
    (0..count)
        .map(|i| {
            let alphabet: &[char] = if i % 2 == 0 { &['a', 'b'] } else { &['a', 'b', 'c'] };
            let texts = rng.range(1, 3);
            (0..texts)
                .map(|_| {
                    let len = rng.range(1, 12);
                    rng.text(alphabet, len)
                })
                .collect()
        })
        .collect()
}
