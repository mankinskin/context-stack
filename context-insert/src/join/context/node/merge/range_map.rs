//! Shared RangeMap implementation for partition merging.
//!
//! Used by both intermediary and root merge contexts to track merged partitions
//! by offset index range.

use std::{
    borrow::Borrow,
    collections::HashMap,
    ops::Range,
};

use derive_more::{Deref, DerefMut};
use context_trace::*;

/// RangeMap for tracking merged partitions by offset index range.
///
/// The range key represents offset indices (not atom positions).
/// For example, range `0..2` means the partition from offset 0 to offset 2.
#[derive(Debug, Default, Deref, DerefMut)]
pub struct RangeMap<R = Range<usize>> {
    #[deref]
    #[deref_mut]
    pub map: HashMap<R, Token>,
}

impl<C: Borrow<Token>, I: IntoIterator<Item = C>> From<I>
    for RangeMap<Range<usize>>
{
    fn from(iter: I) -> Self {
        let mut map = HashMap::default();
        for (i, part) in iter.into_iter().enumerate() {
            // Each initial partition occupies range i..(i+1)
            // This represents a single partition at index i
            map.insert(i..(i + 1), *part.borrow());
        }
        Self { map }
    }
}

impl RangeMap<Range<usize>> {
    /// Get all 2-way merge combinations for a range.
    ///
    /// Iterates over interior split points to generate all possible binary splits.
    /// For example, range `0..3` produces splits at points 1 and 2:
    /// - `(0..1) + (1..3)`
    /// - `(0..2) + (2..3)`
    pub fn range_sub_merges(
        &self,
        range: Range<usize>,
    ) -> impl IntoIterator<Item = Pattern> + '_ {
        let (start, end) = (range.start, range.end);
        // Iterate interior split points only (start+1..end)
        // For range 0..3, this gives [1, 2] producing splits:
        // - (0..1) + (1..3)
        // - (0..2) + (2..3)
        // For single-partition ranges like 0..1, this gives [] (empty)
        (start + 1..end).map(move |ri| {
            let &left = self.map.get(&(start..ri)).unwrap();
            let &right = self.map.get(&(ri..end)).unwrap();
            Pattern::from(vec![left, right])
        })
    }
}
