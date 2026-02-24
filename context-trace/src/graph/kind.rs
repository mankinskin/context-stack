use crate::{
    direction::{
        Right,
        pattern::PatternDirection,
    },
    graph::vertex::atom::Atomize,
};
use serde::{
    Deserialize,
    Serialize,
    de::DeserializeOwned,
};
use std::fmt::{
    Debug,
    Display,
};

pub trait GraphKind:
    Debug + Clone + Default + PartialEq + Eq + Send + Sync
{
    type Atom: Atomize + Display + DeserializeOwned;
    type Direction: PatternDirection;
}

pub type AtomOf<K> = <K as GraphKind>::Atom;
//pub(crate) type DefaultAtom = AtomOf<BaseGraphKind>;
//pub(crate) type DirectionOf<K> = <K as GraphKind>::Direction;
//pub(crate) type DefaultDirection = DirectionOf<BaseGraphKind>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BaseGraphKind;

impl GraphKind for BaseGraphKind {
    type Atom = char;
    type Direction = Right;
}
