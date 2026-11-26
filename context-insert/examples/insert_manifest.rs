//! Write the insert lane manifest `context-read/tests/fixtures/v3/insert/scenarios.json`
//! (insert plan rev 3, task 3.1.1) and remove the lane's old case files.
//! Then generate the cases with the `ngrams` CLI (see context-read/README.md).
//!
//! ```sh
//! cargo run -p context-insert --features test-api --example insert_manifest
//! ```

use context_insert::fixtures::{
    insert_dir,
    insert_manifest,
};

fn main() -> std::io::Result<()> {
    let dir = insert_dir();
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
    let entries: Vec<String> = insert_manifest()
        .iter()
        .map(|entry| {
            format!("  {}", serde_json::to_string(entry).expect("serializable"))
        })
        .collect();
    let manifest = dir.join("scenarios.json");
    std::fs::write(&manifest, format!("[\n{}\n]\n", entries.join(",\n")))?;
    println!("{}: {} insert cases", manifest.display(), entries.len());
    Ok(())
}
