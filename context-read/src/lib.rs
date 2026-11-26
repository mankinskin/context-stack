#![deny(clippy::disallowed_methods)]

mod expand;
mod forward;
mod read;
mod root;
mod segment;
pub use read::{
    Commit,
    Step,
    StepKind,
    read,
    read_observed,
    read_traced,
};

/// Row-model reader, fenced off from the model-agnostic root.
#[cfg(feature = "legacy")]
pub mod legacy;

#[cfg(test)]
mod tests {
    mod observed;
    mod steps;

    use std::collections::BTreeMap;

    use context_trace::{
        Token,
        positioned::{
            PositionedGraphRef,
            fixtures::fixture_graphs,
        },
    };

    /// The v3 fixture graph `name` and its vertex names.
    pub(crate) fn fixture(name: &str) -> (PositionedGraphRef, BTreeMap<String, Token>) {
        let fixture = fixture_graphs()
            .into_iter()
            .find(|fixture| fixture.name == name)
            .unwrap_or_else(|| panic!("no fixture {}", name));
        (PositionedGraphRef::new(fixture.graph), fixture.names)
    }
}
