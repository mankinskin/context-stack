//! Staging-graph benchmarks (staging plan §7.1): reader throughput on the
//! v3 corpus and on long periodic inputs, `find_ancestor` on committed
//! fixture graphs directly and through an empty overlay view. The commit
//! cost is measured by the `staging_commit` bench, peak memory by
//! `staging_memory`.
//!
//! ```sh
//! cargo bench -p context-read --bench staging --no-default-features --features test-api
//! ```

use std::time::Duration;

use context_read::read;
use context_search::Find;
use context_trace::{
    positioned::{
        fixtures::{
            fixture_graphs,
            fixtures_dir,
            manifest,
            Scenario,
        },
        HasGraph,
        PositionedGraphRef,
    },
};
use context_read::stage::overlay::OverlayHandle;
use criterion::{
    BenchmarkId,
    Criterion,
    Throughput,
    black_box,
    criterion_group,
    criterion_main,
};

/// Read time grows roughly cubically in `n`; 512 takes about 0.3 s.
const PERIODIC: [usize; 3] = [64, 256, 512];

fn read_all(inputs: &[String]) -> PositionedGraphRef {
    let graph = PositionedGraphRef::default();
    for input in inputs {
        black_box(read(&graph, input.chars()));
    }
    graph
}

fn atoms<'a>(inputs: impl IntoIterator<Item = &'a String>) -> u64 {
    inputs.into_iter().map(|input| input.chars().count() as u64).sum()
}

fn read_corpus(c: &mut Criterion) {
    let scenarios = manifest(&fixtures_dir());
    let mut group = c.benchmark_group("read/corpus");
    group.throughput(Throughput::Elements(atoms(
        scenarios.iter().flat_map(|scenario| &scenario.inputs),
    )));
    group.bench_function("all", |b| {
        b.iter(|| {
            for Scenario { inputs, .. } in &scenarios {
                black_box(read_all(inputs));
            }
        })
    });
    group.finish();
}

fn read_periodic(c: &mut Criterion) {
    let mut group = c.benchmark_group("read/periodic");
    group.sample_size(10);
    for n in PERIODIC {
        let input = format!("{}x", "abc".repeat(n));
        group.throughput(Throughput::Elements(input.chars().count() as u64));
        group.bench_with_input(BenchmarkId::new("abc^n_x", n), &input, |b, input| {
            b.iter(|| read_all(std::slice::from_ref(input)))
        });
    }
    group.finish();
}

fn find_ancestor(c: &mut Criterion) {
    let fixtures = fixture_graphs();
    let mut group = c.benchmark_group("find_ancestor");
    group.throughput(Throughput::Elements(
        fixtures.iter().map(|fixture| fixture.corpus.len() as u64).sum(),
    ));
    group.bench_function("corpus_inputs", |b| {
        b.iter(|| {
            for fixture in &fixtures {
                for input in &fixture.corpus {
                    let _ = black_box(fixture.graph.find_ancestor(input.chars()));
                }
            }
        })
    });
    group.finish();
}

/// The same search on every committed fixture graph through an empty
/// overlay view (cost of the indirection alone).
fn find_ancestor_overlay(c: &mut Criterion) {
    let fixtures = fixture_graphs();
    let graphs: Vec<PositionedGraphRef> = fixtures
        .iter()
        .map(|fixture| PositionedGraphRef::new(fixture.graph.clone()))
        .collect();
    let handles: Vec<OverlayHandle<'_>> = graphs.iter().map(OverlayHandle::open).collect();
    let views: Vec<_> = handles.iter().map(|handle| handle.graph()).collect();
    let mut group = c.benchmark_group("find_ancestor");
    group.throughput(Throughput::Elements(
        fixtures.iter().map(|fixture| fixture.corpus.len() as u64).sum(),
    ));
    group.bench_function("corpus_inputs_overlay", |b| {
        b.iter(|| {
            for (fixture, view) in fixtures.iter().zip(&views) {
                for input in &fixture.corpus {
                    let _ = black_box(view.find_ancestor(input.chars()));
                }
            }
        })
    });
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
        .sample_size(30);
    targets = read_corpus, read_periodic, find_ancestor, find_ancestor_overlay
}
criterion_main!(benches);
