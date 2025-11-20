use std::{
    collections::VecDeque,
    iter::{
        Extend,
        FusedIterator,
    },
};

#[derive(Clone)]
pub(crate) struct Bft<T, F, I>
    where
        T: Sized,
        F: FnMut(&T) -> I,
        I: Iterator<Item=T>,
{
    queue: VecDeque<(usize, T)>,
    iter_children: F,
}

impl<T, F, I> Bft<T, F, I>
    where
        T: Sized,
        F: FnMut(&T) -> I,
        I: Iterator<Item=T>,
{
    #[inline]
    pub(crate) fn new(
        root: T,
        iter_children: F,
    ) -> Self {
        Self {
            queue: VecDeque::from(vec![(0, root)]),
            iter_children,
        }
    }
}

impl<T, F, I> Iterator for Bft<T, F, I>
    where
        T: Sized,
        F: FnMut(&T) -> I,
        I: Iterator<Item=T>,
{
    type Item = (usize, T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some((depth, node)) = self.queue.pop_front() {
            let tokens = (self.iter_children)(&node);
            self.queue.extend(tokens.map(|token| (depth + 1, token)));

            Some((depth, node))
        } else {
            None
        }
    }
}

impl<T, F, I> FusedIterator for Bft<T, F, I>
    where
        T: Sized,
        F: FnMut(&T) -> I,
        I: Iterator<Item=T>,
{}

pub(crate)(crate) trait HasGraph {
    type Node;
    type State;
}

pub(crate)(crate) trait BreadthFirstTraversal<'g> {
    type Trav: HasGraph;
    fn end_op(state: <Self::Trav as HasGraph>::State) -> Vec<<Self::Trav as HasGraph>::Node>;
}
