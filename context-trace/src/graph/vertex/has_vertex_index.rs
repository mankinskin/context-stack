use crate::graph::vertex::{
    VertexIndex,
    id::{
        StageIndex,
        StagedIndex,
        VertexId,
    },
    token::Token,
    wide::Wide,
};
use std::fmt::Debug;

/// Anything that carries a vertex index (legacy `GetVertexIndex`). The
/// row model uses it with the untyped [`VertexIndex`]
/// (`HasVertexIndex<Index = VertexIndex>`).
pub trait HasVertexIndex: Sized {
    type Index: VertexId;
    fn vertex_index(&self) -> Self::Index;
}

impl<I: HasVertexIndex> HasVertexIndex for &'_ I {
    type Index = I::Index;
    fn vertex_index(&self) -> Self::Index {
        (**self).vertex_index()
    }
}

impl<I: HasVertexIndex> HasVertexIndex for &'_ mut I {
    type Index = I::Index;
    fn vertex_index(&self) -> Self::Index {
        (**self).vertex_index()
    }
}

impl HasVertexIndex for VertexIndex {
    type Index = Self;
    fn vertex_index(&self) -> Self::Index {
        *self
    }
}

impl HasVertexIndex for StagedIndex {
    type Index = Self;
    fn vertex_index(&self) -> Self::Index {
        *self
    }
}

impl HasVertexIndex for StageIndex {
    type Index = Self;
    fn vertex_index(&self) -> Self::Index {
        *self
    }
}

#[cfg(feature = "legacy")]
impl HasVertexIndex for crate::legacy::VertexData {
    type Index = VertexIndex;
    fn vertex_index(&self) -> VertexIndex {
        self.token.vertex_index()
    }
}
impl<I: VertexId> HasVertexIndex for Token<I> {
    type Index = I;
    fn vertex_index(&self) -> I {
        self.index
    }
}

pub trait ToToken: HasVertexIndex + Wide + Debug {
    fn to_token(&self) -> Token<Self::Index> {
        Token::new(self.vertex_index(), self.width())
    }
}

impl<T: HasVertexIndex + Wide + Debug> ToToken for T {}

//pub(crate) trait MaybeIndexed<T: Atomize> {
//    type Inner: HasVertexIndex;
//    fn into_inner(self) -> Result<Self::Inner, T>;
//}
//
//impl<I: HasVertexIndex, T: Atomize> MaybeIndexed<T> for Result<I, T> {
//    type Inner = I;
//    fn into_inner(self) -> Result<Self::Inner, T> {
//        self
//    }
//}
//impl<I: Indexed, T: Atomize> MaybeIndexed<T> for I {
//    type Inner = I;
//    fn into_inner(self) -> Result<Self::Inner, T> {
//        Ok(self)
//    }
//}
