//! Parent management operations

use crate::{
    graph::{
        getters::vertex::VertexSet,
        kind::GraphKind,
        vertex::{
            has_vertex_index::{
                HasVertexIndex,
                ToToken,
            },
            location::child::ChildLocation,
            pattern::{
                id::PatternId,
                pattern_width,
                IntoPattern,
            },
            token::Token,
            wide::WideMut,
        },
    },
    HashSet,
    Hypergraph,
    Wide,
};

impl<G: GraphKind> Hypergraph<G> {
    /// Replace the last child of an exclusively owned, parentless pattern.
    ///
    /// The caller must own the parent; structural guards are checked here.
    /// Parentlessness also excludes a replacement containing the parent.
    /// Updates the vertex width and child-parent links, including retained
    /// children's parent widths. Returns a token with the updated width;
    /// previously held parent tokens retain their old width.
    ///
    /// # Panics
    ///
    /// Panics if the parent has parents or multiple patterns, the pattern or
    /// expected tail is missing, the parent or replacement width is stale, the
    /// replacement does not grow, references the parent, or overflows its width.
    pub fn replace_owned_tail(
        &self,
        parent: Token,
        pattern_id: PatternId,
        old: Token,
        replacement: Token,
    ) -> Token {
        let vertex = self.expect_vertex_data(parent);
        assert_eq!(vertex.width(), parent.width(), "stale parent width");
        assert!(vertex.parents().is_empty(), "owned tail parent has parents");
        assert_eq!(
            vertex.child_patterns().len(),
            1,
            "owned tail parent has multiple patterns"
        );
        let pattern = vertex.expect_child_pattern(&pattern_id).clone();
        assert_eq!(pattern.last(), Some(&old), "stale last child");
        assert!(
            replacement.width() > old.width(),
            "tail replacement must grow"
        );
        assert_ne!(
            replacement.vertex_index(),
            parent.vertex_index(),
            "self-referential tail"
        );
        assert_eq!(
            self.expect_vertex_data(replacement).width(),
            replacement.width(),
            "stale replacement width"
        );

        let width = crate::TokenWidth(
            parent
                .width()
                .0
                .checked_add(replacement.width().0 - old.width().0)
                .expect("tail replacement width overflow"),
        );
        let tail_index = pattern.len() - 1;
        self.with_vertex_mut(old.vertex_index(), |node| {
            node.remove_parent_index(
                parent.vertex_index(),
                pattern_id,
                tail_index,
            );
        })
        .expect("old tail must exist");
        self.with_vertex_mut(parent.vertex_index(), |node| {
            node.expect_child_pattern_mut(&pattern_id)[tail_index] =
                replacement;
            *node.width_mut() = width;
        })
        .expect("owned parent must exist");
        for child in pattern
            .iter()
            .take(tail_index)
            .copied()
            .collect::<HashSet<_>>()
        {
            self.with_vertex_mut(child.vertex_index(), |node| {
                node.expect_parent_mut(parent.vertex_index()).width = width;
            })
            .expect("retained child must exist");
        }
        let parent = Token::new(parent.vertex_index(), width);
        self.add_pattern_parent(
            parent,
            vec![replacement],
            pattern_id,
            tail_index,
        );
        parent
    }

    /// Utility: builds total width, indices and tokens for pattern
    pub(super) fn to_width_indices_children(
        &self,
        indices: impl IntoIterator<Item = impl HasVertexIndex>,
    ) -> (usize, Vec<crate::graph::vertex::VertexIndex>, Vec<Token>) {
        let mut width = 0;
        let (a, b) = indices
            .into_iter()
            .map(|index| {
                let index = index.vertex_index();
                let w = self.expect_vertex_data(index.vertex_index()).width();
                width += w.0;
                (index, Token::new(index, w))
            })
            .unzip();
        (width, a, b)
    }

    /// Adds a parent to all nodes in a pattern
    #[track_caller]
    pub fn add_parents_to_pattern_nodes<I: HasVertexIndex, P: ToToken>(
        &self,
        pattern: Vec<I>,
        parent: P,
        pattern_id: PatternId,
    ) {
        for (i, token) in pattern.into_iter().enumerate() {
            self.with_vertex_mut(token.vertex_index(), |node| {
                node.add_parent(ChildLocation::new(
                    parent.to_token(),
                    pattern_id,
                    i,
                ));
            })
            .expect("Vertex should exist");
        }
    }

    pub(crate) fn add_pattern_parent(
        &self,
        parent: impl ToToken,
        pattern: impl IntoPattern,
        pattern_id: PatternId,
        start: usize,
    ) {
        pattern
            .into_pattern()
            .into_iter()
            .enumerate()
            .for_each(|(pos, c)| {
                let pos = start + pos;
                self.with_vertex_mut(c.vertex_index(), |node| {
                    node.add_parent(ChildLocation::new(
                        parent.to_token(),
                        pattern_id,
                        pos,
                    ));
                })
                .expect("Vertex should exist");
            });
    }
}

#[allow(dead_code)]
impl<G: GraphKind> Hypergraph<G> {
    #[deprecated(
        since = "0.2.0",
        note = "Use `extend_root_pattern` (safe, creates new vertex) or `append_to_owned_pattern` (in-place, with ownership guards) instead."
    )]
    #[allow(deprecated)]
    pub fn append_to_pattern(
        &self,
        parent: impl ToToken,
        pattern_id: PatternId,
        new: impl IntoIterator<Item = impl ToToken>,
    ) -> Token {
        let new: Vec<_> = new.into_iter().map(|c| c.to_token()).collect();
        if new.is_empty() {
            return parent.to_token();
        }
        let width = pattern_width(&new);

        // Get pattern and update child parents
        let pattern = self
            .with_vertex(parent.vertex_index(), |vertex| {
                vertex.expect_child_pattern(&pattern_id).clone()
            })
            .expect("Parent vertex should exist");

        for c in pattern.into_iter().collect::<HashSet<_>>() {
            self.with_vertex_mut(c.vertex_index(), |node| {
                node.get_parent_mut(parent.vertex_index()).unwrap().width +=
                    width;
            })
            .expect("Child vertex should exist");
        }

        // Update parent vertex
        let (offset, final_width) = self
            .with_vertex_mut(parent.vertex_index(), |vertex| {
                let pattern = vertex.expect_child_pattern_mut(&pattern_id);
                let offset = pattern.len();
                pattern.extend(new.iter());
                *vertex.width_mut() += width.0;
                (offset, vertex.width())
            })
            .expect("Parent vertex should exist");

        let parent = Token::new(parent.vertex_index(), final_width);
        self.add_pattern_parent(parent, new, pattern_id, offset);
        parent
    }

    /// Extend a pattern by creating a NEW vertex containing the existing
    /// pattern's children followed by the new tokens.
    ///
    /// This is the **safe** variant — it never mutates the parent vertex.
    /// The original vertex remains unchanged, so any other patterns or
    /// tokens referencing it are unaffected.
    ///
    /// # Returns
    /// A new `Token` for the newly-created vertex. The caller must update
    /// any root references to point to this new token.
    pub fn extend_root_pattern(
        &self,
        parent: impl ToToken,
        pattern_id: PatternId,
        new: impl IntoIterator<Item = impl ToToken>,
    ) -> Token {
        let new: Vec<_> = new.into_iter().map(|c| c.to_token()).collect();
        if new.is_empty() {
            return parent.to_token();
        }

        // Read the existing pattern from the parent vertex (non-mutating)
        let existing_pattern = self
            .with_vertex(parent.vertex_index(), |vertex| {
                vertex.expect_child_pattern(&pattern_id).clone()
            })
            .expect("Parent vertex should exist");

        // Build new_pattern = existing_pattern ++ new_tokens
        let mut combined: Vec<Token> = existing_pattern.to_vec();
        combined.extend(new.iter());

        // Insert as a brand-new vertex via insert_pattern
        self.insert_pattern(combined)
    }

    /// Append tokens to a pattern **in place** (mutates the parent vertex).
    ///
    /// # Safety Contract (enforced via debug_assert)
    ///
    /// The parent vertex MUST:
    /// 1. Have exactly one child pattern
    /// 2. Have no parents (i.e., it is a root/owned vertex)
    ///
    /// Violating these invariants will panic in debug builds and cause
    /// silent corruption in release builds.
    ///
    /// # When to Use
    ///
    /// Only when you have exclusive ownership of the vertex (e.g., a
    /// `RootManager` root that you know has no other references).
    /// Prefer [`extend_root_pattern`] for safety.
    pub fn append_to_owned_pattern(
        &self,
        parent: impl ToToken,
        pattern_id: PatternId,
        new: impl IntoIterator<Item = impl ToToken>,
    ) -> Token {
        let new: Vec<_> = new.into_iter().map(|c| c.to_token()).collect();
        if new.is_empty() {
            return parent.to_token();
        }

        // Debug-mode ownership assertions
        debug_assert!(
            self.with_vertex(parent.vertex_index(), |vertex| {
                vertex.parents().is_empty()
            })
            .unwrap_or(false),
            "append_to_owned_pattern: parent vertex {:?} has parents — use extend_root_pattern instead",
            parent.vertex_index()
        );
        debug_assert!(
            self.with_vertex(parent.vertex_index(), |vertex| {
                vertex.child_patterns().len() == 1
            })
            .unwrap_or(false),
            "append_to_owned_pattern: parent vertex {:?} has multiple child patterns — use extend_root_pattern instead",
            parent.vertex_index()
        );

        let width = pattern_width(&new);

        // Get pattern and update child parents
        let pattern = self
            .with_vertex(parent.vertex_index(), |vertex| {
                vertex.expect_child_pattern(&pattern_id).clone()
            })
            .expect("Parent vertex should exist");

        for c in pattern.into_iter().collect::<HashSet<_>>() {
            self.with_vertex_mut(c.vertex_index(), |node| {
                node.get_parent_mut(parent.vertex_index()).unwrap().width +=
                    width;
            })
            .expect("Child vertex should exist");
        }

        // Update parent vertex
        let (offset, final_width) = self
            .with_vertex_mut(parent.vertex_index(), |vertex| {
                let pattern = vertex.expect_child_pattern_mut(&pattern_id);
                let offset = pattern.len();
                pattern.extend(new.iter());
                *vertex.width_mut() += width.0;
                (offset, vertex.width())
            })
            .expect("Parent vertex should exist");

        let parent = Token::new(parent.vertex_index(), final_width);
        self.add_pattern_parent(parent, new, pattern_id, offset);
        parent
    }
}
