//! Dissolution of provisional tokens (dissolution plan §3.2–§3.3, D4):
//! [`GraphEdit::dissolve`], [`GraphEdit::discard`] and
//! [`GraphEdit::settle`] on a [`Stage`], with the step's [`Candidates`].

use super::{
    Error,
    GraphEdit,
    Publication,
};
use crate::positioned::check::expect_vertex;
use crate::{
    graph::vertex::StageIndex,
    positioned::{
        model::{
            Composition,
            Occurrence,
            PositionedError,
            maximal,
        },
        stage::Stage,
        store::{
            CompositionRead as _,
            VertexRead,
            raw::SEAL,
        },
    },
};

/// The candidates of one reader step: the vertices whose parent count may
/// have dropped below 2 (§3.3: children of removed occurrences, created
/// tokens and the previous root), and the vertices that may have gained a
/// fixed parent occurrence (cheap stabilization plan §3: E1 children of
/// added occurrences and of created tokens, E3 the root's previous last
/// child, and the previous root).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Candidates {
    removal: Vec<StageIndex>,
    gained: Vec<StageIndex>,
    /// Created tokens: all their children gained a parent occurrence
    /// (expanded by `settle`, which reads the compositions).
    created: Vec<StageIndex>,
}

impl Candidates {
    /// The children of the `removed` occurrences of `publication` and its
    /// created tokens (removal); the children of its `added` occurrences
    /// and its created tokens with their children (gained, E1).
    pub fn record(
        &mut self,
        publication: &Publication<StageIndex>,
    ) {
        self.removal
            .extend(publication.edits.iter().flat_map(|edit| {
                edit.removed.iter().map(|occurrence| occurrence.child.index)
            }));
        self.removal
            .extend(publication.created.iter().map(|token| token.index));
        self.gained
            .extend(publication.edits.iter().flat_map(|edit| {
                edit.added.iter().map(|occurrence| occurrence.child.index)
            }));
        self.created
            .extend(publication.created.iter().map(|token| token.index));
    }
    /// A vertex that lost its exemption (the previous root): a candidate
    /// for removal and for stabilization.
    pub fn push(
        &mut self,
        vertex: StageIndex,
    ) {
        self.removal.push(vertex);
        self.gained.push(vertex);
    }
    /// A vertex that may have gained a fixed parent occurrence without an
    /// edit record (E3: the root's previous last child after the root grew
    /// in place).
    pub fn push_gained(
        &mut self,
        vertex: StageIndex,
    ) {
        self.gained.push(vertex);
    }
    /// The recorded removal candidates, in recording order (with repeats).
    pub fn vertices(&self) -> &[StageIndex] {
        &self.removal
    }
    /// The recorded gained candidates (E1 without the children of created
    /// tokens, E3), in recording order (with repeats).
    pub fn gained(&self) -> &[StageIndex] {
        &self.gained
    }
}

/// The result of [`GraphEdit::dissolve`] or [`GraphEdit::discard`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removal {
    /// The removed vertex (its slot is now a hole).
    pub removed: StageIndex,
    /// The vertex dissolved into; `None` for a discard.
    pub host: Option<StageIndex>,
    /// The children of the removed vertex in composition order.
    pub children: Vec<StageIndex>,
}

/// Counts of one [`GraphEdit::settle`] and the surviving gained candidates.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settled {
    /// Distinct candidates given.
    pub candidates: usize,
    pub dissolved: usize,
    pub discarded: usize,
    /// Dissolutions whose host was the root.
    pub into_root: usize,
    /// The staged gained candidates (E1, E3, previous root) without the
    /// removed vertices, plus the surviving
    /// created tokens with their children and the children of every
    /// dissolution (E1); sorted and deduplicated. The input of
    /// [`GraphEdit::stabilize_candidates`].
    pub gained: Vec<StageIndex>,
}

impl<S: Stage + ?Sized> GraphEdit<'_, S> {
    /// Remove the staged composite `t ≠ root` with exactly one parent
    /// occurrence `(U, o)`: `comp(U) := maximal(comp(U) − t@o ∪ comp(t) + o)`,
    /// the children of `t` lose their links to `t` (those kept in `U` are
    /// linked to `U`), then `t` is removed (its slot becomes a hole).
    ///
    /// # Errors
    /// Graph unchanged: `UnknownVertex`, `Atom`, `NotStaged` (committed),
    /// `IsRoot`, `ParentCount` (count ≠ 1).
    pub fn dissolve(
        &mut self,
        t: StageIndex,
        root: StageIndex,
    ) -> Result<Removal, Error<S>> {
        let children = self.removable(t, root, 1)?;
        let (host, offset) = expect_vertex(&*self.graph, t)?
            .parents()
            .next()
            .expect("checked parent count");
        let host = host.index;
        let (_, old) = self.composite(host)?;
        let mut union: Vec<Occurrence<StageIndex>> = old
            .iter()
            .filter(|o| !(o.child.index == t && o.offset == offset))
            .copied()
            .collect();
        union.extend(
            children
                .iter()
                .map(|c| Occurrence::new(offset.0 + c.offset.0, c.child)),
        );
        let composition =
            Composition::new(maximal(union)).map_err(|error| {
                PositionedError::Composition {
                    owner: Some(host),
                    error,
                }
            })?;
        self.mutate();
        self.recompose(host, &old, composition);
        for child in &children {
            self.unlink(t, child);
        }
        Ok(self.remove(t, Some(host), &children))
    }

    /// Remove the parentless staged composite `t ≠ root`: its children lose
    /// their links to it, then it is removed (its slot becomes a hole).
    ///
    /// # Errors
    /// As [`Self::dissolve`], with `ParentCount` for count ≠ 0.
    pub fn discard(
        &mut self,
        t: StageIndex,
        root: StageIndex,
    ) -> Result<Removal, Error<S>> {
        let children = self.removable(t, root, 0)?;
        self.mutate();
        for child in &children {
            self.unlink(t, child);
        }
        Ok(self.remove(t, None, &children))
    }

    /// Restore invariant (S) (§3.3): dissolve or discard every staged
    /// composite other than `root` with ≤ 1 parent occurrence, starting from
    /// `candidates` and continuing with the children of removed vertices.
    /// Stable vertices are skipped. The gained candidates are returned in
    /// [`Settled::gained`], with the children of the surviving created
    /// tokens and of every dissolution (E1).
    ///
    /// # Panics
    /// If a removal fails, and (debug and `test-api` builds) if a staged
    /// composite other than `root` has < 2 parent occurrences afterwards:
    /// both are bugs (incomplete candidates).
    pub fn settle(
        &mut self,
        candidates: Candidates,
        root: StageIndex,
    ) -> Settled {
        let Candidates {
            removal: mut work,
            mut gained,
            mut created,
        } = candidates;
        work.sort();
        work.dedup();
        gained.sort();
        gained.dedup();
        let mut settled = Settled {
            candidates: work.len(),
            ..Settled::default()
        };
        while let Some(t) = work.pop() {
            if !t.is_staged() || t == root {
                continue;
            }
            if self.graph.is_stable(t) {
                debug_assert!(
                    expect_vertex(&*self.graph, t)
                        .is_ok_and(|vertex| vertex.parent_count() >= 2),
                    "stable {t} lost its parent occurrences"
                );
                continue;
            }
            let count = expect_vertex(&*self.graph, t)
                .unwrap_or_else(|error| panic!("settle: {error}"))
                .parent_count();
            let removal = match count {
                0 => {
                    settled.discarded += 1;
                    self.discard(t, root)
                },
                1 => {
                    settled.dissolved += 1;
                    self.dissolve(t, root)
                },
                _ => continue,
            }
            .unwrap_or_else(|error| panic!("settle: {error}"));
            work.retain(|&i| i != t);
            gained.retain(|&i| i != t);
            created.retain(|&i| i != t);
            if removal.host == Some(root) {
                settled.into_root += 1;
            }
            if removal.host.is_some() {
                // E1: the children of `t` were added to the host.
                gained.extend(&removal.children);
            }
            work.extend(removal.children);
        }
        for vertex in created {
            if let Some(composition) = expect_vertex(&*self.graph, vertex)
                .ok()
                .and_then(|vertex| vertex.node().composite())
            {
                gained.extend(composition.occurrences().map(|c| c.child.index));
            }
            gained.push(vertex);
        }
        gained.retain(|index| index.is_staged());
        gained.sort();
        gained.dedup();
        settled.gained = gained;
        #[cfg(any(debug_assertions, feature = "test-api"))]
        self.assert_settled(root);
        settled
    }

    /// Invariant (S) by a full scan of the stage.
    #[cfg(any(debug_assertions, feature = "test-api"))]
    fn assert_settled(
        &self,
        root: StageIndex,
    ) {
        use crate::positioned::store::GraphRead;
        for staged in self.graph.staged() {
            let index = StageIndex::from(staged);
            let vertex = self.graph.vertex(index).expect("staged vertex");
            let count = vertex.parent_count();
            if index != root && vertex.node().composite().is_some() && count < 2
            {
                panic!(
                    "invariant (S) violated after settle: staged {index} has {count} parent occurrences"
                );
            }
        }
    }

    /// The checks of [`Self::dissolve`] / [`Self::discard`]; the children of
    /// `t`.
    fn removable(
        &self,
        t: StageIndex,
        root: StageIndex,
        count: usize,
    ) -> Result<Vec<Occurrence<StageIndex>>, Error<S>> {
        let (_, children) = self.composite(t)?;
        if !t.is_staged() {
            return Err(PositionedError::NotStaged(t));
        }
        if t == root {
            return Err(PositionedError::IsRoot(t));
        }
        let found = expect_vertex(&*self.graph, t)?.parent_count();
        if found != count {
            return Err(PositionedError::ParentCount {
                vertex: t,
                count: found,
            });
        }
        Ok(children)
    }

    /// Remove the unlinked `t` and update the bookkeeping: `t` leaves
    /// `touched`, `edits` and `created` (also as the child of recorded
    /// occurrences, so no record points at the hole).
    fn remove(
        &mut self,
        t: StageIndex,
        host: Option<StageIndex>,
        children: &[Occurrence<StageIndex>],
    ) -> Removal {
        self.graph.remove(SEAL, t);
        self.touched.remove(&t);
        self.created.retain(|token| token.index != t);
        self.edits.remove(&t);
        for edit in self.edits.values_mut() {
            for occurrences in [&mut edit.added, &mut edit.removed] {
                occurrences.retain(|o| o.child.index != t);
            }
        }
        Removal {
            removed: t,
            host,
            children: children.iter().map(|c| c.child.index).collect(),
        }
    }
}
