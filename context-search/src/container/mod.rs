use crate::state::TraversalState;
use std::fmt::Debug;

pub mod bft;
//pub(crate) mod dft;
pub(crate) mod order;
//pub(crate) mod pruning;

pub trait StateContainer:
    Iterator<Item = (usize, TraversalState)>
    + Default
    + Debug
    + FromIterator<(usize, TraversalState)>
{
}
