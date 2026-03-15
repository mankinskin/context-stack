use crate::{
    direction::{
        Direction,
        pattern::PatternDirection,
    },
    graph::vertex::{
        pattern::Pattern,
        token::Token,
        wide::Wide,
    },
    trace::has_graph::{
        HasGraph,
        TravDir,
    },
    *,
};
use itertools::{
    FoldWhile,
    Itertools,
};
use std::collections::VecDeque;

// ── Policies ─────────────────────────────────────────────────────────────────

pub trait BandExpandingPolicy<G: HasGraph> {
    fn map_band(
        location: PatternLocation,
        pattern: &Pattern,
    ) -> (ChildLocation, Token);
    fn map_batch(
        batch: impl IntoIterator<Item = (ChildLocation, Token)>
    ) -> Vec<(ChildLocation, Token)> {
        batch.into_iter().collect_vec()
    }
}

#[derive(Debug)]
pub struct PostfixExpandingPolicy<D: PatternDirection> {
    _ty: std::marker::PhantomData<D>,
}

impl<G: HasGraph, D: PatternDirection> BandExpandingPolicy<G>
    for PostfixExpandingPolicy<D>
where
    <D as Direction>::Opposite: PatternDirection,
{
    fn map_band(
        location: PatternLocation,
        pattern: &Pattern,
    ) -> (ChildLocation, Token) {
        let last = D::last_index(pattern);
        (location.to_child_location(last), pattern[last])
    }

    fn map_batch(
        batch: impl IntoIterator<Item = (ChildLocation, Token)>
    ) -> Vec<(ChildLocation, Token)> {
        batch
            .into_iter()
            .sorted_by(|a, b| b.1.width().cmp(&a.1.width()))
            .collect_vec()
    }
}

#[derive(Debug)]
pub struct PrefixExpandingPolicy<D: Direction> {
    _ty: std::marker::PhantomData<D>,
}

impl<G: HasGraph, D: Direction> BandExpandingPolicy<G>
    for PrefixExpandingPolicy<D>
{
    fn map_band(
        location: PatternLocation,
        pattern: &Pattern,
    ) -> (ChildLocation, Token) {
        (location.to_child_location(0), pattern[0])
    }

    fn map_batch(
        batch: impl IntoIterator<Item = (ChildLocation, Token)>
    ) -> Vec<(ChildLocation, Token)> {
        batch
            .into_iter()
            .sorted_by(|a, b| b.1.width().cmp(&a.1.width()))
            .collect_vec()
    }
}

// ── BandIterator trait ────────────────────────────────────────────────────────

pub trait BandIterator<'a, G: HasGraph + 'a>:
    Iterator<Item = (ChildLocation, Token)>
{
    type Policy: BandExpandingPolicy<G>;

    fn band_iter(
        trav: G,
        root: Token,
    ) -> Self;

    fn trav(&self) -> &G;

    /// Get all postfixes/prefixes of `index` with their locations.
    fn next_children(
        &self,
        index: Token,
    ) -> Vec<(ChildLocation, Token)> {
        Self::Policy::map_batch(
            self.trav().graph().expect_child_patterns(index).iter().map(
                |(pid, pattern)| {
                    Self::Policy::map_band(
                        PatternLocation::new(index, *pid),
                        pattern,
                    )
                },
            ),
        )
    }
}

// ── Type aliases ──────────────────────────────────────────────────────────────

pub type PostfixIterator<'a, G> =
    BandExpandingIterator<'a, G, PostfixExpandingPolicy<TravDir<G>>>;

pub type PrefixIterator<'a, G> =
    BandExpandingIterator<'a, G, PrefixExpandingPolicy<TravDir<G>>>;

// ── BandExpandingIterator ─────────────────────────────────────────────────────

#[derive(Debug)]
pub struct BandExpandingIterator<'a, G, P>
where
    G: HasGraph + 'a,
    P: BandExpandingPolicy<G>,
{
    trav: G,
    queue: VecDeque<(ChildLocation, Token)>,
    last: (Option<ChildLocation>, Token),
    _ty: std::marker::PhantomData<&'a P>,
}

impl<'a, G, P> BandIterator<'a, G> for BandExpandingIterator<'a, G, P>
where
    G: HasGraph + 'a,
    P: BandExpandingPolicy<G>,
{
    type Policy = P;

    fn band_iter(
        trav: G,
        root: Token,
    ) -> Self {
        Self {
            trav,
            queue: VecDeque::new(),
            last: (None, root),
            _ty: Default::default(),
        }
    }

    fn trav(&self) -> &G {
        &self.trav
    }
}

impl<G, P> Iterator for BandExpandingIterator<'_, G, P>
where
    G: HasGraph,
    P: BandExpandingPolicy<G>,
{
    type Item = (ChildLocation, Token);

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.next_children(self.last.1);
        if self.queue.is_empty() {
            self.queue.extend(next);
        }
        self.queue.pop_front().map(|(location, node)| {
            self.last.0 = Some(location);
            self.last.1 = node;
            (location, node)
        })
    }
}

// ── HasTokenRoleIters trait ───────────────────────────────────────────────────

pub trait HasTokenRoleIters: ToToken {
    fn postfix_iter<'a, G: HasGraph + 'a>(
        &self,
        trav: G,
    ) -> PostfixIterator<'a, G>
    where
        <TravDir<G> as Direction>::Opposite: PatternDirection,
    {
        PostfixIterator::band_iter(trav, self.to_token())
    }

    fn prefix_iter<'a, G: HasGraph + 'a>(
        &self,
        trav: G,
    ) -> PrefixIterator<'a, G> {
        PrefixIterator::band_iter(trav, self.to_token())
    }

    /// Calculate the prefix path from this token to the given advanced path.
    fn prefix_path<G>(
        &self,
        trav: &G,
        prefix: Token,
    ) -> IndexStartPath
    where
        G: HasGraph + Clone,
    {
        let mut prefix_iter = self.prefix_iter(trav.clone());
        let entry = prefix_iter.next().unwrap().0;

        prefix_iter
            .fold_while(
                RootedRolePath::new_location(entry),
                |mut acc: IndexStartPath, (prefix_location, pre)| {
                    acc.path_append(prefix_location);
                    if pre == prefix {
                        FoldWhile::Done(acc)
                    } else {
                        FoldWhile::Continue(acc)
                    }
                },
            )
            .into_inner()
    }
}

impl<T: ToToken> HasTokenRoleIters for T {}
