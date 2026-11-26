use crate::{Token, TokenWidth};

impl Wide for [Token] {
    fn width(&self) -> TokenWidth {
        self.iter().map(Wide::width).sum()
    }
}

pub trait Wide {
    fn width(&self) -> TokenWidth;
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
