//! Store-generic structural checks, shared by `PositionedGraph::validate`
//! and `GraphEdit` (written once for every store, staging plan §3.5).

#[cfg(any(debug_assertions, test, feature = "test-api", feature = "serde"))]
use crate::positioned::store::Navigate;
use crate::{
    Token,
    TokenWidth,
    graph::vertex::{
        has_vertex_index::{
            HasVertexIndex,
            ToToken,
        },
        id::Plain,
        wide::Wide,
    },
    positioned::{
        model::{
            Composition,
            Occurrence,
            PositionedError,
        },
        store::{
            CompositionRead,
            GraphRead,
            NodeRef,
            VertexRead,
        },
    },
};

/// The vertex `index`, or `UnknownVertex`.
pub(crate) fn expect_vertex<S: GraphRead + ?Sized>(
    graph: &S,
    index: S::Index,
) -> Result<S::Vertex<'_>, PositionedError<S::Index>> {
    graph
        .vertex(index)
        .ok_or(PositionedError::UnknownVertex(index))
}

/// The token of a known vertex.
///
/// # Panics
/// If `index` is unknown.
pub(crate) fn expect_token<S: GraphRead + ?Sized>(
    graph: &S,
    index: S::Index,
) -> Token<S::Index> {
    graph
        .vertex(index)
        .unwrap_or_else(|| panic!("unknown vertex {}", Plain(index)))
        .to_token()
}

/// The child of `occurrence` exists and has the recorded width.
pub(crate) fn check_child<S: GraphRead + ?Sized>(
    graph: &S,
    occurrence: &Occurrence<S::Index>,
) -> Result<(), PositionedError<S::Index>> {
    let child = expect_vertex(graph, occurrence.child.index)?;
    if child.width() != occurrence.child.width {
        return Err(PositionedError::Width {
            vertex: child.vertex_index(),
            expected: child.width(),
            found: occurrence.child.width,
        });
    }
    Ok(())
}

/// Structure, width and reciprocal links of `vertex`.
pub(crate) fn validate_vertex<'a, V: VertexRead<'a>>(
    vertex: V
) -> Result<(), PositionedError<V::Index>> {
    let index = vertex.vertex_index();
    let resolve = |i: V::Index| {
        vertex.resolve(i).ok_or(PositionedError::UnknownVertex(i))
    };
    match vertex.node() {
        NodeRef::Atom(_) =>
            if vertex.width().0 != 1 {
                return Err(PositionedError::Width {
                    vertex: index,
                    expected: TokenWidth(1),
                    found: vertex.width(),
                });
            },
        NodeRef::Composite(composition) => {
            let error = |error| PositionedError::Composition {
                owner: Some(index),
                error,
            };
            // Order is guaranteed by construction; re-check the rest.
            Composition::new(composition.occurrences().collect())
                .map_err(error)?;
            if composition.width() != vertex.width() {
                return Err(PositionedError::Width {
                    vertex: index,
                    expected: composition.width(),
                    found: vertex.width(),
                });
            }
            for occurrence in composition.occurrences() {
                let child = resolve(occurrence.child.index)?;
                if child.width() != occurrence.child.width {
                    return Err(PositionedError::Width {
                        vertex: child.vertex_index(),
                        expected: child.width(),
                        found: occurrence.child.width,
                    });
                }
                if !child.is_parent_at(index, occurrence.offset) {
                    return Err(PositionedError::ParentLink {
                        parent: index,
                        child: occurrence.child.index,
                        offset: occurrence.offset,
                    });
                }
            }
        },
    }
    for (parent, offset) in vertex.parents() {
        let present =
            resolve(parent.index)?.node().composite().is_some_and(|c| {
                c.occurrences()
                    .any(|o| o.offset == offset && o.child.index == index)
            });
        if !present {
            return Err(PositionedError::ParentLink {
                parent: parent.index,
                child: index,
                offset,
            });
        }
    }
    Ok(())
}

/// Structure, widths and reciprocal links of every vertex of `store`, and
/// G4 for every composite (debug and `test-api` builds): the checks of
/// `PositionedGraph::validate` that need no content traversal, for any
/// store (also an open stage). Also run on every deserialized graph
/// (feature `serde`), in every build.
#[cfg(any(test, feature = "test-api", feature = "serde"))]
pub fn validate_store<S: GraphRead + ?Sized>(
    store: &S
) -> Result<(), PositionedError<S::Index>> {
    for vertex in store.vertices() {
        validate_vertex(vertex)?;
        if let NodeRef::Composite(composition) = vertex.node() {
            check_overlap_closure(
                store,
                vertex.vertex_index(),
                composition.occurrences(),
            )?;
        }
    }
    Ok(())
}

/// G4, structurally: for adjacent overlapping `L@l`, `R@r`, the largest
/// token starting at `r − l` in `L` ends at `|L|`, i.e. the overlap is a
/// vertex. Non-adjacent pairs are not checked.
#[cfg(any(debug_assertions, test, feature = "test-api", feature = "serde"))]
pub(crate) fn check_overlap_closure<S: GraphRead + ?Sized>(
    graph: &S,
    owner: S::Index,
    occurrences: impl IntoIterator<Item = Occurrence<S::Index>>,
) -> Result<(), PositionedError<S::Index>> {
    let mut occurrences = occurrences.into_iter();
    let Some(mut left) = occurrences.next() else {
        return Ok(());
    };
    for right in occurrences {
        let left = std::mem::replace(&mut left, right);
        if right.offset >= left.end() {
            continue;
        }
        let start = right.offset.0 - left.offset.0;
        let closed = graph
            .vertex(left.child.index)
            .and_then(|vertex| vertex.start_at(start))
            .is_some_and(|(token, _)| {
                start + token.width.0 == left.child.width.0
            });
        if !closed {
            return Err(PositionedError::OverlapNotVertex {
                owner,
                left,
                right,
            });
        }
    }
    Ok(())
}

/// Children's labels joined in order, each from the first position not
/// covered by the previous occurrences. Assumes one `char` per atom label
/// (true for `char` atoms).
#[cfg(debug_assertions)]
pub(crate) fn composed_label<S: GraphRead + ?Sized>(
    graph: &S,
    occurrences: impl IntoIterator<Item = Occurrence<S::Index>>,
) -> String {
    let mut label = String::new();
    let mut end: usize = 0;
    for occurrence in occurrences {
        let child = graph
            .vertex(occurrence.child.index)
            .expect("checked child")
            .label();
        let skip = end.saturating_sub(occurrence.offset.0);
        label.extend(child.chars().skip(skip));
        end = end.max(occurrence.end().0);
    }
    label
}
