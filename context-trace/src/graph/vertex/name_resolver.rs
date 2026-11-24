//! Model-neutral hook for human-readable token names in debug output.
//!
//! `Token` and `VertexIndex` display a token's string representation when a
//! resolver is registered for the current thread. Graph models (e.g. the
//! legacy test graph registry) install a resolver; the display code does not
//! depend on any particular graph model.
//!
//! Uses thread-local storage so parallel tests do not interfere.

use std::cell::RefCell;

/// Resolves a vertex index to the string representation of its token.
pub trait TokenNameResolver: Send + Sync {
    fn token_name(
        &self,
        index: usize,
    ) -> Option<String>;
    /// The name of the staged vertex in `slot` of the current stage.
    fn staged_token_name(
        &self,
        _slot: usize,
    ) -> Option<String> {
        None
    }
}

thread_local! {
    static RESOLVER: RefCell<Option<Box<dyn TokenNameResolver>>> = RefCell::new(None);
}

/// Install `resolver` for the current thread, replacing any previous one.
pub fn set_token_name_resolver(resolver: Box<dyn TokenNameResolver>) {
    RESOLVER.with(|r| {
        *r.borrow_mut() = Some(resolver);
    });
}

/// Resolve the string representation of `index` with the current thread's resolver.
pub fn resolve_token_name(index: usize) -> Option<String> {
    RESOLVER.with(|r| {
        r.borrow()
            .as_ref()
            .and_then(|resolver| resolver.token_name(index))
    })
}

/// Resolve the name of staged slot `slot` with the current thread's resolver.
pub fn resolve_staged_token_name(slot: usize) -> Option<String> {
    RESOLVER.with(|r| {
        r.borrow()
            .as_ref()
            .and_then(|resolver| resolver.staged_token_name(slot))
    })
}

/// Remove the current thread's resolver.
pub fn clear_token_name_resolver() {
    RESOLVER.with(|r| {
        *r.borrow_mut() = None;
    });
}
