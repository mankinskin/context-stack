//! Oracle fixture support with two explicit lanes:
//!
//! - [`legacy`]: frozen v2 row fixtures (`tests/fixtures/<name>.json`).
//! - [`positioned`]: v3 positioned fixtures (`tests/fixtures/v3/<name>.json`).
//!
//! This module holds what both lanes share: the scenario manifest and file
//! loading.
pub mod legacy;
pub mod positioned;

pub use ngrams::output::Scenario;
use std::path::PathBuf;

pub fn scenarios() -> Vec<Scenario> {
    serde_json::from_str(include_str!("../fixtures/scenarios.json"))
        .expect("invalid oracle scenario manifest")
}

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> T {
    serde_json::from_slice(
        &std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}
