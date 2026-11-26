//! Oracle fixture support for the frozen v2 row lane ([`legacy`]:
//! `tests/fixtures/v2/<name>.json`, manifest `v2/scenarios.json`).
//!
//! This module holds manifest and file loading.
pub mod legacy;

pub use ngrams::output::Scenario;
use std::path::{Path, PathBuf};

/// File name of a lane's scenario manifest inside its fixture directory.
pub const MANIFEST: &str = "scenarios.json";

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Scenario manifest of the lane in `dir`.
fn manifest(dir: &Path) -> Vec<Scenario> {
    read_json(&dir.join(MANIFEST))
}

/// Fixture names (file stems) in `dir`, excluding the manifest.
pub fn fixture_names(dir: &Path) -> std::collections::BTreeSet<String> {
    std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter(|path| path.file_name().is_some_and(|name| name != MANIFEST))
        .map(|path| path.file_stem().unwrap().to_str().unwrap().to_owned())
        .collect()
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    serde_json::from_slice(
        &std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}
