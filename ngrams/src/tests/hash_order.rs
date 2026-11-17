use super::TestCorpus;
use crate::{
    cancellation::Cancellation,
    graph::{
        labelling::{
            LabellingCtx,
            LabellingImage,
        },
        partitions::PartitionsCtx,
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
};
use context_trace::Hypergraph;
use std::collections::{
    BTreeMap,
    BTreeSet,
};

struct RootOrder<'a> {
    inner: PartitionsCtx<'a>,
    reverse: bool,
}

impl TraversalPass for RootOrder<'_> {
    type Node = NGramId;
    type NextNode = NGramId;
    type Queue = LinearQueue<Self>;

    fn ctx(&self) -> &LabellingCtx {
        self.inner.ctx
    }

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
        Ok(roots.into_iter().collect())
    }

    fn node_condition(
        &mut self,
        node: Self::Node,
    ) -> bool {
        self.inner.node_condition(node)
    }

    fn on_node(
        &mut self,
        node: &Self::Node,
    ) -> RunResult<Option<Vec<Self::NextNode>>> {
        self.inner.on_node(node)
    }

    fn finish_run(&mut self) -> RunResult<()> {
        self.inner.finish_run()
    }
}

fn canonical(graph: &Hypergraph) -> BTreeMap<String, BTreeSet<Vec<String>>> {
    let mut result = BTreeMap::new();
    for key in graph.vertex_keys() {
        let text = graph.vertex_key_string(&key);
        let patterns = graph
            .expect_child_patterns(key)
            .values()
            .map(|pattern| {
                pattern
                    .iter()
                    .map(|child| graph.index_string(*child))
                    .collect()
            })
            .collect();
        assert!(
            result.insert(text.clone(), patterns).is_none(),
            "duplicate token content: {text:?}"
        );
    }
    result
}

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
        pass.run().unwrap();
        canonical(&pass.inner.graph)
    };

    pretty_assertions::assert_eq!(
        run(false),
        run(true),
        "identical corpus and semantic labels must produce the same graph \
         for either root HashSet iteration order"
    );
}
