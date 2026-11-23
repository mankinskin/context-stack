//! The input's own vertex and its commits (reader plan §3.4, §3.5; legacy
//! `pipeline/root.rs`).
//!
//! The root is a vertex at all times. A root created by this read and
//! without parents (*owned*) grows in place through `GraphEdit::extend`; an
//! adopted token is never edited: a new owned root is created instead. The
//! manager never searches.

use context_trace::{
    Token,
    positioned::{
        Occurrence,
        PositionedGraphRef,
        maximal,
    },
};

/// One step of the reader, committed before the next search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Span {
    /// `t` starts at the end of the root.
    Append(Token),
    /// `Y` is an existing token covering the root and the consumed input.
    ReplaceRoot(Token),
    /// The root's postfix `postfix` extends to `expansion` (§3.2).
    Expand { postfix: Token, expansion: Token },
}

/// How a commit changed the root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Commit {
    /// An existing token became the root (not owned).
    Adopt,
    /// The owned root grew in place (`GraphEdit::extend`).
    InPlace,
    /// A new owned root was created (`GraphEdit::create`).
    NewRoot,
}

/// The root of the input read so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RootManager {
    root: Option<Token>,
    /// Created by this read; editable while it has no parents.
    owned: bool,
}

impl RootManager {
    pub(crate) fn token(&self) -> Option<Token> {
        self.root
    }

    #[cfg(test)]
    pub(crate) fn owned(&self) -> bool {
        self.owned
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

    /// Owned and parentless (legacy `is_editable`).
    fn editable(
        &self,
        graph: &PositionedGraphRef,
        root: Token,
    ) -> bool {
        self.owned
            && graph
                .read()
                .vertex(root.index)
                .is_some_and(|vertex| vertex.parents().is_empty())
    }

    /// Apply `span` (table of §3.4) through one `GraphEdit`.
    ///
    /// # Panics
    ///
    /// On a span that does not extend the root or a rejected edit: both are
    /// bugs of the reader.
    pub(crate) fn commit(
        &mut self,
        graph: &PositionedGraphRef,
        span: Span,
    ) -> Commit {
        let Some(root) = self.root else {
            let Span::Append(token) = span else {
                panic!("{:?} without a root", span);
            };
            self.root = Some(token);
            self.owned = false;
            return Commit::Adopt;
        };
        let (occurrence, in_place) = match span {
            Span::ReplaceRoot(token) => {
                assert!(
                    token.width > root.width,
                    "replacement must extend the root"
                );
                self.root = Some(token);
                self.owned = false;
                return Commit::Adopt;
            },
            Span::Append(token) => (
                Occurrence::new(root.width.0, token),
                token != root && self.editable(graph, root),
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
                    expansion != root && self.editable(graph, root),
                )
            },
        };
        let mut guard = graph.write();
        let mut edit = guard.edit();
        let result = if in_place {
            edit.extend(root.index, occurrence)
        } else {
            edit.create(maximal(vec![Occurrence::new(0, root), occurrence]))
        };
        let token = result.unwrap_or_else(|error| {
            panic!("root commit {:?} rejected: {}", span, error)
        });
        edit.finish();
        self.root = Some(token);
        self.owned = true;
        if in_place {
            Commit::InPlace
        } else {
            Commit::NewRoot
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::fixture;

    fn composition(
        graph: &PositionedGraphRef,
        token: Token,
    ) -> Vec<Occurrence> {
        graph
            .read()
            .vertex(token.index)
            .and_then(|vertex| vertex.composition())
            .map(|composition| composition.occurrences().to_vec())
            .unwrap_or_default()
    }

    /// `Append`: adopt without a root; a new owned root on an adopted root
    /// (which stays unchanged); in place on an owned root; a new root when
    /// the root is appended to itself.
    #[test]
    fn append_rows() {
        let (graph, names) = fixture("two_chars");
        let (a, b) = (names["a"], names["b"]);
        let generation = graph.read().generation();
        let mut root = RootManager::default();

        assert_eq!(root.commit(&graph, Span::Append(a)), Commit::Adopt);
        assert_eq!((root.token(), root.owned()), (Some(a), false));
        assert_eq!(graph.read().generation(), generation);

        assert_eq!(root.commit(&graph, Span::Append(a)), Commit::NewRoot);
        let aa = root.token().unwrap();
        assert!(root.owned() && aa.index != a.index && aa.width.0 == 2);
        assert!(composition(&graph, aa).contains(&Occurrence::new(0, a)));
        assert!(composition(&graph, aa).contains(&Occurrence::new(1, a)));
        graph.read().validate().unwrap();

        assert_eq!(root.commit(&graph, Span::Append(b)), Commit::InPlace);
        let aab = root.token().unwrap();
        assert_eq!((aab.index, aab.width.0), (aa.index, 3));
        assert!(composition(&graph, aab).contains(&Occurrence::new(2, b)));
        graph.read().validate().unwrap();

        assert_eq!(root.commit(&graph, Span::Append(aab)), Commit::NewRoot);
        let doubled = root.token().unwrap();
        assert!(root.owned() && doubled.index != aab.index);
        assert!(composition(&graph, doubled).contains(&Occurrence::new(0, aab)));
        assert!(composition(&graph, doubled).contains(&Occurrence::new(3, aab)));
        graph.read().validate().unwrap();
    }

    /// `ReplaceRoot` adopts; the next append creates a new root and leaves
    /// the adopted token unchanged.
    #[test]
    fn replace_root_adopts() {
        let (graph, names) = fixture("two_chars");
        let (a, b, ab) = (names["a"], names["b"], names["ab"]);
        let mut root = RootManager::default();
        root.commit(&graph, Span::Append(a));
        assert_eq!(root.commit(&graph, Span::ReplaceRoot(ab)), Commit::Adopt);
        assert_eq!((root.token(), root.owned()), (Some(ab), false));
        let before = composition(&graph, ab);
        assert_eq!(root.commit(&graph, Span::Append(b)), Commit::NewRoot);
        assert_ne!(root.token().unwrap().index, ab.index);
        assert_eq!(composition(&graph, ab), before);
        graph.read().validate().unwrap();
    }

    /// `Expand` on an owned root grows it in place at `|R| − |P|`.
    #[test]
    fn expand_owned_in_place() {
        let (graph, names) = fixture("linear_abc");
        let abc = names["abc"];
        let mut root = RootManager::default();
        root.commit(&graph, Span::Append(abc));
        root.commit(&graph, Span::Append(abc));
        let owned = root.token().unwrap();
        let expansion =
            context_insert::insert(&graph, vec![abc, names["a"]]).unwrap();
        let span = Span::Expand {
            postfix: abc,
            expansion,
        };
        assert_eq!(root.consumed(span), 1);
        assert_eq!(root.commit(&graph, span), Commit::InPlace);
        let grown = root.token().unwrap();
        assert_eq!((grown.index, grown.width.0), (owned.index, 7));
        assert!(root.owned());
        assert!(composition(&graph, grown).contains(&Occurrence::new(3, expansion)));
        graph.read().validate().unwrap();
    }

    /// `Expand` on an adopted root creates `maximal([R@0, E@o])`.
    #[test]
    fn expand_adopted_creates() {
        let (graph, names) = fixture("closure_forward_seed");
        let (ab, b, bc) = (names["ab"], names["b"], names["bc"]);
        let mut root = RootManager::default();
        root.commit(&graph, Span::Append(ab));
        let span = Span::Expand {
            postfix: b,
            expansion: bc,
        };
        assert_eq!(root.commit(&graph, span), Commit::NewRoot);
        let new = root.token().unwrap();
        assert!(root.owned() && new.width.0 == 3);
        assert!(composition(&graph, new).contains(&Occurrence::new(0, ab)));
        assert!(composition(&graph, new).contains(&Occurrence::new(1, bc)));
        graph.read().validate().unwrap();
    }

    /// A self-expansion (`E == R`) creates a new root even when `R` is
    /// owned.
    #[test]
    fn self_expansion_creates() {
        let (graph, names) = fixture("two_chars");
        let a = names["a"];
        let mut root = RootManager::default();
        root.commit(&graph, Span::Append(a));
        root.commit(&graph, Span::Append(a));
        let aa = root.token().unwrap();
        assert!(root.owned());
        let span = Span::Expand {
            postfix: a,
            expansion: aa,
        };
        assert_eq!(root.commit(&graph, span), Commit::NewRoot);
        let new = root.token().unwrap();
        assert!(root.owned() && new.index != aa.index && new.width.0 == 3);
        assert!(composition(&graph, new).contains(&Occurrence::new(0, aa)));
        assert!(composition(&graph, new).contains(&Occurrence::new(1, aa)));
        assert_eq!(composition(&graph, aa).len(), 2);
        graph.read().validate().unwrap();
    }
}
