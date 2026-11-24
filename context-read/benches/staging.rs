//! Staging-graph benchmarks (staging plan §7.1): reader throughput on the v3
//! corpus and on long periodic inputs, `find_ancestor` on committed fixture
//! graphs directly and through an empty overlay view, and the cost of an
//! overlay commit. S0 saves the pre-staging baseline:
//!
//! ```sh
//! cargo bench -p context-read --bench staging -- --save-baseline pre-staging
//! cargo bench -p context-read --bench staging -- --baseline pre-staging
//! ```

use std::time::{
    Duration,
    Instant,
};

use context_read::read;
use context_search::Find;
use context_trace::{
    StageIndex,
    Token,
    positioned::{
        GraphEdit,
        GraphRead,
        HasGraph,
        NodeRef,
        Occurrence,
        OverlayHandle,
        PositionedGraph,
        PositionedGraphRef,
        StageHandle,
        VertexRead,
        fixtures::{
            Scenario,
            fixture_graphs,
            fixtures_dir,
            manifest,
        },
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

/// Replay every vertex of `graph` (index order) into a stage over an empty
/// base: the stage of a read that produced `graph`.
fn stage_of<'g>(
    base: &'g PositionedGraphRef,
    graph: &PositionedGraph,
) -> (OverlayHandle<'g>, Token<StageIndex>) {
    let mut handle = OverlayHandle::open(base);
    let mut tokens: Vec<Token<StageIndex>> = Vec::with_capacity(graph.vertex_count());
    {
        let mut view = handle.stage();
        let mut edit = GraphEdit::new(&mut view);
        for vertex in graph.vertices() {
            let token = match vertex.node() {
                NodeRef::Atom(&atom) => edit.atom(atom),
                NodeRef::Composite(composition) => edit
                    .create(
                        composition
                            .occurrences()
                            .iter()
                            .map(|o| Occurrence::new(o.offset, tokens[o.child.index.0]))
                            .collect(),
                    )
                    .expect("replayed composition"),
            };
            tokens.push(token);
        }
        edit.finish();
    }
    let root = *tokens.iter().max_by_key(|token| token.width).expect("nonempty graph");
    (handle, root)
}

/// `StageHandle::commit` of the stage a periodic read builds (the read's
/// graph replayed into a stage), relative to `read/periodic`.
fn commit_overlay(c: &mut Criterion) {
    let mut group = c.benchmark_group("commit/overlay");
    group.sample_size(10);
    for n in PERIODIC {
        let input = format!("{}x", "abc".repeat(n));
        let graph = read_all(std::slice::from_ref(&input));
        let graph = graph.read().clone();
        group.throughput(Throughput::Elements(graph.vertex_count() as u64));
        group.bench_with_input(BenchmarkId::new("abc^n_x", n), &graph, |b, graph| {
            b.iter_custom(|iterations| {
                let mut total = Duration::ZERO;
                for _ in 0..iterations {
                    let base = PositionedGraphRef::default();
                    let (handle, root) = stage_of(&base, graph);
                    let start = Instant::now();
                    black_box(handle.commit(root).expect("unchanged base"));
                    total += start.elapsed();
                }
                total
            })
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
        .sample_size(30);
    targets = read_corpus, read_periodic, find_ancestor, find_ancestor_overlay, commit_overlay
}
criterion_main!(benches);
