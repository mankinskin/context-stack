//! The input's own vertex and its commits (reader plan §3.4, §3.5; legacy
//! `pipeline/root.rs`).
//!
//! The root is a vertex at all times. Every edit of the read goes into its
//! stage (staging plan §2.3). A staged, parentless composite root is
//! *editable* (the former "owned" root) and grows in place through
//! `GraphEdit::extend`; any other root (adopted, committed, an atom or with
//! parents) is never edited: a new root is created instead. The stage never
//! searches.

use context_trace::{
    StageIndex,
    Token,
    positioned::{
        Candidates,
        CompositionRead as _,
        GraphEdit,
        GraphRead,
        HasGraph,
        Occurrence,
        OverlayHandle,
        PositionedGraphRef,
        Publication,
        Settled,
        StageHandle,
        Translation,
        VertexRead,
        VertexState,
        maximal,
    },
};

/// One step of the reader, committed before the next search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Span {
    /// `t` starts at the end of the root.
    Append(Token<StageIndex>),
    /// `Y` is an existing token covering the root and the consumed input.
    ReplaceRoot(Token<StageIndex>),
    /// The root's postfix `postfix` extends to `expansion` (§3.2).
    Expand {
        postfix: Token<StageIndex>,
        expansion: Token<StageIndex>,
    },
}

/// How a commit changed the root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Commit {
    /// An existing token became the root (not editable).
    Adopt,
    /// The editable root grew in place (`GraphEdit::extend`).
    InPlace,
    /// A new editable root was created (`GraphEdit::create`).
    NewRoot,
}

/// The result of [`ReadStage::settle`]: the removals and the number of
/// vertices stabilized in the same edit.
#[derive(Debug, Default)]
pub(crate) struct Settlement {
    pub(crate) settled: Settled,
    pub(crate) stabilized: usize,
}

/// The stage of one read and the root of the input read so far.
pub(crate) struct ReadStage<'g> {
    pub(crate) handle: OverlayHandle<'g>,
    root: Option<Token<StageIndex>>,
}

impl<'g> ReadStage<'g> {
    pub(crate) fn open(graph: &'g PositionedGraphRef) -> Self {
        Self {
            handle: OverlayHandle::open(graph),
            root: None,
        }
    }

    pub(crate) fn token(&self) -> Option<Token<StageIndex>> {
        self.root
    }

    /// Input atoms consumed by `span` (beyond the root).
    pub(crate) fn consumed(
        &self,
        span: Span,
    ) -> usize {
        let root = self.root.map_or(0, |root| root.width.0);
        match span {
            Span::Append(token) => token.width.0,
            Span::ReplaceRoot(token) => token.width.0 - root,
            Span::Expand {
                postfix,
                expansion,
            } => expansion.width.0 - postfix.width.0,
        }
    }

    /// Staged, parentless and composite (staging plan §2.3; legacy
    /// `is_editable`).
    fn editable(
        &self,
        root: Token<StageIndex>,
    ) -> bool {
        self.handle.graph().vertex(root).is_some_and(|vertex| {
            vertex.state() == VertexState::Staged
                && vertex.parents().next().is_none()
                && vertex.node().composite().is_some()
        })
    }

    /// Apply `span` (table of §3.4) through one `GraphEdit` on the stage;
    /// its edit record (`None` for `Adopt`, which does not edit) and, when
    /// the root grew in place, its previous last child, which now ends
    /// strictly inside the root (event E3 of the cheap stabilization plan
    /// §3).
    ///
    /// # Panics
    ///
    /// On a span that does not extend the root or a rejected edit: both are
    /// bugs of the reader.
    pub(crate) fn commit(
        &mut self,
        span: Span,
    ) -> (Commit, Option<Publication<StageIndex>>, Option<StageIndex>) {
        let Some(root) = self.root else {
            let Span::Append(token) = span else {
                panic!("{:?} without a root", span);
            };
            self.root = Some(token);
            return (Commit::Adopt, None, None);
        };
        let (occurrence, in_place) = match span {
            Span::ReplaceRoot(token) => {
                assert!(
                    token.width > root.width,
                    "replacement must extend the root"
                );
                self.root = Some(token);
                return (Commit::Adopt, None, None);
            },
            Span::Append(token) => (
                Occurrence::new(root.width.0, token),
                token != root && self.editable(root),
            ),
            Span::Expand {
                postfix,
                expansion,
            } => {
                assert!(
                    expansion.width > postfix.width
                        && postfix.width < root.width,
                    "expansion must extend a proper postfix of the root"
                );
                // A self-expansion (`E == R`) never edits the root in place
                // (legacy `ClosurePolicy::SingleStep`).
                (
                    Occurrence::new(root.width.0 - postfix.width.0, expansion),
                    expansion != root && self.editable(root),
                )
            },
        };
        // By G2 exactly one child occurrence ends at the root's end.
        let last = if in_place {
            self.handle
                .graph()
                .vertex(root)
                .and_then(|vertex| {
                    vertex.node().composite().and_then(|composition| {
                        composition
                            .occurrences()
                            .find(|o| usize::from(o.offset) + o.child.width.0 == root.width.0)
                    })
                })
                .map(|o| o.child.index)
        } else {
            None
        };
        let mut view = self.handle.stage();
        let mut edit = GraphEdit::new(&mut view);
        let result = if in_place {
            edit.extend(root.index, occurrence)
        } else {
            edit.create(maximal(vec![Occurrence::new(0, root), occurrence]))
        };
        let token = result.unwrap_or_else(|error| {
            panic!("root commit {:?} rejected: {}", span, error)
        });
        let publication = edit.finish();
        drop(view);
        self.root = Some(token);
        let kind = if in_place {
            Commit::InPlace
        } else {
            Commit::NewRoot
        };
        (kind, Some(publication), last)
    }

    /// Restore invariant (S) after a step (dissolution plan §3.3): dissolve
    /// or discard the staged composites among `candidates` (and their
    /// cascades) with ≤ 1 parent occurrence; then stabilize by Rule S′
    /// (cheap stabilization plan §2.2, §3) from the gained candidates and
    /// the cascade of every stabilization. One `GraphEdit` on the stage.
    pub(crate) fn settle(
        &mut self,
        candidates: Candidates,
    ) -> Settlement {
        let Some(root) = self.root else {
            return Settlement::default();
        };
        let mut view = self.handle.stage();
        let mut edit = GraphEdit::new(&mut view);
        let mut settled = edit.settle(candidates, root.index);
        let stabilized = edit
            .stabilize_candidates(std::mem::take(&mut settled.gained), root.index);
        edit.finish();
        drop(view);
        Settlement {
            settled,
            stabilized,
        }
    }

    /// Commit the stage; the root translated (`None` for empty input).
    pub(crate) fn finish(self) -> Option<(Token, Translation)> {
        let root = self.root?;
        Some(self.handle.commit(root).unwrap_or_else(|error| {
            panic!("committing the read failed: {}", error)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::fixture;

    type Stage<'g> = ReadStage<'g>;

    fn composition(
        stage: &Stage<'_>,
        token: Token<StageIndex>,
    ) -> Vec<Occurrence<StageIndex>> {
        stage
            .handle
            .graph()
            .vertex(token)
            .and_then(|vertex| vertex.node().composite())
            .map(|composition| composition.occurrences().collect())
            .unwrap_or_default()
    }

    fn editable(
        stage: &Stage<'_>,
    ) -> bool {
        stage.editable(stage.token().unwrap())
    }

    /// Commit the stage and validate the committed graph.
    fn finish(
        graph: &PositionedGraphRef,
        stage: Stage<'_>,
    ) {
        stage.finish().unwrap();
        graph.read().validate().unwrap();
    }

    /// `Append`: adopt without a root; a new editable root on an adopted
    /// root (which stays unchanged); in place on an editable root; a new
    /// root when the root is appended to itself.
    #[test]
    fn append_rows() {
        let (graph, names) = fixture("two_chars");
        let (a, b) = (names["a"].into(), names["b"].into());
        let generation = graph.read().generation();
        let mut root = Stage::open(&graph);

        assert_eq!(root.commit(Span::Append(a)).0, Commit::Adopt);
        assert_eq!(root.token(), Some(a));
        assert!(!editable(&root), "an adopted atom is not editable");
        assert_eq!(root.handle.graph().generation().stage, 0);

        assert_eq!(root.commit(Span::Append(a)).0, Commit::NewRoot);
        let aa = root.token().unwrap();
        assert!(editable(&root) && aa.index != a.index && aa.width.0 == 2);
        assert!(composition(&root, aa).contains(&Occurrence::new(0, a)));
        assert!(composition(&root, aa).contains(&Occurrence::new(1, a)));

        // In place: the previous last child `a@1` now ends inside (E3).
        let (kind, _, last) = root.commit(Span::Append(b));
        assert_eq!((kind, last), (Commit::InPlace, Some(a.index)));
        let aab = root.token().unwrap();
        assert_eq!((aab.index, aab.width.0), (aa.index, 3));
        assert!(composition(&root, aab).contains(&Occurrence::new(2, b)));

        assert_eq!(root.commit(Span::Append(aab)).0, Commit::NewRoot);
        let doubled = root.token().unwrap();
        assert!(editable(&root) && doubled.index != aab.index);
        assert!(composition(&root, doubled).contains(&Occurrence::new(0, aab)));
        assert!(composition(&root, doubled).contains(&Occurrence::new(3, aab)));
        assert_eq!(graph.read().generation(), generation, "the base is read-only");
        finish(&graph, root);
    }

    /// `ReplaceRoot` adopts; the next append creates a new root and leaves
    /// the adopted token unchanged.
    #[test]
    fn replace_root_adopts() {
        let (graph, names) = fixture("two_chars");
        let (a, b, ab) = (names["a"].into(), names["b"].into(), names["ab"].into());
        let mut root = Stage::open(&graph);
        root.commit(Span::Append(a));
        assert_eq!(root.commit(Span::ReplaceRoot(ab)).0, Commit::Adopt);
        assert_eq!(root.token(), Some(ab));
        assert!(!editable(&root));
        let before = composition(&root, ab);
        assert_eq!(root.commit(Span::Append(b)).0, Commit::NewRoot);
        assert_ne!(root.token().unwrap().index, ab.index);
        assert_eq!(composition(&root, ab), before);
        finish(&graph, root);
    }

    /// `Expand` on an editable root grows it in place at `|R| − |P|`.
    #[test]
    fn expand_editable_in_place() {
        let (graph, names) = fixture("linear_abc");
        let abc = names["abc"].into();
        let mut root = Stage::open(&graph);
        root.commit(Span::Append(abc));
        root.commit(Span::Append(abc));
        let editable_root = root.token().unwrap();
        let expansion =
            context_insert::insert(&mut root.handle, vec![abc, names["a"].into()]).unwrap();
        let span = Span::Expand {
            postfix: abc,
            expansion,
        };
        assert_eq!(root.consumed(span), 1);
        assert_eq!(root.commit(span).0, Commit::InPlace);
        let grown = root.token().unwrap();
        assert_eq!((grown.index, grown.width.0), (editable_root.index, 7));
        assert!(editable(&root));
        assert!(composition(&root, grown).contains(&Occurrence::new(3, expansion)));
        finish(&graph, root);
    }

    /// `Expand` on an adopted root creates `maximal([R@0, E@o])`.
    #[test]
    fn expand_adopted_creates() {
        let (graph, names) = fixture("closure_forward_seed");
        let (ab, b, bc) = (names["ab"].into(), names["b"].into(), names["bc"].into());
        let mut root = Stage::open(&graph);
        root.commit(Span::Append(ab));
        let span = Span::Expand {
            postfix: b,
            expansion: bc,
        };
        assert_eq!(root.commit(span).0, Commit::NewRoot);
        let new = root.token().unwrap();
        assert!(editable(&root) && new.width.0 == 3);
        assert!(composition(&root, new).contains(&Occurrence::new(0, ab)));
        assert!(composition(&root, new).contains(&Occurrence::new(1, bc)));
        finish(&graph, root);
    }

    /// A self-expansion (`E == R`) creates a new root even when `R` is
    /// editable.
    #[test]
    fn self_expansion_creates() {
        let (graph, names) = fixture("two_chars");
        let a = names["a"].into();
        let mut root = Stage::open(&graph);
        root.commit(Span::Append(a));
        root.commit(Span::Append(a));
        let aa = root.token().unwrap();
        assert!(editable(&root));
        let span = Span::Expand {
            postfix: a,
            expansion: aa,
        };
        assert_eq!(root.commit(span).0, Commit::NewRoot);
        let new = root.token().unwrap();
        assert!(editable(&root) && new.index != aa.index && new.width.0 == 3);
        assert!(composition(&root, new).contains(&Occurrence::new(0, aa)));
        assert!(composition(&root, new).contains(&Occurrence::new(1, aa)));
        assert_eq!(composition(&root, aa).len(), 2);
        finish(&graph, root);
    }

    /// An atom new in this read is committed at once: as a root it is
    /// adopted, not editable, and appending to it creates a new root.
    #[test]
    fn new_atom_root_is_not_editable() {
        let graph = PositionedGraphRef::default();
        let mut root = Stage::open(&graph);
        let segments = crate::segment::segments(&mut root.handle, "xy".chars());
        let (x, y) = (segments[0].unknown[0], segments[0].unknown[1]);
        assert!(!x.index.is_staged() && !y.index.is_staged());
        assert_eq!(root.commit(Span::Append(x)).0, Commit::Adopt);
        assert!(!editable(&root));
        assert_eq!(root.commit(Span::Append(y)).0, Commit::NewRoot);
        let xy = root.token().unwrap();
        assert!(xy.index.is_staged() && editable(&root));
        assert_eq!(
            composition(&root, xy),
            vec![Occurrence::new(0, x), Occurrence::new(1, y)]
        );
        finish(&graph, root);
    }
}
