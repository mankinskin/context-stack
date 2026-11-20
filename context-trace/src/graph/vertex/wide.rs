use crate::TokenWidth;
#[cfg(feature = "legacy")]
use crate::{
    Token,
    legacy::graph::vertex::{
        data::VertexData,
        pattern::{
            Pattern,
            pattern_width,
        },
    },
};

pub trait Wide {
    fn width(&self) -> TokenWidth;
}

#[cfg(feature = "legacy")]
impl Wide for Pattern {
    fn width(&self) -> TokenWidth {
        pattern_width(self)
    }
}

#[cfg(feature = "legacy")]
impl Wide for [Token] {
    fn width(&self) -> TokenWidth {
        pattern_width(self)
    }
}

impl Wide for char {
    fn width(&self) -> TokenWidth {
        1.into()
    }
}
//impl<R> Wide for RolePath<R> {
//    fn width(&self) -> TokenWidth {
//        self.width
//    }
//}

impl<T: Wide> Wide for &'_ T {
    fn width(&self) -> TokenWidth {
        (**self).width()
    }
}

impl<T: Wide> Wide for &'_ mut T {
    fn width(&self) -> TokenWidth {
        (**self).width()
    }
}

//impl Wide for OverlapPrimer {
//    fn width(&self) -> TokenWidth {
//        self.width
//    }
//}
#[cfg(feature = "legacy")]
impl Wide for VertexData {
    fn width(&self) -> TokenWidth {
        self.token.width()
    }
}
#[cfg(feature = "legacy")]
impl WideMut for VertexData {
    fn width_mut(&mut self) -> &mut TokenWidth {
        self.token.width_mut()
    }
}

#[allow(dead_code)]
pub(crate) trait WideMut: Wide {
    fn width_mut(&mut self) -> &mut TokenWidth;
}
//impl<P: WideMut> WideMut for OriginPath<P> {
//    fn width_mut(&mut self) -> &mut TokenWidth {
//        self.postfix.width_mut()
//    }
//}
//impl WideMut for OverlapPrimer {
//    fn width_mut(&mut self) -> &mut TokenWidth {
//        &mut self.width
//    }
//}
