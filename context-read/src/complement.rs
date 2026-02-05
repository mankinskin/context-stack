use context_insert::*;
use context_trace::*;
use derive_new::new;

use crate::expansion::link::ExpansionLink;

use crate::context::ReadCtx;

#[derive(Debug, new)]
pub struct ComplementBuilder {
    link: ExpansionLink,
}

impl ComplementBuilder {
    pub fn build(
        self,
        trav: &mut ReadCtx,
    ) -> Token {
        // Get the root index from the postfix path
        use context_trace::GraphRootChild;
        let root = self.link.root_postfix.graph_root_child(trav);

        // Calculate the complement end bound from the postfix path
        use context_trace::HasRootChildIndex;
        let intersection_start = self.link.root_postfix.root_child_index();

        if intersection_start == 0 {
            // If intersection is at the beginning, no complement exists
            return root;
        }

        // Build the trace cache for the complement path
        let complement_cache =
            self.build_complement_trace_cache(trav, root, intersection_start);

        // Create InitInterval for the complement range
        let init_interval = InitInterval {
            root,
            cache: complement_cache,
            end_bound: intersection_start.into(),
        };
        // Safe to expect since we checked intersection_start != 0 above
        trav.insert_init((), init_interval)
            .expect("complement insert_init should succeed with non-zero end_bound")
    }

    fn build_complement_trace_cache(
        &self,
        _trav: &ReadCtx,
        root: Token,
        _end_bound: usize,
    ) -> TraceCache {
        // Initialize cache with the root
        // The complement range is from the start to the intersection point
        // We use checkpointing approach: build a trace cache up to end_bound
        TraceCache::new(root)

        // TODO: Use search/checkpoint API to build trace cache up to end_bound
        // For now, return minimal cache - this may need expansion based on
        // how complement tracing should work with checkpointing
    }
}

// back context
// what is the back context?
// The back context is the complement of the next expansion
// in the current root index.
// It is the part of the root index that is not covered by the next expansion.
// It is used to create a new band that will be appended to the chain.
// The back context is used to create a new band that will be appended to the chain
// and to create a new expansion link that will be used to link the new band
// to the previous band in the chain.
//
// what is the expansion link?
// The expansion link is the link between the new band and the previous band in the chain
// It is used to link the new band to the previous band in the chain.
// It contains the prefix path, the expansion and the start bound.
// The prefix path is the path from the start of the root index to the start of
// the next expansion.
//
// what is a band?
// A band is a collection of indices that are adjacent to each other. It has a pattern,
// a start bound and an end bound.
//
// what is a band chain?
// A band chain is a collection of bands that are ordered by their end bound.
// It is used to keep track of the bands that have been created so far and to finally
// create a final index that contains all the bands in the chain.
//
// Adding an expansion to the chain:
// 1.
