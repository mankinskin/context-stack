//! Positioned reader entry point (not yet implemented).

use context_trace::{
    Token,
    positioned::PositionedGraphRef,
};

/// Read `input` into `graph` and return the token of the whole input, or
/// `None` for empty input.
///
/// Target signature of the positioned reader (reader plan Phases 3-5); the
/// tests in `tests/reader_behavior.rs` are ignored until it is implemented.
pub fn read(
    _graph: &PositionedGraphRef,
    _input: impl IntoIterator<Item = char>,
) -> Option<Token> {
    unimplemented!("positioned reader pending")
}
