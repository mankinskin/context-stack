use crate::graph::{
    Hypergraph,
    getters::{
        ErrorReason,
        vertex::VertexSet,
    },
    kind::GraphKind,
    vertex::{
        IndexPattern,
        VertexIndex,
        atom::{
            AsAtom,
            Atom,
        },
        data::VertexData,
        key::VertexKey,
        pattern::{
            IntoPattern,
            Pattern,
        },
        token::Token,
    },
};

impl<G: GraphKind> Hypergraph<G> {
    #[track_caller]
    pub fn expect_atom_child(
        &self,
        atom: impl AsAtom<G::Atom>,
    ) -> Token {
        Token::new(self.expect_atom_index(atom), 1)
    }
    #[track_caller]
    pub fn expect_atom_children(
        &self,
        atoms: impl IntoIterator<Item = impl AsAtom<G::Atom>>,
    ) -> Pattern {
        self.get_atom_children(atoms)
            .expect("Failed to convert atoms to tokens")
            .into_pattern()
    }
    #[track_caller]
    pub(crate) fn expect_atom_index(
        &self,
        atom: impl AsAtom<G::Atom>,
    ) -> VertexIndex {
        self.get_atom_index(atom).expect("Atom does not exist")
    }
    pub fn get_atom_children(
        &self,
        atoms: impl IntoIterator<Item = impl AsAtom<G::Atom>>,
    ) -> Result<Pattern, ErrorReason> {
        self.to_atom_children_iter(atoms)
            .collect::<Result<Pattern, _>>()
    }
    pub(crate) fn get_atom_index(
        &self,
        atom: impl AsAtom<G::Atom>,
    ) -> Result<VertexIndex, ErrorReason> {
        Ok(self
            .graph
            .get_index_of(&self.get_atom_key(atom.as_atom())?)
            .unwrap()
            .into())
    }
    pub(crate) fn get_atom_key(
        &self,
        atom: impl AsAtom<G::Atom>,
    ) -> Result<VertexKey, ErrorReason> {
        self.atom_keys
            .get(&atom.as_atom())
            .copied()
            .ok_or(ErrorReason::UnknownAtom)
    }
    pub(crate) fn to_atom_index_iter<'a>(
        &'a self,
        atoms: impl IntoIterator<Item = impl AsAtom<G::Atom>> + 'a,
    ) -> impl Iterator<Item = Result<VertexIndex, ErrorReason>> + 'a {
        atoms.into_iter().map(move |atom| self.get_atom_index(atom))
    }
    pub(crate) fn to_atom_children_iter<'a>(
        &'a self,
        atoms: impl IntoIterator<Item = impl AsAtom<G::Atom>> + 'a,
    ) -> impl Iterator<Item = Result<Token, ErrorReason>> + 'a {
        self.to_atom_index_iter(atoms)
            .map(move |r| r.map(|index| Token::new(index, 1)))
    }
    #[allow(dead_code)]
    pub(crate) fn get_atom_data(
        &self,
        atom: &Atom<G::Atom>,
    ) -> Result<&VertexData, ErrorReason> {
        self.get_vertex(self.get_atom_index(atom)?)
    }
    #[allow(dead_code)]
    pub(crate) fn get_atom_data_mut(
        &mut self,
        atom: &Atom<G::Atom>,
    ) -> Result<&mut VertexData, ErrorReason> {
        self.get_vertex_mut(self.get_atom_index(atom)?)
    }
    #[allow(dead_code)]
    pub(crate) fn get_atom_child(
        &self,
        atom: impl AsAtom<G::Atom>,
    ) -> Result<Token, ErrorReason> {
        self.get_atom_index(atom).map(|i| Token::new(i, 1))
    }
    #[allow(dead_code)]
    pub(crate) fn to_atom_keys_iter<'a>(
        &'a self,
        atoms: impl IntoIterator<Item = impl AsAtom<G::Atom>> + 'a,
    ) -> impl Iterator<Item = Result<VertexKey, ErrorReason>> + 'a {
        atoms.into_iter().map(move |atom| self.get_atom_key(atom))
    }
    #[allow(dead_code)]
    pub(crate) fn get_atom_indices(
        &self,
        atoms: impl IntoIterator<Item = impl AsAtom<G::Atom>>,
    ) -> Result<IndexPattern, ErrorReason> {
        let atoms = atoms.into_iter();
        let mut v = IndexPattern::with_capacity(atoms.size_hint().0);
        for atom in atoms {
            let index = self.get_atom_index(atom)?;
            v.push(index);
        }
        Ok(v)
    }
    #[allow(dead_code)]
    pub(crate) fn expect_atom_indices(
        &self,
        atoms: impl IntoIterator<Item = impl AsAtom<G::Atom>>,
    ) -> IndexPattern {
        self.get_atom_indices(atoms)
            .expect("Failed to convert atoms to indices")
    }
}
