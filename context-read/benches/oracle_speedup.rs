//! Speedup of the incremental reader over the n-gram oracle
//! ([`ngrams::graph::parse_corpus`]). Both build the graph of the same
//! corpus: the reader reads its inputs one by one into an empty graph, the
//! oracle builds it in a single run over all inputs.
//!
//! Workloads:
//! - `corpus/<scenario>`: every fixture scenario.
//! - `periodic/abc^n_x` and `random/k4`: single inputs of growing length.
//! - `multi/k4_len64/<n>`: `n` random inputs of 64 atoms.
//!
//! Every single run is limited to a time budget (default 30 s). The oracle is
//! cancelled cooperatively once the budget passes and the row reports
//! `>budget (timeout)` with a lower bound for the speedup; larger sizes of the
//! same series are then skipped. The reader cannot be cancelled, so a reader
//! run that exceeds the budget only stops its series. Lift the budget for
//! exact numbers with `--no-timeout`, `CONTEXT_BENCH_BUDGET=none` or the
//! `long-bench` feature.
//!
//! ```sh
//! cargo bench -p context-read --bench oracle_speedup --no-default-features --features test-api
//! cargo bench -p context-read --bench oracle_speedup --no-default-features --features test-api -- --budget 120 --filter periodic
//! cargo bench -p context-read --bench oracle_speedup --no-default-features --features test-api,long-bench
//! ```
//!
//! Options (after `--`): `--budget <secs>`, `--no-timeout`, `--filter <substr>`.
//! Results are printed and written to
//! `target/bench-results/oracle_speedup.csv`.

use std::{
    env,
    fmt::Write as _,
    fs,
    hint::black_box,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{
            AtomicBool,
            Ordering,
        },
        mpsc::{
            self,
            RecvTimeoutError,
        },
    },
    thread,
    time::{
        Duration,
        Instant,
    },
};

use context_read::read;
use context_trace::positioned::{
    PositionedGraphRef,
    fixtures::{
        fixtures_dir,
        manifest,
    },
};
use ngrams::graph::{
    Corpus,
    Status,
    StatusHandle,
    parse_corpus,
    traversal::pass::CancelReason,
};
use rand::{
    Rng,
    SeedableRng,
    rngs::StdRng,
};

const DEFAULT_BUDGET: Duration = Duration::from_secs(30);
/// Runs faster than this are repeated and reported as a median.
const REPEAT_BELOW: Duration = Duration::from_secs(1);
const MAX_SAMPLES: usize = 10;

struct Config {
    /// `None`: no limit.
    budget: Option<Duration>,
    filter: Option<String>,
}

impl Config {
    fn from_env() -> Self {
        let mut budget = if cfg!(feature = "long-bench") {
            None
        } else {
            Some(DEFAULT_BUDGET)
        };
        if let Ok(value) = env::var("CONTEXT_BENCH_BUDGET") {
            budget = parse_budget(&value);
        }
        let mut filter = None;
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--no-timeout" => budget = None,
                "--budget" => {
                    budget = parse_budget(&args.next().expect("--budget <secs|none>"))
                },
                "--filter" => filter = Some(args.next().expect("--filter <substr>")),
                // `cargo bench` passes `--bench`; ignore other harness flags.
                _ => {},
            }
        }
        Self { budget, filter }
    }
}

fn parse_budget(value: &str) -> Option<Duration> {
    match value {
        "none" | "0" | "inf" => None,
        secs => Some(Duration::from_secs_f64(
            secs.parse().unwrap_or_else(|_| panic!("invalid budget: {}", secs)),
        )),
    }
}

#[derive(Clone, Copy)]
enum Outcome {
    Done(Duration),
    /// Exceeded the budget after the given time.
    TimedOut(Duration),
    /// Not run: a smaller size of the same series exceeded the budget.
    Skipped,
}

/// One oracle run, cancelled once `budget` has passed.
fn run_oracle(
    inputs: &[String],
    budget: Option<Duration>,
) -> Outcome {
    let cancel = Arc::new(AtomicBool::new(false));
    let (done, finished) = mpsc::channel::<()>();
    let watchdog = budget.map(|budget| {
        let cancel = cancel.clone();
        thread::spawn(move || {
            // A disconnect means the oracle finished; only a timeout cancels.
            if let Err(RecvTimeoutError::Timeout) = finished.recv_timeout(budget) {
                cancel.store(true, Ordering::SeqCst);
            }
        })
    });
    let start = Instant::now();
    let result = parse_corpus(
        Corpus::new("oracle_speedup", inputs),
        StatusHandle::from(Status::new(inputs)),
        cancel,
    );
    let elapsed = start.elapsed();
    drop(done);
    if let Some(watchdog) = watchdog {
        watchdog.join().expect("watchdog");
    }
    match result {
        Ok(parsed) => {
            black_box(parsed);
            Outcome::Done(elapsed)
        },
        Err(CancelReason::Cancelled) => Outcome::TimedOut(elapsed),
        Err(error) => panic!("oracle failed: {:?}", error),
    }
}

/// Repeat `run` while single runs are short; the median of the samples.
fn sample(mut run: impl FnMut() -> Outcome) -> Outcome {
    let first = match run() {
        Outcome::Done(time) => time,
        other => return other,
    };
    let mut times = vec![first];
    let mut total = first;
    while first < REPEAT_BELOW && times.len() < MAX_SAMPLES && total < 2 * REPEAT_BELOW
    {
        match run() {
            Outcome::Done(time) => {
                total += time;
                times.push(time);
            },
            other => return other,
        }
    }
    times.sort();
    Outcome::Done(times[times.len() / 2])
}

/// Time of reading `inputs` into `graph`; the reader cannot be cancelled, so
/// exceeding the budget is only reported.
fn time_read(
    graph: &PositionedGraphRef,
    inputs: &[String],
    budget: Option<Duration>,
) -> Outcome {
    let start = Instant::now();
    for input in inputs {
        black_box(read(graph, input.chars()));
    }
    let elapsed = start.elapsed();
    match budget {
        Some(budget) if elapsed > budget => Outcome::TimedOut(elapsed),
        _ => Outcome::Done(elapsed),
    }
}

/// Benchmark case: both algorithms build the graph of `inputs`.
struct Case {
    series: String,
    size: String,
    inputs: Vec<String>,
}

impl Case {
    fn new(
        series: impl ToString,
        size: impl ToString,
        inputs: Vec<String>,
    ) -> Self {
        Self {
            series: series.to_string(),
            size: size.to_string(),
            inputs,
        }
    }
    fn name(&self) -> String {
        format!("{}/{}", self.series, self.size)
    }
}

struct Row {
    name: String,
    atoms: usize,
    read: Outcome,
    oracle: Outcome,
}

fn random_text(
    rng: &mut StdRng,
    len: usize,
    alphabet: u8,
) -> String {
    (0..len)
        .map(|_| (b'a' + rng.random_range(0..alphabet)) as char)
        .collect()
}

fn cases() -> Vec<Case> {
    let mut cases: Vec<Case> = manifest(&fixtures_dir())
        .into_iter()
        .map(|scenario| Case::new("corpus", scenario.name, scenario.inputs))
        .collect();
    for n in [16, 32, 64, 128, 256, 512] {
        cases.push(Case::new(
            "periodic/abc^n_x",
            n,
            vec![format!("{}x", "abc".repeat(n))],
        ));
    }
    let mut rng = StdRng::seed_from_u64(0x5eed);
    for len in [64, 128, 256, 512, 1024, 2048] {
        cases.push(Case::new(
            "random/k4",
            len,
            vec![random_text(&mut rng, len, 4)],
        ));
    }
    let mut rng = StdRng::seed_from_u64(0xa99e);
    let inputs: Vec<String> =
        (0..64).map(|_| random_text(&mut rng, 64, 4)).collect();
    for n in [2, 4, 8, 16, 32, 64] {
        cases.push(Case::new("multi/k4_len64", n, inputs[..n].to_vec()));
    }
    cases
}

fn run_case(
    case: &Case,
    config: &Config,
    skip_oracle: bool,
    skip_read: bool,
) -> Row {
    let read_outcome = if skip_read {
        Outcome::Skipped
    } else {
        sample(|| {
            time_read(&PositionedGraphRef::default(), &case.inputs, config.budget)
        })
    };
    let oracle_outcome = if skip_oracle {
        Outcome::Skipped
    } else {
        sample(|| run_oracle(&case.inputs, config.budget))
    };
    Row {
        name: case.name(),
        atoms: case.inputs.iter().map(|input| input.chars().count()).sum(),
        read: read_outcome,
        oracle: oracle_outcome,
    }
}

fn format_outcome(outcome: Outcome) -> String {
    match outcome {
        Outcome::Done(time) => format!("{time:.2?}"),
        Outcome::TimedOut(time) => format!(">{time:.1?} (timeout)"),
        Outcome::Skipped => "skipped".into(),
    }
}

fn speedup(row: &Row) -> String {
    match (row.read, row.oracle) {
        (Outcome::Done(read), Outcome::Done(oracle)) =>
            format!("{:.1}x", oracle.as_secs_f64() / read.as_secs_f64()),
        (Outcome::Done(read), Outcome::TimedOut(oracle)) =>
            format!(">={:.1}x", oracle.as_secs_f64() / read.as_secs_f64()),
        _ => "-".into(),
    }
}

fn results_path() -> PathBuf {
    env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target"))
        .join("bench-results/oracle_speedup.csv")
}

fn main() {
    let config = Config::from_env();
    match config.budget {
        Some(budget) => println!("budget per run: {budget:?}"),
        None => println!("budget per run: unlimited"),
    }
    println!(
        "{:<40} {:>8} {:>20} {:>20} {:>10}",
        "case", "atoms", "read", "oracle", "speedup"
    );
    let mut csv = String::from("case,atoms,read_s,oracle_s,oracle_timeout,speedup\n");
    let mut stopped: Vec<(String, bool, bool)> = Vec::new();
    for case in cases() {
        let name = case.name();
        if config.filter.as_ref().is_some_and(|filter| !name.contains(filter)) {
            continue;
        }
        // Fixture scenarios are independent; only size series escalate.
        let escalates = case.series != "corpus";
        let (skip_oracle, skip_read) = stopped
            .iter()
            .find(|(series, ..)| escalates && *series == case.series)
            .map(|&(_, oracle, read)| (oracle, read))
            .unwrap_or_default();
        let row = run_case(&case, &config, skip_oracle, skip_read);
        if escalates {
            let oracle = matches!(row.oracle, Outcome::TimedOut(_) | Outcome::Skipped);
            let read = matches!(row.read, Outcome::TimedOut(_) | Outcome::Skipped);
            if oracle || read {
                stopped.retain(|(series, ..)| *series != case.series);
                stopped.push((case.series.clone(), oracle, read));
            }
        }
        println!(
            "{:<40} {:>8} {:>20} {:>20} {:>10}",
            row.name,
            row.atoms,
            format_outcome(row.read),
            format_outcome(row.oracle),
            speedup(&row),
        );
        let secs = |outcome| match outcome {
            Outcome::Done(time) | Outcome::TimedOut(time) =>
                format!("{:.6}", Duration::as_secs_f64(&time)),
            Outcome::Skipped => String::new(),
        };
        writeln!(
            csv,
            "{},{},{},{},{},{}",
            row.name,
            row.atoms,
            secs(row.read),
            secs(row.oracle),
            matches!(row.oracle, Outcome::TimedOut(_)),
            speedup(&row),
        )
        .unwrap();
    }
    let path = results_path();
    fs::create_dir_all(path.parent().unwrap()).expect("results directory");
    fs::write(&path, csv).expect("results file");
    println!("results: {}", path.display());
}
