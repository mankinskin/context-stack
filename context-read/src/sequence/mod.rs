pub mod block_iter;

use context_trace::{
    graph::vertex::atom::NewAtomIndices,
    *,
};

use std::{
    fmt::Debug,
    str::Chars,
};

pub trait ToNewAtomIndices: Debug {
    #[allow(non_snake_case)] // Follows NewAtom naming convention
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
//impl<T: Atomize> ToNewTokenIndices<T> for Vec<T> {
//    fn to_new_atom_indices<'a: 'g, 'g, G: HasGraphMut>(
//        self,
//        graph: &'a mut G,
//    ) -> NewTokenIndices {
//        graph.graph_mut().new_atom_indices(self)
//    }
//}

//impl<Iter: IntoIterator<Item = DefaultAtom> + Debug + Send + Sync> ToNewAtomIndices<DefaultAtom>
//    for Iter
//{
//    fn to_new_atom_indices<'a: 'g, 'g, G: HasGraphMut<Kind = BaseGraphKind>>(
//        self,
//        graph: &'a mut G,
//    ) -> NewTokenIndices {
//        graph.graph_mut().new_atom_indices(self)
//    }
//}
