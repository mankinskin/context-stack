use crate::{
    graph::vertex::parent::PatternIndex,
    *,
};

#[macro_use]
#[cfg(test)]
pub mod graph;

pub mod env;

pub mod macros;

pub mod test_case;

#[cfg(test)]
pub mod grammar;

#[cfg(test)]
pub mod public_api;

#[cfg(test)]
pub mod path_advance;

//#[cfg(test)]
//pub mod state_advance;

#[cfg(test)]
pub mod tracing_demo;

#[cfg(test)]
pub mod compact_format_demo;

#[cfg(test)]
pub mod test_string_repr;

#[cfg(test)]
pub mod test_env1_string_repr;

pub fn assert_parents(
    graph: &Hypergraph,
    token: impl ToToken,
    parent: impl ToToken,
    pattern_indices: impl IntoIterator<Item = PatternIndex>,
) {
    assert_eq!(
        graph
            .expect_parents(token)
            .clone()
            .into_iter()
            .collect::<HashMap<_, _>>(),
        HashMap::from_iter([(
            parent.vertex_index(),
            Parent {
                pattern_indices: pattern_indices.into_iter().collect(),
                width: parent.width(),
            }
        )])
    );
}
