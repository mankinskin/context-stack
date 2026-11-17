//! Greedily pack positioned children into alternative, gap-filled token lines.
//!
//! Tokens here still use containment-graph indices. Partition construction
//! translates them into output-graph indices after the lines are complete.

use crate::graph::{
    partitions::{
        container::PartitionCell,
        NodePartitionCtx,
    },
    vocabulary::NGramId,
};
use context_trace::{
    graph::vertex::{
        data::VertexData,
        has_vertex_index::HasVertexIndex,
        has_vertex_key::HasVertexKey,
        pattern::Pattern,
        token::Token,
        wide::Wide,
        ChildPatterns,
        VertexIndex,
    },
    VertexSet,
};
use derive_more::{
    Deref,
    DerefMut,
};
use itertools::Itertools;
use ngram::NGram;
use std::{
    cmp::{
        Ordering,
        Reverse,
    },
    num::NonZeroUsize,
};

/// One non-overlapping decomposition under construction.
#[derive(Debug, Deref, DerefMut)]
pub(crate) struct PartitionLineBuilder<'a, 'b> {
    /// Start offset of the last cell, or zero when empty; not the line's end.
    pos: usize,
    /// Root and vocabulary used to resolve gap substrings.
    ctx: &'a NodePartitionCtx<'a, 'b>,
    /// Ordered cells, including gaps materialized as ordinary tokens.
    #[deref]
    #[deref_mut]
    line: Vec<Token>,
}
impl<'a, 'b> PartitionLineBuilder<'a, 'b> {
    /// Create an empty line starting at the root's beginning.
    pub(crate) fn new(ctx: &'a NodePartitionCtx<'a, 'b>) -> Self {
        Self {
            ctx,
            pos: Default::default(),
            line: Default::default(),
        }
    }
    /// Append a cell and advance the cached last-cell start by the previous width.
    fn push_cell(
        &mut self,
        cell: Token,
    ) {
        if let Some(last) = self.line.last() {
            self.pos += last.width().0;
        }
        self.line.push(cell);
    }
    /// Append a token contiguously at the current end.
    pub(crate) fn append(
        &mut self,
        index: Token,
    ) {
        self.push_cell(index);
    }
    /// Fill a nonempty gap with the root substring covering that interval.
    ///
    /// The interval must lie within the root and resolve in the containment graph.
    pub(crate) fn skip(
        &mut self,
        offset: NonZeroUsize,
    ) {
        let end_pos = self.end_pos();
        // Gap tokens need not already be labelled; the outer pass discovers them.
        let root_data = self
            .ctx
            .vocab()
            .containment
            .expect_vertex_data(self.ctx.root.vertex_key());
        let root_token = root_data.to_token();
        let index = self
            .ctx
            .vocab()
            .containment
            .get_vertex_subrange(root_token, end_pos..(end_pos + offset.get()));
        self.push_cell(index);
    }
    /// Insert an optional gap before appending the supplied child.
    pub(crate) fn append_after_offset(
        &mut self,
        offset: Option<NonZeroUsize>,
        index: Token,
    ) {
        if let Some(non_zero) = offset {
            self.skip(non_zero);
        }
        self.append(index);
    }
    /// Return the positive distance to a position at or beyond this line's end.
    ///
    /// Callers must not pass a position before the current end.
    pub(crate) fn offset_to(
        &self,
        pos: usize,
    ) -> Option<NonZeroUsize> {
        NonZeroUsize::new(pos - self.end_pos())
    }
    /// Extend the line to a target end with a gap token if necessary.
    pub(crate) fn skip_to(
        &mut self,
        end_pos: usize,
    ) {
        if let Some(o) = self.offset_to(end_pos) {
            self.skip(o);
        }
    }
    /// Exclusive end offset of all appended cells.
    pub(crate) fn end_pos(&self) -> usize {
        self.pos
            + self
                .line
                .last()
                .map(|cell| cell.width().0)
                .unwrap_or_default()
    }
    /// Pad to the supplied end and return the completed token sequence.
    pub(crate) fn close(
        mut self,
        end_pos: usize,
    ) -> Vec<Token> {
        self.skip_to(end_pos);
        self.line
    }
}

/// Index of the most recently created or extended decomposition line.
#[derive(Debug, Clone, Copy, Hash)]
struct PartitionCursor {
    /// Index into the enclosing builder's wall.
    pub(crate) line: usize,
}
impl PartitionCursor {
    /// Borrow the selected line, panicking if the cursor is invalid.
    pub(crate) fn get_current_line<'a, 'b>(
        &self,
        builder: &'a mut PartitionBuilder<'a, 'b>,
    ) -> &'a mut PartitionLineBuilder<'a, 'b> {
        builder.get_line_mut(self.line)
    }
}

/// Alternative lines assembled from children supplied in increasing start order.
#[derive(Debug)]
pub(crate) struct PartitionBuilder<'a, 'b> {
    /// Root against which every line and gap is positioned.
    ctx: &'a NodePartitionCtx<'a, 'b>,
    /// Last modified line, also used to choose the final padding extent.
    cursor: PartitionCursor,
    /// Non-overlapping lines; overlaps across different lines are allowed.
    wall: Vec<PartitionLineBuilder<'a, 'b>>,
}

impl<'a, 'b> PartitionBuilder<'a, 'b> {
    /// Initialize the wall by placing its first child.
    pub(crate) fn new(
        ctx: &'a NodePartitionCtx<'a, 'b>,
        offset: usize,
        first: NGramId,
    ) -> Self {
        let mut builder = Self {
            ctx,
            cursor: PartitionCursor { line: 0 },
            wall: Default::default(),
        };
        let index = ctx
            .vocab()
            .containment
            .expect_index_for_key(&first.vertex_key());
        builder.append_child(offset, Token::new(index, first.width()));
        builder
    }
    /// Add a new line with any leading gap and select it as current.
    pub(crate) fn create_line(
        &mut self,
        pos: usize,
        index: Token,
    ) {
        let mut line = PartitionLineBuilder::new(self.ctx);
        line.append_after_offset(NonZeroUsize::new(pos), index);
        self.cursor.line = self.wall.len();
        self.wall.push(line);
    }
    /// Add and select a line containing a child at the requested position.
    ///
    /// Currently performs the same operations as `create_line`.
    pub(crate) fn create_and_append_line(
        &mut self,
        pos: usize,
        index: Token,
    ) {
        let mut line = PartitionLineBuilder::new(self.ctx);
        line.append_after_offset(NonZeroUsize::new(pos), index);
        self.cursor.line = self.wall.len();
        self.wall.push(line);
    }
    /// Borrow a line by index, panicking on an invalid cursor.
    pub(crate) fn get_line_mut(
        &mut self,
        index: usize,
    ) -> &mut PartitionLineBuilder<'a, 'b> {
        self.wall.get_mut(index).expect("Invalid PartitionCursor")
    }
    /// Borrow the most recently modified line.
    pub(crate) fn get_current_line(
        &mut self
    ) -> &mut PartitionLineBuilder<'a, 'b> {
        self.get_line_mut(self.cursor.line)
    }
    /// Fill any gap before a child, append it to a line, and update the cursor.
    ///
    /// `pos` must be at or beyond the selected line's end.
    pub(crate) fn append_at_line(
        &mut self,
        line_index: usize,
        pos: usize,
        index: Token,
    ) {
        let line = self.get_line_mut(line_index);
        line.append_after_offset(line.offset_to(pos), index);
        self.cursor.line = line_index;
    }
    /// Greedily append to a compatible line, creating a new one for overlaps.
    ///
    /// Existing lines are tried by descending last-cell start, not by end offset.
    /// This packing does not enumerate every possible segmentation.
    pub(crate) fn append_child(
        &mut self,
        pos: usize,
        index: Token,
    ) {
        //println!("Find line to insert {}..{}", pos, pos + index.width());
        let mut sorted_lines = self
            .wall
            .iter()
            .enumerate()
            .map(|(i, lb)| (lb.pos, i))
            .sorted_by_key(|&(pos, _)| Reverse(pos))
            .map(|(_, i)| i)
            .collect_vec()
            .into_iter();

        sorted_lines
            .find(|line_index| {
                let line = self.get_line_mut(*line_index);

                let index_width = index.width();
                let end_pos = line.end_pos();
                //println!("{}", end_pos);

                match end_pos.cmp(&pos) {
                    Ordering::Equal | Ordering::Less => true,
                    Ordering::Greater => false,
                }
            })
            .map(|line_index| {
                //println!("Append {}", pos);
                self.append_at_line(line_index, pos, index)
            })
            .unwrap_or_else(|| {
                //println!("Create new {}", pos);
                self.create_and_append_line(pos, index)
            })
    }
    /// Pad all lines to the current line's end and consume the wall.
    ///
    /// Assumes that end is no earlier than every other line's end. It is not
    /// independently replaced with the root width or the maximum line end.
    pub(crate) fn close(mut self) -> Vec<Vec<Token>> {
        let end_pos = self.get_current_line().end_pos();
        self.wall
            .into_iter()
            .map(|line| line.close(end_pos))
            .collect()
    }
}
