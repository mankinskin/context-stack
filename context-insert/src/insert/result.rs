use std::{
    borrow::Borrow,
    fmt::Debug,
};

use context_search::*;
use context_trace::*;

pub trait ResultExtraction {
    fn extract_from(state: &Response) -> Self;
}
impl ResultExtraction for () {
    fn extract_from(_: &Response) -> Self {}
}
impl ResultExtraction for PatternRangePath {
    fn extract_from(state: &Response) -> Self {
        state.query_cursor().path().clone()
    }
}
pub trait TryInitWith<T>: Sized {
    type Error: Into<ErrorState>;
    fn try_init(value: T) -> Result<Self, Self::Error>;
}
impl TryInitWith<IndexWithPath> for Token {
    type Error = IndexWithPath;
    fn try_init(value: IndexWithPath) -> Result<Self, Self::Error> {
        Ok(value.index)
    }
}
impl TryInitWith<IndexWithPath> for IndexWithPath {
    type Error = IndexWithPath;
    fn try_init(value: IndexWithPath) -> Result<Self, Self::Error> {
        Err(value)
    }
}
pub trait InsertResult:
    Debug
    + Borrow<Token>
    + TryInitWith<IndexWithPath>
    //+ TryInitWith<ErrorState, Error = Token>
    + Into<Token>
{
    type Extract: ResultExtraction;
    fn build_with_extract(
        root: Token,
        ext: Self::Extract,
    ) -> Self;
}
impl InsertResult for Token {
    type Extract = ();
    fn build_with_extract(
        root: Token,
        _: Self::Extract,
    ) -> Self {
        root
    }
}
impl InsertResult for IndexWithPath {
    type Extract = PatternRangePath;
    fn build_with_extract(
        root: Token,
        ext: Self::Extract,
    ) -> Self {
        Self {
            index: root,
            path: ext,
        }
    }
}
//impl<T, A: TryFrom<T>> TryInitWith<T> for A {
//    type Error = <A as TryFrom<T>>::Error;
//    fn try_init(value: T) -> Result<Self, Self::Error> {
//        Self::try_from(value)
//    }
//}
//impl TryInitWith<ErrorState> for Token {
//    type Error = Token;
//    fn try_init(value: ErrorState) -> Result<Self, Self::Error> {
//        match value {
//            ErrorState {
//                reason: ErrorReason::SingleIndex(c),
//                found: Some(ResponseKind::Complete(_)),
//            } => Ok(c),
//            ErrorState {
//                reason: ErrorReason::SingleIndex(c),
//                found: Some(ResponseKind::Complete(_)),
//            } => Ok(c),
//        }
//    }
//}
//impl TryInitWith<ErrorState> for IndexWithPath {
//    type Error = Token;
//    fn try_init(value: ErrorState) -> Result<Self, Self::Error> {
//        Err(value)
//    }
//}

//#[derive(Debug, Clone)]
//pub(crate) struct IndexSplitResult {
//    pub(crate) inner: Token,
//    pub(crate) location: ChildLocation,
//    pub(crate) path: Vec<ChildLocation>,
//}
