use context_trace::{
    graph::vertex::atom::{
        Atom,
        NewAtomIndex,
        NewAtomIndices,
    },
    *,
};

use itertools::Itertools;

use std::{
    fmt::Debug,
    io::Read,
    str::Chars,
};

use crate::request::RequestInput;

// ---------------------------------------------------------------------------
// ToNewAtomIndices trait (unchanged)
// ---------------------------------------------------------------------------

pub trait ToNewAtomIndices: Debug {
    fn to_new_atom_indices<G: HasGraph<Kind = BaseGraphKind>>(
        self,
        graph: &G,
    ) -> NewAtomIndices;
}

impl ToNewAtomIndices for NewAtomIndices {
    fn to_new_atom_indices<G: HasGraph<Kind = BaseGraphKind>>(
        self,
        _graph: &G,
    ) -> NewAtomIndices {
        self
    }
}
impl ToNewAtomIndices for Chars<'_> {
    fn to_new_atom_indices<G: HasGraph<Kind = BaseGraphKind>>(
        self,
        graph: &G,
    ) -> NewAtomIndices {
        graph.graph().new_atom_indices(self)
    }
}
impl ToNewAtomIndices for RequestInput {
    fn to_new_atom_indices<G: HasGraph<Kind = BaseGraphKind>>(
        self,
        graph: &G,
    ) -> NewAtomIndices {
        match self {
            RequestInput::Text(text) => text.chars().to_new_atom_indices(graph),
            RequestInput::Pattern(pattern) =>
                pattern.to_new_atom_indices(graph),
        }
    }
}

// ---------------------------------------------------------------------------
// LazyAtomIter — resolves each character to a NewAtomIndex on demand
// ---------------------------------------------------------------------------

/// Lazy atom resolution iterator.
///
/// Instead of eagerly collecting all characters into a `Vec<NewAtomIndex>`,
/// this iterator resolves each character to a `NewAtomIndex` **at consumption
/// time**. Unknown characters are inserted into the graph immediately when
/// first encountered.
///
/// This enables:
/// - Streaming input from `impl Read` sources (files, stdin, sockets)
/// - Bounded memory usage (no full-input materialization)
/// - Correct lazy classification (a character's known/unknown status is
///   determined when it is consumed, not when the input is first scanned)
pub(crate) struct LazyAtomIter<C: Iterator<Item = char>> {
    chars: C,
    graph: HypergraphRef,
}

impl<C: Iterator<Item = char>> LazyAtomIter<C> {
    /// Create a new lazy atom iterator from a character source and graph reference.
    pub(crate) fn new(
        chars: C,
        graph: HypergraphRef,
    ) -> Self {
        Self { chars, graph }
    }
}

impl<C: Iterator<Item = char>> Iterator for LazyAtomIter<C> {
    type Item = NewAtomIndex;

    fn next(&mut self) -> Option<NewAtomIndex> {
        self.chars.next().map(|ch| {
            let atom = Atom::Element(ch);
            match self.graph.get_atom_index(atom) {
                Ok(i) => NewAtomIndex::Known(i),
                Err(_) => {
                    let token = self.graph.insert_atom(atom);
                    NewAtomIndex::New(token.vertex_index())
                },
            }
        })
    }
}

// Manual Debug impl since we can't derive Debug for arbitrary iterators.
impl<C: Iterator<Item = char>> std::fmt::Debug for LazyAtomIter<C> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("LazyAtomIter")
            .field("graph", &"HypergraphRef{..}")
            .finish()
    }
}

// ---------------------------------------------------------------------------
// LazyAtomIter constructors
// ---------------------------------------------------------------------------

impl LazyAtomIter<std::str::Chars<'static>> {
    /// Create a `LazyAtomIter` from a `&'static str`.
    #[allow(dead_code)]
    pub(crate) fn from_static_str(
        s: &'static str,
        graph: HypergraphRef,
    ) -> Self {
        Self::new(s.chars(), graph)
    }
}

/// Create a `LazyAtomIter` from an `impl Read` byte source.
///
/// The reader is wrapped in a `BufReader` and decoded as UTF-8 character by
/// character. Invalid UTF-8 bytes are silently skipped (lossy decoding).
///
/// Returns a boxed iterator to erase the concrete type.
pub(crate) fn lazy_atoms_from_reader(
    reader: impl Read + 'static,
    graph: HypergraphRef,
) -> LazyAtomIter<Box<dyn Iterator<Item = char>>> {
    let buf_reader = std::io::BufReader::new(reader);
    // Read the entire content and decode as UTF-8, replacing invalid sequences.
    // We wrap in a char iterator that reads byte-by-byte through a buffered
    // UTF-8 decoder.
    let chars: Box<dyn Iterator<Item = char>> =
        Box::new(Utf8CharIter::new(buf_reader));
    LazyAtomIter::new(chars, graph)
}

// ---------------------------------------------------------------------------
// Utf8CharIter — streaming UTF-8 character decoder from impl Read
// ---------------------------------------------------------------------------

/// Streaming UTF-8 character decoder.
///
/// Reads bytes from an `impl Read` source and decodes them into `char` values
/// one at a time. Handles multi-byte UTF-8 sequences correctly. Invalid byte
/// sequences are replaced with U+FFFD (REPLACEMENT CHARACTER).
struct Utf8CharIter<R: Read> {
    reader: R,
    /// Small buffer for multi-byte UTF-8 decode (max 4 bytes per char).
    buf: [u8; 4],
}

impl<R: Read> Utf8CharIter<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            buf: [0u8; 4],
        }
    }
}

impl<R: Read> Iterator for Utf8CharIter<R> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        // Read the first byte
        if self.reader.read_exact(&mut self.buf[..1]).is_err() {
            return None; // EOF or I/O error
        }
        let first = self.buf[0];

        // Determine how many continuation bytes we need
        let len = if first < 0x80 {
            1
        } else if first < 0xC0 {
            // Unexpected continuation byte — replace
            return Some('\u{FFFD}');
        } else if first < 0xE0 {
            2
        } else if first < 0xF0 {
            3
        } else if first < 0xF8 {
            4
        } else {
            return Some('\u{FFFD}');
        };

        // Read continuation bytes if needed
        if len > 1 {
            if self.reader.read_exact(&mut self.buf[1..len]).is_err() {
                return Some('\u{FFFD}');
            }
        }

        // Decode
        match std::str::from_utf8(&self.buf[..len]) {
            Ok(s) => s.chars().next(),
            Err(_) => Some('\u{FFFD}'),
        }
    }
}

// ---------------------------------------------------------------------------
// Type-erased atom iterator (for use at ReadCtx boundary)
// ---------------------------------------------------------------------------

/// A type-erased atom iterator.
///
/// Used at the `ReadCtx` boundary to keep the public API simple.
/// The small runtime cost of vtable dispatch per atom is negligible
/// compared to graph operations.
#[allow(dead_code)]
pub(crate) type ErasedAtomIter = Box<dyn Iterator<Item = NewAtomIndex>>;

// ---------------------------------------------------------------------------
// NextSegment (unchanged)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct NextSegment {
    pub(crate) known: Pattern,
    pub(crate) unknown: Pattern,
}

// ---------------------------------------------------------------------------
// SegmentIter — generic over any Iterator<Item = NewAtomIndex>
// ---------------------------------------------------------------------------

/// Partitions an atom stream into alternating unknown/known segments.
///
/// The iterator consumes contiguous runs of same-kind atoms
/// (`is_new()` → unknown, `is_known()` → known) and yields them as
/// `NextSegment` values.
///
/// ## Generic parameter
///
/// `I` is the backing atom iterator. The **default type parameter**
/// preserves backward compatibility: all existing code that says
/// `SegmentIter` without a type parameter continues to work with the
/// eager `Vec`-based iterator.
///
/// New lazy sources produce `SegmentIter<LazyAtomIter<C>>` or use
/// `ErasedSegmentIter` for type-erased contexts.
pub(crate) struct SegmentIter<
    I: Iterator<Item = NewAtomIndex> = std::vec::IntoIter<NewAtomIndex>,
> {
    iter: std::iter::Peekable<I>,
}

/// Type-erased `SegmentIter` that uses `Box<dyn Iterator>` internally.
/// Used at the `ReadCtx` public boundary to avoid leaking type parameters.
pub(crate) type ErasedSegmentIter =
    SegmentIter<Box<dyn Iterator<Item = NewAtomIndex>>>;

// Manual Debug — we can't derive because `I` may not be Debug.
impl<I: Iterator<Item = NewAtomIndex>> std::fmt::Debug for SegmentIter<I> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("SegmentIter").finish_non_exhaustive()
    }
}

impl<I: Iterator<Item = NewAtomIndex>> Iterator for SegmentIter<I> {
    type Item = NextSegment;

    fn next(&mut self) -> Option<Self::Item> {
        let unknown = self.next_pattern_where(|t| t.is_new());
        let known = self.next_pattern_where(|t| t.is_known());
        if unknown.is_empty() && known.is_empty() {
            None
        } else {
            Some(NextSegment { unknown, known })
        }
    }
}

// --- Constructors -----------------------------------------------------------

impl SegmentIter<std::vec::IntoIter<NewAtomIndex>> {
    /// Create a `SegmentIter` from an eagerly-collected `NewAtomIndices` vec.
    ///
    /// This is the **original constructor** — kept for backward compatibility
    /// and used in tests for side-by-side comparison with the lazy path.
    #[allow(dead_code)]
    pub(crate) fn new(sequence: NewAtomIndices) -> Self {
        Self {
            iter: sequence.into_iter().peekable(),
        }
    }
}

impl<I: Iterator<Item = NewAtomIndex>> SegmentIter<I> {
    /// Create a `SegmentIter` from any `Iterator<Item = NewAtomIndex>`.
    pub(crate) fn from_iter(iter: I) -> Self {
        Self {
            iter: iter.peekable(),
        }
    }

    fn next_pattern_where(
        &mut self,
        f: impl FnMut(&NewAtomIndex) -> bool,
    ) -> Pattern {
        self.iter.peeking_take_while(f).map(Token::from).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a fresh empty graph.
    fn empty_graph() -> HypergraphRef {
        HypergraphRef::<BaseGraphKind>::default()
    }

    // --- LazyAtomIter tests ------------------------------------------------

    #[test]
    fn lazy_atom_iter_empty_input() {
        let graph = empty_graph();
        let mut iter = LazyAtomIter::new("".chars(), graph);
        assert!(iter.next().is_none());
    }

    #[test]
    fn lazy_atom_iter_all_new() {
        let graph = empty_graph();
        let items: Vec<_> =
            LazyAtomIter::new("abc".chars(), graph.clone()).collect();
        assert_eq!(items.len(), 3);
        assert!(items[0].is_new(), "first char should be New");
        assert!(items[1].is_new(), "second char should be New");
        assert!(items[2].is_new(), "third char should be New");

        // All three atoms should now exist in the graph
        assert!(graph.get_atom_index(Atom::Element('a')).is_ok());
        assert!(graph.get_atom_index(Atom::Element('b')).is_ok());
        assert!(graph.get_atom_index(Atom::Element('c')).is_ok());
    }

    #[test]
    fn lazy_atom_iter_interleaved_new_known() {
        let graph = empty_graph();

        // Pre-insert 'b' so it's known
        graph.insert_atom(Atom::Element('b'));

        let items: Vec<_> =
            LazyAtomIter::new("aba".chars(), graph.clone()).collect();
        assert_eq!(items.len(), 3);
        assert!(items[0].is_new(), "'a' first seen → New");
        assert!(items[1].is_known(), "'b' pre-existing → Known");
        // 'a' was created two steps ago by lazy resolution
        assert!(items[2].is_known(), "'a' now exists → Known");
    }

    #[test]
    fn lazy_atom_iter_repeated_char() {
        let graph = empty_graph();
        let items: Vec<_> =
            LazyAtomIter::new("aaa".chars(), graph.clone()).collect();
        assert_eq!(items.len(), 3);
        assert!(items[0].is_new(), "first 'a' → New");
        assert!(items[1].is_known(), "second 'a' → Known (created above)");
        assert!(items[2].is_known(), "third 'a' → Known");
    }

    #[test]
    fn lazy_matches_eager_resolution() {
        // Verify that lazy resolution produces the same NewAtomIndex sequence
        // as the eager `new_atom_indices` path.
        let input = "abcab";

        // Eager
        let eager_graph = empty_graph();
        let eager: Vec<_> = eager_graph.new_atom_indices(input.chars());

        // Lazy
        let lazy_graph = empty_graph();
        let lazy: Vec<_> =
            LazyAtomIter::new(input.chars(), lazy_graph.clone()).collect();

        assert_eq!(eager.len(), lazy.len());
        for (i, (e, l)) in eager.iter().zip(lazy.iter()).enumerate() {
            assert_eq!(
                e.is_new(),
                l.is_new(),
                "mismatch at position {i}: eager={e:?}, lazy={l:?}"
            );
            assert_eq!(
                e.is_known(),
                l.is_known(),
                "mismatch at position {i}: eager={e:?}, lazy={l:?}"
            );
        }
    }

    // --- SegmentIter generic tests -----------------------------------------

    #[test]
    fn segment_iter_from_lazy_matches_eager() {
        let input = "abcab";

        // Eager path
        let eager_graph = empty_graph();
        let eager_indices = eager_graph.new_atom_indices(input.chars());
        let eager_segments: Vec<_> = SegmentIter::new(eager_indices).collect();

        // Lazy path
        let lazy_graph = empty_graph();
        let lazy_atoms = LazyAtomIter::new(input.chars(), lazy_graph);
        let lazy_segments: Vec<_> =
            SegmentIter::from_iter(lazy_atoms).collect();

        assert_eq!(
            eager_segments.len(),
            lazy_segments.len(),
            "should produce same number of segments"
        );
        for (i, (e, l)) in
            eager_segments.iter().zip(lazy_segments.iter()).enumerate()
        {
            assert_eq!(
                e.unknown.len(),
                l.unknown.len(),
                "segment {i}: unknown length mismatch"
            );
            assert_eq!(
                e.known.len(),
                l.known.len(),
                "segment {i}: known length mismatch"
            );
        }
    }

    #[test]
    fn segment_iter_all_new_single_segment() {
        let graph = empty_graph();
        let atoms = LazyAtomIter::new("xyz".chars(), graph);
        let segments: Vec<_> = SegmentIter::from_iter(atoms).collect();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].unknown.len(), 3, "all 3 chars are new");
        assert_eq!(segments[0].known.len(), 0, "no known chars");
    }

    #[test]
    fn segment_iter_all_known_single_segment() {
        let graph = empty_graph();
        // Pre-insert all atoms
        graph.insert_atom(Atom::Element('a'));
        graph.insert_atom(Atom::Element('b'));

        let atoms = LazyAtomIter::new("ab".chars(), graph);
        let segments: Vec<_> = SegmentIter::from_iter(atoms).collect();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].unknown.len(), 0, "no unknown chars");
        assert_eq!(segments[0].known.len(), 2, "both chars are known");
    }

    #[test]
    fn segment_iter_empty_input() {
        let graph = empty_graph();
        let atoms = LazyAtomIter::new("".chars(), graph);
        let segments: Vec<_> = SegmentIter::from_iter(atoms).collect();
        assert_eq!(segments.len(), 0);
    }

    // --- from_reader tests -------------------------------------------------

    #[test]
    fn lazy_atoms_from_reader_basic() {
        use std::io::Cursor;
        let graph = empty_graph();
        let reader = Cursor::new(b"hello");
        let lazy = lazy_atoms_from_reader(reader, graph.clone());
        let items: Vec<_> = lazy.collect();
        assert_eq!(items.len(), 5);
        // 'h','e','l','l','o' — first 4 unique chars are New, second 'l' is Known
        assert!(items[0].is_new()); // 'h'
        assert!(items[1].is_new()); // 'e'
        assert!(items[2].is_new()); // 'l'
        assert!(items[3].is_known()); // 'l' (second occurrence)
        assert!(items[4].is_new()); // 'o'
    }

    #[test]
    fn lazy_atoms_from_reader_unicode() {
        use std::io::Cursor;
        let graph = empty_graph();
        let input = "héllo 🌍";
        let reader = Cursor::new(input.as_bytes());
        let lazy = lazy_atoms_from_reader(reader, graph.clone());
        let items: Vec<_> = lazy.collect();
        // "héllo 🌍" = h, é, l, l, o, ' ', 🌍 = 7 chars
        assert_eq!(items.len(), 7);
    }

    #[test]
    fn segment_iter_from_reader() {
        use std::io::Cursor;
        let graph = empty_graph();
        let reader = Cursor::new(b"abc");
        let lazy = lazy_atoms_from_reader(reader, graph);
        let segments: Vec<_> = SegmentIter::from_iter(lazy).collect();
        // All new chars on empty graph → 1 segment with 3 unknown, 0 known
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].unknown.len(), 3);
        assert_eq!(segments[0].known.len(), 0);
    }

    // --- ErasedSegmentIter tests -------------------------------------------

    #[test]
    fn erased_segment_iter_works() {
        let graph = empty_graph();
        let lazy = LazyAtomIter::new("ab".chars(), graph);
        let boxed: Box<dyn Iterator<Item = NewAtomIndex>> = Box::new(lazy);
        let segments: Vec<_> = SegmentIter::from_iter(boxed).collect();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].unknown.len(), 2);
    }
}
