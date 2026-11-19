//! Order-independence regression for `["subdivision", "visualization"]`.
//!
//! The v3 checks hold selection fixed while reversing composition tie order,
//! then reverse the corpus inputs through the full pipeline. The legacy
//! partition reproducer is kept, ignored, for diagnostics only: the legacy
//! pipeline is known to be order-dependent.

use std::{
    collections::BTreeSet,
    path::PathBuf,
};

use context_trace::legacy::logging::build_capture_dispatch;

use super::TestCorpus;
use crate::{
    cancellation::Cancellation,
    graph::{
        labelling::{
            LabellingCtx,
            LabellingImage,
        },
        partitions::PartitionsCtx,
        trace,
        traversal::{
            pass::{
                RunResult,
                TraversalPass,
            },
            queue::{
                LinearQueue,
                Queue,
            },
        },
        vocabulary::{
            NGramId,
            Vocabulary,
        },
        composition::{
            compose_with_order,
            TieOrder,
        },
        parse_corpus,
        Corpus,
        StatusHandle,
    },
    output::legacy::GraphData,
};

const TEXTS: [&str; 2] = ["subdivision", "visualization"];

/// Fixed selection: reversing same-width processing leaves covers unchanged.
#[test]
fn composition_is_independent_of_tie_order() {
    let ctx = LabellingCtx::select_frequency_fresh(
        Corpus::new("root_hash_order", TEXTS),
        StatusHandle::default(),
        Cancellation::None,
    )
    .unwrap();
    let run = |order| {
        compose_with_order(
            ctx.vocab(),
            ctx.labels(),
            &ctx.status,
            &ctx.cancellation,
            order,
        )
        .unwrap()
        .by_content(ctx.vocab())
    };
    let forward = run(TieOrder::Forward);
    let reversed = run(TieOrder::Reversed);
    assert_eq!(
        forward.keys().collect::<Vec<_>>(),
        reversed.keys().collect::<Vec<_>>()
    );
    pretty_assertions::assert_eq!(forward, reversed);
}

/// Full pipeline: reversed corpus inputs give the same retained set and covers.
#[test]
fn parse_corpus_is_independent_of_input_order() {
    let parse = |texts: [&str; 2]| {
        parse_corpus(
            Corpus::new("root_hash_order", texts),
            StatusHandle::default(),
            Cancellation::None,
        )
        .unwrap()
        .graph
    };
    let forward = parse(TEXTS);
    let mut reversed_texts = TEXTS;
    reversed_texts.reverse();
    let reversed = parse(reversed_texts);
    let retained = |g: &crate::output::PositionedGraphData| {
        g.vertices.keys().cloned().collect::<BTreeSet<_>>()
    };
    pretty_assertions::assert_eq!(retained(&forward), retained(&reversed));
    pretty_assertions::assert_eq!(forward, reversed);
    assert_eq!(
        forward.vertices["vis"],
        [(0, "v"), (1, "i"), (2, "s")].map(|(o, c)| (o, c.to_string()))
    );
    assert!(!forward.vertices.contains_key("vi"));
}

/// Partition-pass adapter that changes only the initial root processing order.
struct RootOrder<'a> {
    /// Real implementation whose admission, processing, and completion hooks run.
    inner: PartitionsCtx<'a>,
    /// Whether to reverse lexicographically sorted root content.
    reverse: bool,
}

impl TraversalPass for RootOrder<'_> {
    type Node = NGramId;
    type NextNode = NGramId;
    type Queue = LinearQueue<Self>;

    /// Use the real pass's context so cancellation and labels are unchanged.
    fn ctx(&self) -> &LabellingCtx {
        self.inner.ctx
    }

    /// Preserve root initialization side effects, then replace only queue order.
    fn start_queue(&mut self) -> RunResult<Self::Queue> {
        let mut queue = self.inner.start_queue()?;
        let mut roots: Vec<_> =
            std::iter::from_fn(|| queue.pop_front()).collect();
        // Exercise both possible root HashSet orders without random retries.
        roots.sort_by_key(|root| {
            self.ctx().vocab().containment.vertex_key_string(&root.key)
        });
        if self.reverse {
            roots.reverse();
        }
        trace::scheduled_roots(self.ctx().vocab(), &roots);
        Ok(roots.into_iter().collect())
    }

    /// Preserve the real partition pass's eligibility and visit tracking.
    fn node_condition(
        &mut self,
        node: Self::Node,
    ) -> bool {
        self.inner.node_condition(node)
    }

    /// Delegate all decomposition and gap-discovery behavior unchanged.
    fn on_node(
        &mut self,
        node: &Self::Node,
    ) -> RunResult<Option<Vec<Self::NextNode>>> {
        self.inner.on_node(node)
    }

    /// Preserve normal partition completion and status changes.
    fn finish_run(&mut self) -> RunResult<()> {
        self.inner.finish_run()
    }
}

fn with_partition_trace<T>(reverse: bool, run: impl FnOnce() -> T) -> T {
    let Some(dir) = std::env::var_os("NGRAMS_TRACE_DIR") else {
        return run();
    };
    let file = if reverse {
        "visualization-first.jsonl"
    } else {
        "subdivision-first.jsonl"
    };
    let path = PathBuf::from(dir).join(file);
    let capture = build_capture_dispatch(&path, "ngrams=trace")
        .unwrap_or_else(|err| {
            panic!("could not open trace {}: {err}", path.display())
        });
    let result = tracing::dispatcher::with_default(&capture.dispatch, run);
    eprintln!("Partition trace: {}", path.display());
    result
}

/// Legacy reproducer: require row graphs to match for both root orders.
///
/// Exposes the known late-label discovery defect of the legacy partition pass.
/// Equality ignores graph IDs and pattern ordering but preserves child order
/// within each pattern.
#[test]
#[ignore = "legacy, known order-dependent"]
fn legacy_partitions_are_independent_of_root_hash_iteration_order() {
    let texts = ["subdivision", "visualization"];
    let corpus = Corpus::new("root_hash_order", texts);
    let mut status = StatusHandle::default();
    // Bypass caches so both schedules start from this exact semantic selection.
    let image = LabellingImage::from(
        Vocabulary::from_corpus(&corpus, &mut status).unwrap(),
    );
    let mut ctx = LabellingCtx::new(
        TestCorpus::new(image, corpus),
        status,
        Cancellation::None,
    );
    ctx.label_freq().unwrap();
    ctx.label_wrap().unwrap();

    let run = |reverse| {
        // Isolate mutations: neither partition schedule may influence the other.
        let image = LabellingImage {
            vocab: ctx.vocab().clone(),
            labels: ctx.labels().clone(),
        };
        let mut ctx = LabellingCtx::new(
            TestCorpus::new(image, Corpus::new("root_hash_order", texts)),
            StatusHandle::default(),
            Cancellation::None,
        );
        let mut pass = RootOrder {
            inner: PartitionsCtx::from(&mut ctx),
            reverse,
        };
        with_partition_trace(reverse, || pass.run()).unwrap();
        GraphData::from_graph(&pass.inner.graph)
            .expect("partition graph must be structurally valid")
    };

    pretty_assertions::assert_eq!(
        run(false),
        run(true),
        "identical corpus and semantic labels must produce the same graph \
         for either root HashSet iteration order"
    );
}
