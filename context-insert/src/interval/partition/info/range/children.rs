use std::fmt::Debug;

use crate::interval::partition::info::range::{
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
use context_trace::*;

#[derive(Debug, Clone, Copy)]
pub enum InfixChildren {
    Both(Token, Token),
    Left(Token),
    Right(Token),
}

impl InfixChildren {
    pub fn to_joined_pattern(self) -> Result<Pattern, Token> {
        match self {
            InfixChildren::Both(l, r) => Ok([l, r].into_pattern()),
            InfixChildren::Left(c) | InfixChildren::Right(c) => Err(c),
        }
    }
}

pub trait RangeChildren<R: RangeRole>: Debug + Clone {
    fn insert_inner(
        self,
        inner: Option<Token>,
    ) -> Result<Pattern, Token>;
    fn to_token(self) -> Option<Token>;
}

impl<M: PreVisitMode> RangeChildren<Pre<M>> for Token {
    fn insert_inner(
        self,
        inner: Option<Token>,
    ) -> Result<Pattern, Token> {
        if let Some(inner) = inner {
            Ok([inner, self].into_pattern())
        } else {
            Err(self)
        }
    }
    fn to_token(self) -> Option<Token> {
        Some(self)
    }
}

impl<M: PostVisitMode> RangeChildren<Post<M>> for Token {
    fn insert_inner(
        self,
        inner: Option<Token>,
    ) -> Result<Pattern, Token> {
        if let Some(inner) = inner {
            Ok([self, inner].into_pattern())
        } else {
            Err(self)
        }
    }
    fn to_token(self) -> Option<Token> {
        Some(self)
    }
}

impl<M: InVisitMode> RangeChildren<In<M>> for InfixChildren {
    fn insert_inner(
        self,
        inner: Option<Token>,
    ) -> Result<Pattern, Token> {
        if let Some(inner) = inner {
            Ok(match self {
                Self::Both(l, r) => [l, inner, r].into_pattern(),
                Self::Left(l) => [l, inner].into_pattern(),
                Self::Right(r) => [inner, r].into_pattern(),
            })
        } else {
            self.to_joined_pattern()
        }
    }
    fn to_token(self) -> Option<Token> {
        match self {
            Self::Both(_, _) => None,
            Self::Left(c) | Self::Right(c) => Some(c),
        }
    }
}
