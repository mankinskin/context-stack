use crate::{
    PositionAnnotated,
    graph::vertex::{
        location::{
            child::ChildLocation,
            pattern::{
                IntoPatternLocation,
                PatternLocation,
            },
        },
        pattern::Pattern,
        token::Token,
    },
    path::structs::rooted::root::IndexRoot,
    trace::has_graph::HasGraph,
};

pub trait GraphRootPattern: GraphRoot + RootPattern {
    fn root_pattern_location(&self) -> PatternLocation;
    fn graph_root_pattern<'a: 'g, 'g, G: HasGraph + 'a>(
        &self,
        trav: &'g G::Guard<'a>,
    ) -> Pattern {
        trav.expect_pattern_at(self.root_pattern_location())
    }
}
impl GraphRootPattern for PatternLocation {
    fn root_pattern_location(&self) -> PatternLocation {
        *self
    }
}
impl<T: GraphRootPattern> GraphRootPattern for PositionAnnotated<T> {
    fn root_pattern_location(&self) -> PatternLocation {
        self.node.root_pattern_location()
    }
}
impl GraphRootPattern for ChildLocation {
    fn root_pattern_location(&self) -> PatternLocation {
        self.into_pattern_location()
    }
}
pub trait GraphRoot {
    fn root_parent(&self) -> Token;
}
impl GraphRoot for PatternLocation {
    fn root_parent(&self) -> Token {
        self.parent
    }
}
impl GraphRoot for ChildLocation {
    fn root_parent(&self) -> Token {
        self.parent
    }
}
impl<T: GraphRoot> GraphRoot for PositionAnnotated<T> {
    fn root_parent(&self) -> Token {
        self.node.root_parent()
    }
}

pub trait PatternRoot {
    fn pattern_root_pattern(&self) -> &Pattern;
}

#[macro_export]
macro_rules! impl_root {
    {
        $(< $( $par:ident $( : $bhead:tt $( + $btail:tt )*)? ),* >)? RootPattern for $target:ty, $self_:ident, $trav:ident => $func:expr
    } => {
        impl <$( $( $par $(: $bhead $( + $btail )* )? ),* )?> $crate::RootPattern for $target {
            fn root_pattern<
                'a: 'g,
                'b: 'g,
                'g,
                G: $crate::HasGraph + 'a
            >(&'b $self_, $trav: &'g G::Guard<'a>) -> $crate::Pattern {
                $func
            }
        }
    };
    {
        $(< $( $par:ident $( : $bhead:tt $( + $btail:tt )*)? ),* >)? PatternRoot for $target:ty, $self_:ident => $func:expr
    } => {
        impl <$( $( $par $(: $bhead $( + $btail )* )? ),* )?> $crate::PatternRoot for $target {
            fn pattern_root_pattern(& $self_) -> &Pattern {
                $func
            }
        }
    };
    {
        $(< $( $par:ident $( : $bhead:tt $( + $btail:tt )*)? ),* >)? GraphRootPattern for $target:ty, $self_:ident => $func:expr
    } => {
        impl <$( $( $par $(: $bhead $( + $btail )* )? ),* )?> GraphRootPattern for $target {
            fn root_pattern_location(& $self_) -> $crate::PatternLocation {
                $func
            }
        }
    };
    {
        $(< $( $par:ident $( : $bhead:tt $( + $btail:tt )*)? ),* >)? GraphRoot for $target:ty, $self_:ident => $func:expr
    } => {
        impl <$( $( $par $(: $bhead $( + $btail )* )? ),* )?> $crate::GraphRoot for $target {
            fn root_parent(& $self_) -> $crate::Token {
                $func
            }
        }
    }
}

pub trait RootPattern {
    fn root_pattern<'a: 'g, 'b: 'g, 'g, G: HasGraph + 'a>(
        &'b self,
        trav: &'g G::Guard<'a>,
    ) -> Pattern;
}
impl_root! { RootPattern for ChildLocation, self, trav => GraphRootPattern::graph_root_pattern::<G>(self, trav) }
impl_root! { RootPattern for PatternLocation, self, trav => GraphRootPattern::graph_root_pattern::<G>(self, trav) }
impl_root! { RootPattern for IndexRoot, self, trav => self.location.root_pattern::<G>(trav) }
impl_root! { RootPattern for Pattern, self, _trav => self.clone() }
impl_root! { <T: RootPattern> RootPattern for PositionAnnotated<T>, self, trav => self.node.root_pattern::<G>(trav) }
