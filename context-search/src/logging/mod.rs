//! Compact formatting implementations for context-search types

mod cursor_format;

use crate::{
    compare::{
        parent::ParentCompareState,
        state::{
            CompareEndResult,
            CompareLeafResult,
            CompareState,
            PathPairMode,
        },
    },
    cursor::CursorState,
    policy::SearchKind,
    r#match::{
        iterator::SearchIterator,
        SearchNode,
        SearchQueue,
    },
};
use context_trace::*;
use std::fmt;

impl CompactFormat for ParentCompareState {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        let cursor_pos: usize = self.cursor.current().atom_position.into();
        let vertex = self.parent_state.path.root_parent();
        write!(f, "ParentCandidate(vertex:{}, pos:{})", vertex, cursor_pos)
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        let vertex = self.parent_state.path.root_parent();

        write_indent(f, indent)?;
        writeln!(f, "ParentCompareState {{")?;
        write_indent(f, indent + 1)?;
        writeln!(f, "vertex: {},", vertex)?;
        write_indent(f, indent + 1)?;
        writeln!(f, "cursor:")?;
        self.cursor.fmt_indented(f, indent + 2)?;
        writeln!(f, ",")?;
        write_indent(f, indent + 1)?;
        writeln!(f, "parent_state:")?;
        self.parent_state.path.fmt_indented(f, indent + 2)?;
        writeln!(f)?;
        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

impl CompactFormat for SearchNode {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        match self {
            SearchNode::ParentCandidate(state) => state.fmt_compact(f),
            SearchNode::ChildCandidate(state) => state.fmt_compact(f),
        }
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        match self {
            SearchNode::ParentCandidate(state) => state.fmt_indented(f, indent),
            SearchNode::ChildCandidate(state) => state.fmt_indented(f, indent),
        }
    }
}

impl<Q, I, EndNode> CompactFormat for CompareState<Q, I, EndNode>
where
    Q: CursorState,
    I: CursorState,
    EndNode: PathNode,
{
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        let mode_str = match self.mode {
            PathPairMode::GraphMajor => "G",
            PathPairMode::QueryMajor => "Q",
        };

        let query_pos: usize = self.query.current().atom_position.into();
        //let index_pos: usize =
        // (*self.child.current().child_state.target_offset()).into();
        let checkpoint_pos: usize =
            self.query.checkpoint().atom_position.into();

        write!(
            f,
            "Compare(mode:{}, query@{}, checkpoint@{})",
            mode_str, query_pos, checkpoint_pos
        )
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        let query_state =
            std::any::type_name::<Q>().split("::").last().unwrap_or("?");
        let index_state =
            std::any::type_name::<I>().split("::").last().unwrap_or("?");

        write_indent(f, indent)?;
        writeln!(f, "CompareState<{}, {}> {{", query_state, index_state)?;

        write_indent(f, indent + 1)?;
        writeln!(f, "mode: {:?},", self.mode)?;

        write_indent(f, indent + 1)?;
        write!(f, "child: ")?;
        self.child
            .current()
            .child_state
            .fmt_indented(f, indent + 1)?;
        writeln!(f, ",")?;

        write_indent(f, indent + 1)?;
        write!(f, "query: ")?;
        self.query.current().fmt_indented(f, indent + 1)?;
        writeln!(f, ",")?;

        write_indent(f, indent + 1)?;
        write!(f, "query_checkpoint: ")?;
        self.query.checkpoint().fmt_indented(f, indent + 1)?;
        writeln!(f, ",")?;

        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

impl<EndNode: PathNode + CompactFormat> CompactFormat
    for CompareEndResult<EndNode>
{
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        match self {
            CompareEndResult::FoundMatch(state) => {
                write!(f, "Match(")?;
                state.fmt_compact(f)?;
                write!(f, ")")
            },
            CompareEndResult::Mismatch(state) => {
                write!(f, "Mismatch(")?;
                state.fmt_compact(f)?;
                write!(f, ")")
            },
        }
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        match self {
            CompareEndResult::FoundMatch(state) => {
                write_indent(f, indent)?;
                writeln!(f, "FoundMatch(")?;
                state.fmt_indented(f, indent + 1)?;
                writeln!(f)?;
                write_indent(f, indent)?;
                write!(f, ")")
            },
            CompareEndResult::Mismatch(state) => {
                write_indent(f, indent)?;
                writeln!(f, "Mismatch(")?;
                state.fmt_indented(f, indent + 1)?;
                writeln!(f)?;
                write_indent(f, indent)?;
                write!(f, ")")
            },
        }
    }
}
impl<EndNode: PathNode + CompactFormat> CompactFormat
    for CompareLeafResult<EndNode>
{
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        match self {
            CompareLeafResult::Finished(state) => {
                write!(f, "Finished(")?;
                state.fmt_compact(f)?;
                write!(f, ")")
            },
            CompareLeafResult::Prefixes(queue) => {
                write!(f, "Prefixes(")?;
                write!(f, "count: {}", queue.len())?;
                write!(f, ")")
            },
        }
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        match self {
            CompareLeafResult::Finished(state) => {
                write_indent(f, indent)?;
                writeln!(f, "Finished(")?;
                state.fmt_indented(f, indent + 1)?;
                writeln!(f)?;
                write_indent(f, indent)?;
                write!(f, ")")
            },
            CompareLeafResult::Prefixes(queue) => {
                write_indent(f, indent)?;
                writeln!(f, "Prefixes(")?;
                write_indent(f, indent + 1)?;
                write!(f, "count: {}", queue.len())?;
                writeln!(f)?;
                write_indent(f, indent)?;
                write!(f, ")")
            },
        }
    }
}
impl<K: SearchKind> CompactFormat for SearchIterator<K> {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        let queue_size = self.queue.nodes.len();
        let cache_size = self.trace_ctx.cache.entries.len();

        write!(
            f,
            "SearchIterator(cache:{}, queue:{})",
            cache_size, queue_size
        )
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        write_indent(f, indent)?;
        writeln!(f, "SearchIterator {{")?;

        // TraceCtx - show cache info
        write_indent(f, indent + 1)?;
        writeln!(f, "trace_ctx: TraceCtx {{")?;
        write_indent(f, indent + 2)?;
        writeln!(f, "cache_entries: {},", self.trace_ctx.cache.entries.len())?;
        write_indent(f, indent + 1)?;
        writeln!(f, "}},")?;

        // SearchQueue - show nodes
        write_indent(f, indent + 1)?;
        writeln!(f, "queue: SearchQueue {{")?;
        write_indent(f, indent + 2)?;
        writeln!(f, "queue_size: {},", self.queue.nodes.len())?;

        if !self.queue.nodes.is_empty() {
            write_indent(f, indent + 2)?;
            writeln!(f, "nodes: [")?;

            for (i, node) in self.queue.nodes.iter().enumerate() {
                write_indent(f, indent + 3)?;
                write!(f, "[{}] ", i)?;
                node.fmt_indented(f, indent + 3)?;
                writeln!(f, ",")?;
            }

            write_indent(f, indent + 2)?;
            writeln!(f, "],")?;
        }

        write_indent(f, indent + 1)?;
        writeln!(f, "}},")?;

        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

impl CompactFormat for SearchQueue {
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(f, "SearchQueue(nodes:{})", self.nodes.len())
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result {
        write_indent(f, indent)?;
        writeln!(f, "SearchQueue {{")?;

        write_indent(f, indent + 1)?;
        writeln!(f, "count: {},", self.nodes.len())?;

        if !self.nodes.is_empty() {
            write_indent(f, indent + 1)?;
            writeln!(f, "nodes: [")?;

            for (i, node) in self.nodes.iter().enumerate() {
                write_indent(f, indent + 2)?;
                write!(f, "[{}] ", i)?;
                node.fmt_compact(f)?;
                writeln!(f, ",")?;
            }

            write_indent(f, indent + 1)?;
            writeln!(f, "]")?;
        }

        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

// Implement Display for types to enable % formatting in tracing without Compact wrapper
impl_display_via_compact!(CompareEndResult<EndNode> where EndNode: PathNode + CompactFormat);
impl_display_via_compact!(CompareState<Q, I, EndNode> where Q: CursorState, I: CursorState, EndNode: PathNode);
impl_display_via_compact!(SearchIterator<K> where K: SearchKind);
impl_display_via_compact!(SearchQueue);
impl_display_via_compact!(ParentCompareState);
impl_display_via_compact!(SearchNode);
