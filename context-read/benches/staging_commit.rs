//! Commit cost (staging plan §7.1): only
//! `StageHandle::commit` of a stage holding the graph a read of `abc^n x`
//! builds, replayed vertex by vertex into a stage over an empty base.
//!
//! ```sh
//! cargo bench -p context-read --bench staging_commit --no-default-features --features test-api
//! ```

use std::time::{
    Duration,
    Instant,
};

use context_read::read;
use context_trace::{
    StageIndex,
    Token,
    positioned::{
        GraphEdit,
        GraphRead,
        NodeRef,
        Occurrence,
        OverlayHandle,
        PositionedGraph,
        PositionedGraphRef,
        StageHandle,
        VertexRead,
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

const PERIODIC: [usize; 3] = [64, 256, 512];

/// Replay every vertex of `graph` (index order) into a stage over an empty
/// base: the stage of a read that produced `graph`.
fn stage_of<'g>(
    mut handle: OverlayHandle<'g>,
    graph: &PositionedGraph,
) -> (OverlayHandle<'g>, Token<StageIndex>) {
    // Atoms are committed immediately (dissolution plan §4.2).
    let mut tokens: Vec<Option<Token<StageIndex>>> = graph
        .vertices()
        .map(|vertex| match vertex.node() {
            NodeRef::Atom(&atom) => Some(handle.atom(atom).expect("unchanged base")),
            NodeRef::Composite(_) => None,
        })
        .collect();
    {
        let mut view = handle.stage();
        let mut edit = GraphEdit::new(&mut view);
        for (i, vertex) in graph.vertices().enumerate() {
            if let NodeRef::Composite(composition) = vertex.node() {
                let token = edit
                    .create(
                        composition
                            .occurrences()
                            .iter()
                            .map(|o| {
                                Occurrence::new(
                                    o.offset,
                                    tokens[o.child.index.0].expect("child replayed first"),
                                )
                            })
                            .collect(),
                    )
                    .expect("replayed composition");
                tokens[i] = Some(token);
            }
        }
        edit.finish();
    }
    let root = tokens
        .iter()
        .flatten()
        .copied()
        .max_by_key(|token| token.width)
        .expect("nonempty graph");
    (handle, root)
}

/// `StageHandle::commit` of the stage a periodic read builds (the read's
/// graph replayed into a stage), relative to `read/periodic` of the
/// `staging` bench.
fn commit(c: &mut Criterion) {
    let mut group = c.benchmark_group("commit/overlay");
    group.sample_size(10);
    for n in PERIODIC {
        let input = format!("{}x", "abc".repeat(n));
        let graph = PositionedGraphRef::default();
        read(&graph, input.chars());
        let graph = graph.read().clone();
        group.throughput(Throughput::Elements(graph.vertex_count() as u64));
        group.bench_with_input(BenchmarkId::new("abc^n_x", n), &graph, |b, graph| {
            b.iter_custom(|iterations| {
                let mut total = Duration::ZERO;
                for _ in 0..iterations {
                    let base = PositionedGraphRef::default();
                    let (handle, root) = stage_of(OverlayHandle::open(&base), graph);
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
    targets = commit
}
criterion_main!(benches);
