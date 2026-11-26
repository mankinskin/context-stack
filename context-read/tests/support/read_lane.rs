//! The read lane of the reader fixtures: `tests/fixtures/v3/read` holds one
//! oracle graph per read of a multi-input scenario (reader plan §9.1); its
//! manifest is derived from the v3 manifest by [`read_manifest`]. Shared by
//! the integration tests and the `read_manifest` example.

use std::path::PathBuf;

use context_trace::positioned::{
    fixtures::{
        Scenario,
        fixtures_dir,
        manifest,
        snapshot,
    },
    interchange::PositionedGraphSnapshot,
};

/// Directory of the read lane (one oracle graph per read).
pub fn read_dir() -> PathBuf {
    fixtures_dir().join("read")
}

/// The read lane manifest, derived from the v3 manifest: for every scenario
/// with `n ≥ 2` inputs and every `k ∈ 1..=n`, the entry `<scenario>__<k>`
/// with inputs `inputs[..k]`.
pub fn read_manifest() -> Vec<Scenario> {
    manifest(&fixtures_dir())
        .into_iter()
        .filter(|scenario| scenario.inputs.len() >= 2)
        .flat_map(|Scenario { name, inputs }| {
            (1..=inputs.len())
                .map(|k| Scenario {
                    name: format!("{name}__{k}"),
                    inputs: inputs[..k].to_vec(),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// One read case: the oracle graph of the earlier inputs (`None` for the
/// first read), the input to read and the oracle graph after it.
pub struct ReadCase {
    /// `<scenario>__<k>`.
    pub name: String,
    pub base: Option<PositionedGraphSnapshot>,
    pub input: String,
    pub expected: PositionedGraphSnapshot,
}

/// Every entry of the read lane manifest; case `k` reads into the graph of
/// case `k − 1`.
pub fn read_cases() -> Vec<ReadCase> {
    let mut previous: Option<(String, PositionedGraphSnapshot)> = None;
    manifest(&read_dir())
        .into_iter()
        .map(|Scenario { name, mut inputs }| {
            let (scenario, k) = name
                .rsplit_once("__")
                .and_then(|(scenario, k)| Some((scenario.to_string(), k.parse::<usize>().ok()?)))
                .unwrap_or_else(|| panic!("{name}: not <scenario>__<k>"));
            assert_eq!(inputs.len(), k, "{name}: inputs must be the first {k} inputs");
            let expected = snapshot(&read_dir(), &name);
            assert_eq!(expected.corpus, inputs, "{name}: corpus must equal the inputs");
            let input = inputs.pop().unwrap_or_else(|| panic!("{name}: no inputs"));
            let base = (k > 1).then(|| {
                let (previous_name, base) =
                    previous.take().unwrap_or_else(|| panic!("{name}: no previous case"));
                assert_eq!(
                    previous_name,
                    format!("{scenario}__{}", k - 1),
                    "{name}: cases of a scenario must be consecutive"
                );
                assert_eq!(base.corpus, inputs, "{name}: base corpus must be the earlier inputs");
                base
            });
            previous = Some((name.clone(), expected.clone()));
            ReadCase {
                name,
                base,
                input,
                expected,
            }
        })
        .collect()
}
