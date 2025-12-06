use std::{
    borrow::Borrow,
    cmp::Ordering,
    fmt::{
        Debug,
        Display,
    },
};

use derive_more::{
    Add,
    AddAssign,
    Deref,
    Display,
    From,
    Sub,
    SubAssign,
    Sum,
};
use serde::{
    Deserialize,
    Serialize,
};

use crate::{
    graph::vertex::{
        PatternId,
        VertexIndex,
        atom::NewAtomIndex,
        has_vertex_index::HasVertexIndex,
        location::{
            SubLocation,
            child::ChildLocation,
            pattern::PatternLocation,
        },
        wide::{
            Wide,
            WideMut,
        },
    },
    trace::cache::key::directed::{
        down::{
            DownKey,
            DownPosition,
        },
        up::{
            UpKey,
            UpPosition,
        },
    },
};

#[derive(
    Debug,
    Clone,
    Copy,
    From,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    Add,
    Sub,
    AddAssign,
    SubAssign,
    Deref,
    Sum,
    Display,
    Default,
)]
pub struct TokenWidth(pub usize);

// Allow comparing TokenWidth with usize directly
impl PartialEq<usize> for TokenWidth {
    fn eq(
        &self,
        other: &usize,
    ) -> bool {
        self.0 == *other
    }
}

impl PartialEq<TokenWidth> for usize {
    fn eq(
        &self,
        other: &TokenWidth,
    ) -> bool {
        *self == other.0
    }
}

impl PartialOrd<usize> for TokenWidth {
    fn partial_cmp(
        &self,
        other: &usize,
    ) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl PartialOrd<TokenWidth> for usize {
    fn partial_cmp(
        &self,
        other: &TokenWidth,
    ) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

// Allow adding/subtracting usize to/from TokenWidth
impl std::ops::Add<usize> for TokenWidth {
    type Output = TokenWidth;
    fn add(
        self,
        rhs: usize,
    ) -> TokenWidth {
        TokenWidth(self.0 + rhs)
    }
}

impl std::ops::Add<TokenWidth> for usize {
    type Output = TokenWidth;
    fn add(
        self,
        rhs: TokenWidth,
    ) -> TokenWidth {
        TokenWidth(self + rhs.0)
    }
}

impl std::ops::Sub<usize> for TokenWidth {
    type Output = TokenWidth;
    fn sub(
        self,
        rhs: usize,
    ) -> TokenWidth {
        TokenWidth(self.0 - rhs)
    }
}

impl std::ops::AddAssign<usize> for TokenWidth {
    fn add_assign(
        &mut self,
        rhs: usize,
    ) {
        self.0 += rhs;
    }
}

impl std::ops::SubAssign<usize> for TokenWidth {
    fn sub_assign(
        &mut self,
        rhs: usize,
    ) {
        self.0 -= rhs;
    }
}

impl Borrow<TokenWidth> for Token {
    fn borrow(&self) -> &TokenWidth {
        &self.width
    }
}

impl crate::logging::compact_format::CompactFormat for TokenWidth {
    fn fmt_compact(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }

    fn fmt_indented(
        &self,
        f: &mut std::fmt::Formatter,
        _indent: usize,
    ) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}


#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubToken {
    pub(crate) token: Token,
    pub(crate) location: SubLocation,
}
pub trait HasToken {
    fn token(&self) -> Token;
}
impl HasToken for SubToken {
    fn token(&self) -> Token {
        self.token
    }
}
pub trait HasSubLocation {
    fn sub_location(&self) -> &SubLocation;
}
impl HasSubLocation for SubToken {
    fn sub_location(&self) -> &SubLocation {
        &self.location
    }
}

#[derive(Eq, Clone, Copy, Serialize, Deserialize)]
pub struct Token {
    pub index: VertexIndex, // the token index
    pub width: TokenWidth,  // the atom width
}

impl Debug for Token {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        // Use Display formatting for Debug to show string representations in test output
        Display::fmt(self, f)
    }
}

impl Token {
    pub fn new(
        index: impl HasVertexIndex,
        width: impl Into<TokenWidth>,
    ) -> Self {
        Self {
            index: index.vertex_index(),
            width: width.into(),
        }
    }

    #[cfg(any(test, feature = "test-api"))]
    pub fn get_string_repr(&self) -> Option<String> {
        crate::graph::test_graph::get_token_string_from_test_graph(*self.index)
    }
    pub fn to_pattern_location(
        self,
        pattern_id: PatternId,
    ) -> PatternLocation {
        PatternLocation::new(self, pattern_id)
    }
    pub fn to_child_location(
        self,
        sub: SubLocation,
    ) -> ChildLocation {
        ChildLocation::new(self, sub.pattern_id, sub.sub_index)
    }
}
#[allow(dead_code)]
impl Token {
    pub(crate) fn get_width(&self) -> usize {
        self.width.0
    }
    pub(crate) fn down_key(
        self,
        pos: impl Into<DownPosition>,
    ) -> DownKey {
        DownKey::new(self, pos.into())
    }
    pub(crate) fn up_key(
        self,
        pos: impl Into<UpPosition>,
    ) -> UpKey {
        UpKey::new(self, pos.into())
    }
}

impl Ord for Token {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.width().cmp(&other.width())
    }
}

impl PartialOrd for Token {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<A: Borrow<Token>, B: Borrow<Token>> From<Result<A, B>> for Token {
    fn from(value: Result<A, B>) -> Self {
        match value {
            Ok(a) => *a.borrow(),
            Err(b) => *b.borrow(),
        }
    }
}

impl std::hash::Hash for Token {
    fn hash<H: std::hash::Hasher>(
        &self,
        h: &mut H,
    ) {
        self.index.hash(h);
    }
}

//impl std::cmp::Ord for Token {
//    fn cmp(
//        &self,
//        other: &Self,
//    ) -> std::cmp::Ordering {
//        self.index.cmp(&other.index)
//    }
//}
impl PartialEq for Token {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.index == other.index
    }
}

impl PartialEq<VertexIndex> for Token {
    fn eq(
        &self,
        other: &VertexIndex,
    ) -> bool {
        self.index == *other
    }
}

impl PartialEq<VertexIndex> for &'_ Token {
    fn eq(
        &self,
        other: &VertexIndex,
    ) -> bool {
        self.index == *other
    }
}

impl PartialEq<VertexIndex> for &'_ mut Token {
    fn eq(
        &self,
        other: &VertexIndex,
    ) -> bool {
        self.index == *other
    }
}

impl<T: Into<Token> + Clone> From<&'_ T> for Token {
    fn from(o: &'_ T) -> Self {
        (*o).clone().into()
    }
}

impl From<NewAtomIndex> for Token {
    fn from(o: NewAtomIndex) -> Self {
        Self::new(o.vertex_index(), 1)
    }
}

impl IntoIterator for Token {
    type Item = Self;
    type IntoIter = std::iter::Once<Token>;
    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self)
    }
}

//impl HasVertexIndex for Token {
//    fn vertex_index(&self) -> VertexIndex {
//        self.index
//    }
//}

impl Wide for Token {
    fn width(&self) -> TokenWidth {
        self.width
    }
}

impl WideMut for Token {
    fn width_mut(&mut self) -> &mut TokenWidth {
        &mut self.width
    }
}

impl Borrow<[Token]> for Token {
    fn borrow(&self) -> &[Token] {
        std::slice::from_ref(self)
    }
}

impl AsRef<[Token]> for Token {
    fn as_ref(&self) -> &[Token] {
        self.borrow()
    }
}
impl Display for Token {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        #[cfg(any(test, feature = "test-api"))]
        {
            if self.get_string_repr().is_some() {
                // Reuse VertexIndex's Display implementation which shows the string representation
                return write!(f, "{}", self.index);
            }
        }
        write!(f, "T{}w{}", self.index, self.width.0)
    }
}

impl crate::logging::compact_format::CompactFormat for Token {
    fn fmt_compact(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        #[cfg(any(test, feature = "test-api"))]
        {
            if let Some(s) = self.get_string_repr() {
                return write!(f, "\"{}\"({})", s, self.index);
            }
        }
        write!(f, "T{}w{}", self.index, self.width.0)
    }

    fn fmt_indented(
        &self,
        f: &mut std::fmt::Formatter,
        _indent: usize,
    ) -> std::fmt::Result {
        self.fmt_compact(f)
    }
}
