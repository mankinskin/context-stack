//! Regression isolating partition scheduling from earlier label selection.
//!
//! Both runs start from one cloned semantic state. With two roots, sorting and
//! reversing exercises every root permutation without relying on randomized hashes.

use std::path::PathBuf;

use context_trace::logging::build_capture_dispatch;

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
        Corpus,
        StatusHandle,
    },
    output::GraphData,
};

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

/// Require structurally valid, content-keyed graphs to match for both root orders.
///
/// Currently exposes the known late-label discovery defect. Equality ignores
/// graph IDs and pattern ordering but preserves child order within each pattern.
#[test]
fn partitions_are_independent_of_root_hash_iteration_order() {
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
