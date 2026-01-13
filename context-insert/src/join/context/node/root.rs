//! Root node join implementation.
//!
//! This module provides the entry point for root node joining.
//! The actual merge algorithm is in the `merge::root` module.

use crate::join::context::node::{
    context::NodeJoinCtx,
    merge::RootMergeCtx,
};
use context_trace::*;

/// Main entry point for root node joining.
///
/// Delegates to RootMergeCtx which reuses the intermediary merge algorithm
/// with protection of non-participating ranges.
pub fn join_root_partitions(ctx: &mut NodeJoinCtx) -> Token {
    let mut root_merge_ctx = RootMergeCtx::new(ctx);
    root_merge_ctx.merge_root()
}

