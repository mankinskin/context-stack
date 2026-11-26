//! Write the read lane manifest `context-read/tests/fixtures/v3/read/scenarios.json`
//! (reader plan §9.1) and remove the lane's old case files.
//! Then generate the cases with the `ngrams` CLI (see context-read/README.md).
//!
//! ```sh
//! cargo run -p context-read --example read_manifest
//! ```

// The read lane of the integration tests; the example needs only the manifest.
#[allow(dead_code)]
#[path = "../tests/support/read_lane.rs"]
mod read_lane;

use read_lane::{
    read_dir,
    read_manifest,
};

fn main() -> std::io::Result<()> {
    let dir = read_dir();
    std::fs::create_dir_all(&dir)?;
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            std::fs::remove_file(path)?;
        }
    }
    let entries: Vec<String> = read_manifest()
        .iter()
        .map(|entry| {
            format!("  {}", serde_json::to_string(entry).expect("serializable"))
        })
        .collect();
    let manifest = dir.join("scenarios.json");
    std::fs::write(&manifest, format!("[\n{}\n]\n", entries.join(",\n")))?;
    println!("{}: {} read cases", manifest.display(), entries.len());
    Ok(())
}
