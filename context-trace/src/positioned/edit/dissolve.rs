//! Dissolution of provisional tokens (dissolution plan §3.2–§3.3, D4):
//! [`GraphEdit::dissolve`], [`GraphEdit::discard`] and
//! [`GraphEdit::settle`] on a [`Stage`], with the step's [`Candidates`].

use super::{
    Error,
    GraphEdit,
    Publication,
    check::expect_vertex,
};
use crate::{
    Token,
    graph::vertex::StageIndex,
    positioned::{
        model::{
            Composition,
            Occurrence,
            PositionedError,
            maximal,
        },
        stage::{
            Moved,
            Stage,
        },
        store::{
            VertexRead,
            raw::SEAL,
        },
    },
};

/// The vertices whose parent count may have dropped below 2 in one reader
/// step (§3.3): children of removed occurrences, created tokens and the
/// previous root.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Candidates(Vec<StageIndex>);

impl Candidates {
    /// The children of the `removed` occurrences of `publication` and its
    /// created tokens.
    pub fn record(
        &mut self,
        publication: &Publication<StageIndex>,
    ) {
        self.0.extend(publication.edits.iter().flat_map(|edit| {
            edit.removed.iter().map(|occurrence| occurrence.child.index)
        }));
        self.0
            .extend(publication.created.iter().map(|token| token.index));
    }
    /// A vertex that lost its exemption (the previous root).
    pub fn push(
        &mut self,
        vertex: StageIndex,
    ) {
        self.0.push(vertex);
    }
    /// The recorded vertices, in recording order (with repeats).
    pub fn vertices(&self) -> &[StageIndex] {
        &self.0
    }
}

/// The result of [`GraphEdit::dissolve`] or [`GraphEdit::discard`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removal {
    /// The removed vertex (its index at removal time; after a move the slot
    /// holds [`Moved::from`]).
    pub removed: StageIndex,
    /// The vertex dissolved into (after renaming); `None` for a discard.
    pub host: Option<StageIndex>,
    /// The children of the removed vertex in composition order (after
    /// renaming).
    pub children: Vec<StageIndex>,
    /// The vertex moved into the freed slot, if any.
    pub moved: Option<Moved>,
}

/// Counts of one [`GraphEdit::settle`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settled {
    /// Distinct candidates given.
    pub candidates: usize,
    pub dissolved: usize,
    pub discarded: usize,
    /// Dissolutions whose host was the root.
    pub into_root: usize,
}

impl<S: Stage + ?Sized> GraphEdit<'_, S> {
    /// Remove the staged composite `t ≠ root` with exactly one parent
    /// occurrence `(U, o)`: `comp(U) := maximal(comp(U) − t@o ∪ comp(t) + o)`,
    /// the children of `t` lose their links to `t` (those kept in `U` are
    /// linked to `U`), then `t` is removed (swap-remove, §3.2).
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
    /// their links to it, then it is removed (swap-remove, §3.2).
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
    /// `root` is renamed when it is moved.
    ///
    /// # Panics
    /// If a removal fails, and (debug and `test-api` builds) if a staged
    /// composite other than `root` has < 2 parent occurrences afterwards:
    /// both are bugs (incomplete candidates).
    pub fn settle(
        &mut self,
        candidates: Candidates,
        root: &mut Token<StageIndex>,
    ) -> Settled {
        let mut work = candidates.0;
        work.sort();
        work.dedup();
        let mut settled = Settled {
            candidates: work.len(),
            ..Settled::default()
        };
        while let Some(t) = work.pop() {
            if !t.is_staged() || t == root.index {
                continue;
            }
            let count = expect_vertex(&*self.graph, t)
                .unwrap_or_else(|error| panic!("settle: {error}"))
                .parent_count();
            let removal = match count {
                0 => {
                    settled.discarded += 1;
                    self.discard(t, root.index)
                },
                1 => {
                    settled.dissolved += 1;
                    self.dissolve(t, root.index)
                },
                _ => continue,
            }
            .unwrap_or_else(|error| panic!("settle: {error}"));
            work.retain(|&i| i != t);
            if let Some(moved) = removal.moved {
                for index in &mut work {
                    *index = moved.apply(*index);
                }
                root.index = moved.apply(root.index);
            }
            if removal.host == Some(root.index) {
                settled.into_root += 1;
            }
            work.extend(removal.children);
        }
        #[cfg(any(debug_assertions, feature = "test-api"))]
        self.assert_settled(root.index);
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

    /// Swap-remove the unlinked `t` and update the bookkeeping: `t` leaves
    /// `touched`, `edits` and `created` (also as the child of recorded
    /// occurrences, so no record points at the reused slot), then the moved
    /// vertex is renamed everywhere.
    fn remove(
        &mut self,
        t: StageIndex,
        host: Option<StageIndex>,
        children: &[Occurrence<StageIndex>],
    ) -> Removal {
        let moved = self.graph.remove(SEAL, t);
        let rename =
            |index: StageIndex| moved.map_or(index, |m| m.apply(index));
        self.touched.remove(&t);
        self.touched = std::mem::take(&mut self.touched)
            .into_iter()
            .map(rename)
            .collect();
        self.created.retain(|token| token.index != t);
        for token in &mut self.created {
            token.index = rename(token.index);
        }
        self.edits.remove(&t);
        self.edits = std::mem::take(&mut self.edits)
            .into_iter()
            .map(|(vertex, mut edit)| {
                let vertex = rename(vertex);
                edit.vertex = vertex;
                for occurrences in [&mut edit.added, &mut edit.removed] {
                    occurrences.retain(|o| o.child.index != t);
                    for occurrence in occurrences.iter_mut() {
                        occurrence.child.index = rename(occurrence.child.index);
                    }
                }
                (vertex, edit)
            })
            .collect();
        Removal {
            removed: t,
            host: host.map(rename),
            children: children.iter().map(|c| rename(c.child.index)).collect(),
            moved,
        }
    }
}
