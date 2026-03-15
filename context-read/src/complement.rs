use context_insert::*;
use context_trace::*;
use derive_new::new;

use crate::expansion::link::ExpansionLink;

#[derive(Debug, new)]
pub(crate) struct ComplementBuilder {
    link: ExpansionLink,
}

impl ComplementBuilder {
    pub(crate) fn build(
        self,
        graph: &HypergraphRef,
    ) -> Token {
        use context_trace::GraphRootChild;
        use tracing::debug;
        let root = self.link.root_postfix.graph_root_child(graph);

        use context_trace::HasRootChildIndex;
        let intersection_start = self.link.root_postfix.root_child_index();

        debug!(
            root = ?root,
            intersection_start = ?intersection_start,
            "ComplementBuilder::build"
        );

        if intersection_start == 0 {
            debug!("No complement needed (intersection at start)");
            return root;
        }

        let complement_cache =
            self.build_trace_cache_stub(root, intersection_start);

        let init_interval = InitInterval {
            root,
            cache: complement_cache,
            end_bound: intersection_start.into(),
        };
        let complement = graph.insert_init((), init_interval).expect(
            "complement insert_init should succeed with non-zero end_bound",
        );
        debug!(complement = ?complement, "Complement built");
        complement
    }

    // DESIGN SESSION REQUIRED: see agents/designs/20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md
    fn build_trace_cache_stub(
        &self,
        root: Token,
        _end_bound: usize,
    ) -> TraceCache {
        TraceCache::new(root)
    }
}
