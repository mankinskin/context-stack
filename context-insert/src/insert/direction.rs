use context_trace::*;

#[allow(dead_code)]
pub trait InsertDirection: Direction + Clone + PartialEq + Eq {
    fn context_then_inner(
        context: Pattern,
        inner: Token,
    ) -> Pattern
    where
        Self::Opposite: InsertDirection,
    {
        <<Self as Direction>::Opposite as InsertDirection>::inner_then_context(
            inner, context,
        )
    }

    fn inner_then_context(
        inner: Token,
        context: Pattern,
    ) -> Pattern;

    //fn split_context_head(context: impl Merge) -> Option<(Token, Pattern)>;
    //fn split_last(context: impl Merge) -> Option<(Pattern, Token)> {
    //    <Self as InsertDirection>::Opposite::split_context_head(context).map(|(c, rem)| (rem, c))
    //}
    //fn split_inner_head(context: impl Merge) -> (Token, Pattern) {
    //    <Self as InsertDirection>::Opposite::split_context_head(context)
    //        .expect("Empty inner pattern!")
    //}
    //// first inner, then context
    //// first context, then inner
    //fn merge_order(
    //    inner: Token,
    //    head: Token,
    //) -> (Token, Token);
    //fn inner_context_range(
    //    back: usize,
    //    front: usize,
    //) -> Range<usize>;
    //fn wrapper_range(
    //    back: usize,
    //    front: usize,
    //) -> RangeInclusive<usize>;
    //fn concat_context_inner_context(
    //    head_context: Token,
    //    inner: impl IntoPattern,
    //    last_context: Token,
    //) -> Pattern;
}

impl InsertDirection for Left {
    fn inner_then_context(
        inner: Token,
        context: Pattern,
    ) -> Pattern {
        context.iter().copied().chain(inner).collect()
    }

    //fn split_context_head(context: impl Merge) -> Option<(Token, Pattern)> {
    //    context.split_back()
    //}
    //fn merge_order(
    //    inner: Token,
    //    head: Token,
    //) -> (Token, Token) {
    //    (head, inner)
    //}
    //fn inner_context_range(
    //    back: usize,
    //    front: usize,
    //) -> Range<usize> {
    //    Self::index_prev(front).unwrap()..back
    //}
    //fn wrapper_range(
    //    back: usize,
    //    front: usize,
    //) -> RangeInclusive<usize> {
    //    front..=back
    //}
    //fn concat_context_inner_context(
    //    head_context: Token,
    //    inner: impl IntoPattern,
    //    last_context: Token,
    //) -> Pattern {
    //    std::iter::once(last_context)
    //        .chain(inner.borrow().to_owned())
    //        .chain(std::iter::once(head_context))
    //        .collect()
    //}
}

impl InsertDirection for Right {
    fn inner_then_context(
        inner: Token,
        context: Pattern,
    ) -> Pattern {
        std::iter::once(inner).chain(context.to_owned()).collect()
    }

    //fn split_context_head(context: impl Merge) -> Option<(Token, Pattern)> {
    //    context.split_front()
    //}
    //fn merge_order(
    //    inner: Token,
    //    head: Token,
    //) -> (Token, Token) {
    //    (inner, head)
    //}
    //fn concat_context_inner_context(
    //    head_context: Token,
    //    inner: impl IntoPattern,
    //    last_context: Token,
    //) -> Pattern {
    //    std::iter::once(head_context)
    //        .chain(inner.borrow().to_owned())
    //        .chain(std::iter::once(last_context))
    //        .collect()
    //}
    //fn inner_context_range(
    //    back: usize,
    //    front: usize,
    //) -> Range<usize> {
    //    Self::index_next(back).unwrap()..front
    //}
    //fn wrapper_range(
    //    back: usize,
    //    front: usize,
    //) -> RangeInclusive<usize> {
    //    back..=front
    //}
}
