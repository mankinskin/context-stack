pub use context_trace::graph::vertex::wide::*;
use crate::{Token, TokenWidth, graph::vertex::{data::VertexData, pattern::{Pattern, pattern_width}}};

pub trait WideMut: Wide {
    fn width_mut(&mut self) -> &mut TokenWidth;
}
impl<I> WideMut for context_trace::Token<I> {
    fn width_mut(&mut self) -> &mut TokenWidth {
        &mut self.width
    }
}
impl Wide for Pattern {
    fn width(&self) -> TokenWidth {
        pattern_width(self)
    }
}
impl Wide for VertexData {
    fn width(&self) -> TokenWidth {
        self.token.width()
    }
}
impl WideMut for VertexData {
    fn width_mut(&mut self) -> &mut TokenWidth {
        self.token.width_mut()
    }
}
#[allow(unused)]
fn _token(_: Token) {}
