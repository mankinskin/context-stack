use derive_more::{
    Display,
    From,
};
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;
#[derive(
    Hash,
    Debug,
    PartialEq,
    Eq,
    From,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Display,
)]
pub struct VertexKey(Uuid);
impl Default for VertexKey {
    fn default() -> Self {
        Self(Uuid::new_v4())
    }
}
//#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, new, Serialize, Deserialize)]
//pub(crate) enum VertexKey<T: Atomize = AtomOf<BaseGraphKind>> {
//    Pattern(Token),
//    Atom(Atom<T>, VertexIndex)
//}
//impl<T: Atomize> HasVertexIndex for VertexKey<T> {
//    fn vertex_index(&self) -> VertexIndex {
//        match self {
//            Self::Atom(_atom, index) => *index,
//            Self::Pattern(token) => token.vertex_index(),
//        }
//    }
//}
//impl<T: Atomize> Borrow<VertexIndex> for VertexKey<T> {
//    fn borrow(&self) -> &VertexIndex {
//        match self {
//            Self::Atom(_atom, index) => index,
//            Self::Pattern(token) => &token.index,
//        }
//    }
//}
