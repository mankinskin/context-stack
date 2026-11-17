//! CLI for generating validated complete graphs from inline texts or scenario files.
//!
//! Graph JSON goes to stdout in inline mode and to named files in batch mode.
//! Diagnostics go to stderr; a failed batch may leave earlier successful outputs.

use clap::Parser;
use ngrams::{
    graph::{parse_corpus, Corpus, Status, StatusHandle},
    output::{GraphData, GraphSnapshot, Scenario, FORMAT_VERSION},
    Cancellation,
};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

/// Generate a complete n-gram graph for each input corpus.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Texts in a single corpus; prints its graph JSON to stdout.
    #[arg(
        value_name = "TEXT",
        required_unless_present = "file",
        conflicts_with = "file"
    )]
    inputs: Vec<String>,

    /// JSON scenario object or array of {name, inputs} objects.
    #[arg(long, value_name = "SCENARIOS", requires = "out")]
    file: Option<PathBuf>,

    /// Output directory for named graphs (required with --file).
    #[arg(long, value_name = "DIRECTORY", requires = "file")]
    out: Option<PathBuf>,
}

/// Accepted file shapes, normalized to a list before validation.
#[derive(Deserialize)]
#[serde(untagged)]
enum Scenarios {
    /// One corpus described directly by an object.
    One(Scenario),
    /// Several independent named corpora.
    Many(Vec<Scenario>),
}

/// Run the oracle for one nonempty corpus and validate its content-keyed output.
fn generate(name: &str, inputs: Vec<String>) -> Result<GraphSnapshot, Box<dyn Error>> {
    if inputs.is_empty() || inputs.iter().any(String::is_empty) {
        return Err(format!("{name}: a corpus must contain nonempty inputs").into());
    }
    let parsed = parse_corpus(
        Corpus::new(name, &inputs),
        StatusHandle::from(Status::new(&inputs)),
        Cancellation::None,
    )
    .map_err(|error| format!("oracle failed for {name}: {error:?}"))?;
    let snapshot = GraphSnapshot {
        format_version: FORMAT_VERSION,
        corpus: inputs,
        graph: GraphData::from_graph(&parsed.graph)?,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

/// Dispatch modes, validating the entire batch's metadata before writing graphs.
///
/// Generation and file replacement remain sequential and are not transactional.
fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    if let Some(file) = cli.file {
        let scenarios = match serde_json::from_slice::<Scenarios>(&fs::read(&file)?)? {
            Scenarios::One(scenario) => vec![scenario],
            Scenarios::Many(scenarios) => scenarios,
        };
        if scenarios.is_empty() {
            return Err("scenario file must contain at least one corpus".into());
        }
        let mut names = BTreeSet::new();
        // Restrict names to safe filename stems and reject collisions up front.
        for scenario in &scenarios {
            if scenario.name.is_empty()
                || scenario.name == "."
                || scenario.name == ".."
                || !scenario
                    .name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_-.".contains(&c))
                || !names.insert(&scenario.name)
            {
                return Err(
                    format!("invalid or duplicate scenario name: {:?}", scenario.name).into(),
                );
            }
            if scenario.inputs.is_empty() || scenario.inputs.iter().any(String::is_empty) {
                return Err(
                    format!("{}: a corpus must contain nonempty inputs", scenario.name).into(),
                );
            }
        }
        let directory = cli.out.ok_or("--file requires --out")?;
        fs::create_dir_all(&directory)?;
        for scenario in scenarios {
            eprintln!("Generating {}", scenario.name);
            let snapshot = generate(&scenario.name, scenario.inputs)?;
            let path = directory.join(format!("{}.json", scenario.name));
            let mut output = fs::File::create(path)?;
            serde_json::to_writer_pretty(&mut output, &snapshot)?;
            writeln!(output)?;
        }
    } else {
        let snapshot = generate("inline", cli.inputs)?;
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, &snapshot)?;
        writeln!(stdout)?;
    }
    Ok(())
}

/// Parse arguments and report operational failures through stderr and exit status.
fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
