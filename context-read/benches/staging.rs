//! Staging-graph benchmarks (staging plan §7.1), per staging strategy:
//! reader throughput on the v3 corpus and on long periodic inputs,
//! `find_ancestor` on committed fixture graphs directly and through empty
//! overlay and tagged views. Ids without a strategy suffix use the default
//! strategy (overlay, D-S1); `_tagged` ids are strategy (b). The commit
//! cost is measured by the `staging_commit` bench, peak memory by
//! `staging_memory`. S0 saves the pre-staging baseline:
//!
//! ```sh
//! cargo bench -p context-read --bench staging -- --save-baseline pre-staging
//! cargo bench -p context-read --bench staging -- --baseline pre-staging
//! ```
//!
//! The pre-staging reader has no strategies, and `--baseline` requires a
//! baseline for every id: the `pre-staging` data of the `_overlay` and
//! `_tagged` ids is a copy of the S0 measurement of the same workload (the
//! id without suffix; staging plan §11, S6).

use std::time::Duration;

use context_read::{
    OverlayStaging,
    StagingStrategy,
    TaggedStaging,
    read_with,
};
use context_search::Find;
use context_trace::positioned::{
    HasGraph,
    OverlayHandle,
    PositionedGraphRef,
    TaggedHandle,
    fixtures::{
        Scenario,
        fixture_graphs,
        fixtures_dir,
        manifest,
    },
};
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

fn read_all<S: StagingStrategy>(inputs: &[String]) -> PositionedGraphRef {
    let graph = PositionedGraphRef::default();
    for input in inputs {
        black_box(read_with::<S>(&graph, input.chars()));
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
    fn bench<S: StagingStrategy>(
        group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
        id: &str,
        scenarios: &[Scenario],
    ) {
        group.bench_function(id, |b| {
            b.iter(|| {
                for Scenario { inputs, .. } in scenarios {
                    black_box(read_all::<S>(inputs));
                }
            })
        });
    }
    bench::<OverlayStaging>(&mut group, "all", &scenarios);
    bench::<TaggedStaging>(&mut group, "all_tagged", &scenarios);
    group.finish();
}

fn read_periodic(c: &mut Criterion) {
    let mut group = c.benchmark_group("read/periodic");
    group.sample_size(10);
    for n in PERIODIC {
        let input = format!("{}x", "abc".repeat(n));
        group.throughput(Throughput::Elements(input.chars().count() as u64));
        group.bench_with_input(BenchmarkId::new("abc^n_x", n), &input, |b, input| {
            b.iter(|| read_all::<OverlayStaging>(std::slice::from_ref(input)))
        });
        group.bench_with_input(BenchmarkId::new("abc^n_x_tagged", n), &input, |b, input| {
            b.iter(|| read_all::<TaggedStaging>(std::slice::from_ref(input)))
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

/// The same search through an empty tagged view.
fn find_ancestor_tagged(c: &mut Criterion) {
    let fixtures = fixture_graphs();
    let graphs: Vec<PositionedGraphRef> = fixtures
        .iter()
        .map(|fixture| PositionedGraphRef::new(fixture.graph.clone()))
        .collect();
    let handles: Vec<TaggedHandle<'_>> = graphs.iter().map(TaggedHandle::open).collect();
    let views: Vec<_> = handles.iter().map(|handle| handle.graph()).collect();
    let mut group = c.benchmark_group("find_ancestor");
    group.throughput(Throughput::Elements(
        fixtures.iter().map(|fixture| fixture.corpus.len() as u64).sum(),
    ));
    group.bench_function("corpus_inputs_tagged", |b| {
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
    targets = read_corpus, read_periodic, find_ancestor, find_ancestor_overlay,
        find_ancestor_tagged
}
criterion_main!(benches);
