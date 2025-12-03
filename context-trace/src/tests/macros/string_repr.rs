//! String representation testing utilities for tokens and vertices.
//!
//! Provides functions and macros to test that token string representations are unique
//! and match expected values, without relying on exact vertex indices.

use crate::{
    Hypergraph,
    HypergraphRef,
    graph::{
        kind::GraphKind,
        vertex::{VertexIndex, token::Token},
        getters::vertex::VertexSet,
    },
};
use std::collections::HashMap;

/// Check if a token's string representation matches the expected string
///
/// # Arguments
/// * `graph` - The hypergraph to check
/// * `token` - The token to check
/// * `expected_str` - The expected string representation
///
/// # Returns
/// `true` if the token's string representation matches the expected string, `false` otherwise
///
/// # Example
/// ```ignore
/// let token = graph.insert_pattern(vec![a, b, c]);
/// assert!(check_token_string_repr(&graph, token, "abc"));
/// ```
pub fn check_token_string_repr<G: GraphKind>(
    graph: &Hypergraph<G>,
    token: Token,
    expected_str: &str,
) -> bool
where
    G::Atom: std::fmt::Display,
{
    let actual_str = Hypergraph::<G>::index_string(graph, token.index);
    actual_str == expected_str
}

/// Assert that a token's string representation matches the expected string
///
/// # Panics
/// Panics if the token's string representation doesn't match the expected string
///
/// # Example
/// ```ignore
/// let token = graph.insert_pattern(vec![a, b, c]);
/// assert_token_string_repr(&graph, token, "abc");
/// ```
pub fn assert_token_string_repr<G: GraphKind>(
    graph: &Hypergraph<G>,
    token: Token,
    expected_str: &str,
)
where
    G::Atom: std::fmt::Display,
{
    let actual_str = Hypergraph::<G>::index_string(graph, token.index);
    assert_eq!(
        actual_str, expected_str,
        "Token string representation mismatch for token {:?}\n\
        Expected: '{}'\n\
        Actual: '{}'",
        token, expected_str, actual_str
    );
}

/// Check if all vertices in a hypergraph have unique string representations
///
/// # Arguments
/// * `graph` - The hypergraph to check
///
/// # Returns
/// A tuple of:
/// * `bool` - `true` if all string representations are unique, `false` otherwise
/// * `Vec<(String, Vec<VertexIndex>)>` - List of duplicate strings with their vertex indices
///
/// # Example
/// ```ignore
/// let (is_unique, duplicates) = check_all_vertices_unique(&graph);
/// assert!(is_unique, "Found duplicate string representations: {:?}", duplicates);
/// ```
pub fn check_all_vertices_unique<G: GraphKind>(
    graph: &Hypergraph<G>,
) -> (bool, Vec<(String, Vec<VertexIndex>)>)
where
    G::Atom: std::fmt::Display,
{
    let mut string_to_indices: HashMap<String, Vec<VertexIndex>> = HashMap::new();
    
    // Collect all vertices and their string representations
    for index in 0..graph.vertex_count() {
        let vertex_index = VertexIndex::from(index);
        if graph.get_vertex(vertex_index).is_ok() {
            let string_repr = Hypergraph::<G>::index_string(graph, vertex_index);
            string_to_indices
                .entry(string_repr)
                .or_insert_with(Vec::new)
                .push(vertex_index);
        }
    }
    
    // Find duplicates
    let duplicates: Vec<(String, Vec<VertexIndex>)> = string_to_indices
        .into_iter()
        .filter(|(_, indices)| indices.len() > 1)
        .collect();
    
    let is_unique = duplicates.is_empty();
    (is_unique, duplicates)
}

/// Assert that all vertices in a hypergraph have unique string representations
///
/// # Panics
/// Panics if any duplicate string representations are found
///
/// # Example
/// ```ignore
/// assert_all_vertices_unique(&graph);
/// ```
pub fn assert_all_vertices_unique<G: GraphKind>(graph: &Hypergraph<G>)
where
    G::Atom: std::fmt::Display,
{
    let (is_unique, duplicates) = check_all_vertices_unique(graph);
    assert!(
        is_unique,
        "Found duplicate string representations in graph:\n{}",
        format_duplicates(&duplicates)
    );
}

/// Format duplicate string representations for error messages
fn format_duplicates(duplicates: &[(String, Vec<VertexIndex>)]) -> String {
    duplicates
        .iter()
        .map(|(string, indices)| {
            format!(
                "  '{}' appears at vertex indices: {:?}",
                string,
                indices.iter().map(|i| i.0).collect::<Vec<_>>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Check if a set of newly inserted tokens have unique string representations
///
/// # Arguments
/// * `graph` - The hypergraph to check
/// * `tokens` - The tokens to check for uniqueness
///
/// # Returns
/// A tuple of:
/// * `bool` - `true` if all tokens have unique string representations, `false` otherwise
/// * `Vec<(String, Vec<Token>)>` - List of duplicate strings with their tokens
///
/// # Example
/// ```ignore
/// let token1 = graph.insert_pattern(vec![a, b]);
/// let token2 = graph.insert_pattern(vec![c, d]);
/// let (is_unique, duplicates) = check_tokens_unique(&graph, &[token1, token2]);
/// assert!(is_unique);
/// ```
pub fn check_tokens_unique<G: GraphKind>(
    graph: &Hypergraph<G>,
    tokens: &[Token],
) -> (bool, Vec<(String, Vec<Token>)>)
where
    G::Atom: std::fmt::Display,
{
    let mut string_to_tokens: HashMap<String, Vec<Token>> = HashMap::new();
    
    for &token in tokens {
        let string_repr = Hypergraph::<G>::index_string(graph, token.index);
        string_to_tokens
            .entry(string_repr)
            .or_insert_with(Vec::new)
            .push(token);
    }
    
    let duplicates: Vec<(String, Vec<Token>)> = string_to_tokens
        .into_iter()
        .filter(|(_, tokens)| tokens.len() > 1)
        .collect();
    
    let is_unique = duplicates.is_empty();
    (is_unique, duplicates)
}

/// Assert that a set of newly inserted tokens have unique string representations
///
/// # Panics
/// Panics if any duplicate string representations are found among the tokens
///
/// # Example
/// ```ignore
/// let token1 = graph.insert_pattern(vec![a, b]);
/// let token2 = graph.insert_pattern(vec![c, d]);
/// assert_tokens_unique(&graph, &[token1, token2]);
/// ```
pub fn assert_tokens_unique<G: GraphKind>(
    graph: &Hypergraph<G>,
    tokens: &[Token],
)
where
    G::Atom: std::fmt::Display,
{
    let (is_unique, duplicates) = check_tokens_unique(graph, tokens);
    assert!(
        is_unique,
        "Found duplicate string representations among tokens:\n{}",
        format_token_duplicates(&duplicates)
    );
}

/// Format duplicate token string representations for error messages
fn format_token_duplicates(duplicates: &[(String, Vec<Token>)]) -> String {
    duplicates
        .iter()
        .map(|(string, tokens)| {
            format!(
                "  '{}' appears in tokens: {:?}",
                string,
                tokens
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Macro to assert that vertices have expected string representations
///
/// This replaces the need for exact vertex index assertions, checking only
/// that the string representation matches the expected value.
///
/// # Example
/// ```ignore
/// assert_string_repr!(graph,
///     token1 => "abc",
///     token2 => "xyz"
/// );
/// ```
#[macro_export]
macro_rules! assert_string_repr {
    ($graph:ident, $($token:ident => $expected:expr),* $(,)?) => {
        {
            use $crate::trace::has_graph::HasGraph;
            let g = $graph.graph();
            $(
                $crate::tests::macros::string_repr::assert_token_string_repr(&*g, $token, $expected);
            )*
            drop(g);
        }
    };
}

/// Macro to assert that all specified tokens have unique string representations
///
/// # Example
/// ```ignore
/// assert_unique_tokens!(graph, token1, token2, token3);
/// ```
#[macro_export]
macro_rules! assert_unique_tokens {
    ($graph:ident, $($token:ident),* $(,)?) => {
        {
            use $crate::trace::has_graph::HasGraph;
            let g = $graph.graph();
            let tokens = vec![$($token),*];
            $crate::tests::macros::string_repr::assert_tokens_unique(&*g, &tokens);
            drop(g);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        insert_atoms,
        insert_patterns,
        trace::has_graph::HasGraph,
    };

    #[test]
    fn test_check_token_string_repr() {
        let mut graph = HypergraphRef::default();
        insert_atoms!(graph, {a, b, c});
        
        let g = graph.graph();
        assert!(check_token_string_repr(&*g, a, "a"));
        assert!(check_token_string_repr(&*g, b, "b"));
        assert!(!check_token_string_repr(&*g, a, "b"));
        drop(g);
        
        insert_patterns!(graph, abc => [a, b, c]);
        let g = graph.graph();
        assert!(check_token_string_repr(&*g, abc, "abc"));
    }

    #[test]
    fn test_assert_token_string_repr() {
        let mut graph = HypergraphRef::default();
        insert_atoms!(graph, {a, b, c});
        insert_patterns!(graph, abc => [a, b, c]);
        
        let g = graph.graph();
        assert_token_string_repr(&*g, a, "a");
        assert_token_string_repr(&*g, abc, "abc");
    }

    #[test]
    #[should_panic(expected = "Token string representation mismatch")]
    fn test_assert_token_string_repr_fails() {
        let mut graph = HypergraphRef::default();
        insert_atoms!(graph, {a, b});
        
        let g = graph.graph();
        assert_token_string_repr(&*g, a, "b");
    }

    #[test]
    fn test_check_all_vertices_unique() {
        let mut graph = HypergraphRef::default();
        insert_atoms!(graph, {a, b, c, d});
        insert_patterns!(graph,
            ab => [a, b],
            cd => [c, d]
        );
        
        let g = graph.graph();
        let (is_unique, duplicates) = check_all_vertices_unique(&*g);
        assert!(is_unique, "Expected unique string reprs, got duplicates: {:?}", duplicates);
    }

    #[test]
    fn test_assert_all_vertices_unique() {
        let mut graph = HypergraphRef::default();
        insert_atoms!(graph, {a, b, c});
        insert_patterns!(graph, abc => [a, b, c]);
        
        let g = graph.graph();
        assert_all_vertices_unique(&*g);
    }

    #[test]
    fn test_check_tokens_unique() {
        let mut graph = HypergraphRef::default();
        insert_atoms!(graph, {a, b, c, d});
        insert_patterns!(graph,
            ab => [a, b],
            cd => [c, d]
        );
        
        let g = graph.graph();
        let (is_unique, duplicates) = check_tokens_unique(&*g, &[ab, cd]);
        assert!(is_unique, "Expected unique tokens, got duplicates: {:?}", duplicates);
    }
}
