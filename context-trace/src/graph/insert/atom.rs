//! Atom insertion and management operations

use crate::{
    Hypergraph,
    TokenWidth,
    graph::{
        kind::GraphKind,
        vertex::{
            atom::{
                Atom,
                NewAtomIndex,
                NewAtomIndices,
            },
            data::{
                VertexData,
                VertexDataBuilder,
            },
            key::VertexKey,
            token::Token,
        },
    },
};

impl<G: GraphKind> Hypergraph<G> {
    fn insert_atom_key(
        &mut self,
        atom: Atom<G::Atom>,
        key: VertexKey,
    ) {
        self.atoms.insert(key, atom);
        self.atom_keys.insert(atom, key);
    }

    /// Insert raw vertex data for an atom
    pub(crate) fn insert_atom_data(
        &mut self,
        atom: Atom<G::Atom>,
        data: VertexData,
    ) -> Token {
        self.insert_atom_key(atom, data.key);
        self.insert_vertex_data(data)
    }

    /// Insert single atom node
    pub fn insert_atom(
        &mut self,
        atom: Atom<G::Atom>,
    ) -> Token {
        let data = VertexData::new(self.next_vertex_index(), TokenWidth(1));
        self.insert_atom_data(atom, data)
    }

    /// Insert multiple atom nodes
    pub fn insert_atoms(
        &mut self,
        atoms: impl IntoIterator<Item = Atom<G::Atom>>,
    ) -> Vec<Token> {
        atoms
            .into_iter()
            .map(|atom| self.insert_atom(atom))
            .collect()
    }
}

#[allow(dead_code)]
impl<G: GraphKind> Hypergraph<G> {
    pub(crate) fn insert_atom_builder(
        &mut self,
        atom: Atom<G::Atom>,
        builder: VertexDataBuilder,
    ) -> Token {
        let data = self.finish_vertex_builder(builder);
        self.insert_atom_data(atom, data)
    }

    pub(crate) fn new_atom_indices(
        &mut self,
        sequence: impl IntoIterator<Item = G::Atom>,
    ) -> NewAtomIndices {
        sequence
            .into_iter()
            .map(Atom::Element)
            .map(|t| match self.get_atom_index(t) {
                Ok(i) => NewAtomIndex::Known(i),
                Err(_) => {
                    let i = self.insert_atom(t);
                    NewAtomIndex::New(i.index)
                },
            })
            .collect()
    }
}
