use std::fmt::Debug;

use crate::legacy::interval::partition::info::range::{
    mode::{
        InVisitMode,
        PostVisitMode,
        PreVisitMode,
    },
    role::{
        In,
        Post,
        Pre,
        RangeRole,
    },
};
use crate::legacy::join::plan::JoinRef;

#[derive(Debug, Clone, Copy)]
pub(crate) enum InfixChildren {
    Both(JoinRef, JoinRef),
    Left(JoinRef),
    Right(JoinRef),
}

impl InfixChildren {
    pub(crate) fn into_joined_pattern(self) -> Result<Vec<JoinRef>, JoinRef> {
        match self {
            InfixChildren::Both(l, r) => Ok(vec![l, r]),
            InfixChildren::Left(c) | InfixChildren::Right(c) => Err(c),
        }
    }
}

pub(crate) trait RangeChildren<R: RangeRole>: Debug + Clone {
    fn insert_inner(
        self,
        inner: Option<JoinRef>,
    ) -> Result<Vec<JoinRef>, JoinRef>;
    fn to_token(self) -> Option<JoinRef>;
}

impl<M: PreVisitMode> RangeChildren<Pre<M>> for JoinRef {
    fn insert_inner(
        self,
        inner: Option<JoinRef>,
    ) -> Result<Vec<JoinRef>, JoinRef> {
        if let Some(inner) = inner {
            Ok(vec![inner, self])
        } else {
            Err(self)
        }
    }
    fn to_token(self) -> Option<JoinRef> {
        Some(self)
    }
}

impl<M: PostVisitMode> RangeChildren<Post<M>> for JoinRef {
    fn insert_inner(
        self,
        inner: Option<JoinRef>,
    ) -> Result<Vec<JoinRef>, JoinRef> {
        if let Some(inner) = inner {
            Ok(vec![self, inner])
        } else {
            Err(self)
        }
    }
    fn to_token(self) -> Option<JoinRef> {
        Some(self)
    }
}

impl<M: InVisitMode> RangeChildren<In<M>> for InfixChildren {
    fn insert_inner(
        self,
        inner: Option<JoinRef>,
    ) -> Result<Vec<JoinRef>, JoinRef> {
        if let Some(inner) = inner {
            Ok(match self {
                Self::Both(l, r) => vec![l, inner, r],
                Self::Left(l) => vec![l, inner],
                Self::Right(r) => vec![inner, r],
            })
        } else {
            self.into_joined_pattern()
        }
    }
    fn to_token(self) -> Option<JoinRef> {
        match self {
            Self::Both(_, _) => None,
            Self::Left(c) | Self::Right(c) => Some(c),
        }
    }
}
