//! Demonstration of compact logging format

use crate::{
    logging::Compact,
    *,
};

#[test]
fn test_compact_formatting() {
    let _tracing = init_test_tracing!();

    // Create a simple graph
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c],
    );
    insert_patterns!(graph,
        cd => [c, d],
        _abcd => [abc, cd]
    );

    // Create a path
    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let path = IndexRangePath::new_empty(root.clone());

    // Demonstrate different formatting levels
    tracing::info!("=== Compact Formatting Demo ===");

    // Compact Display format - single line, easy to scan
    tracing::info!(
        path = %Compact(&path),
        "Using Compact Display (single-line)"
    );

    // Indented Debug format - multi-line, shows structure
    tracing::debug!(
        path = %Compact(&path),
        "Using Compact Debug (multi-line)"
    );

    // Standard Display (uses our custom Display trait)
    tracing::info!(
        path = %path,
        "Using standard Display trait"
    );

    // Compare formats side by side
    tracing::info!(
        compact = %Compact(&path),
        display = %path,
        "Comparing Compact vs Display"
    );

    tracing::info!("Demo complete - check log formatting above");
}

#[test]
fn test_compact_with_token_display() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    insert_atoms!(graph, {x, y, z});
    insert_patterns!(graph,
        (_xyz, xyz_id) => [x, y, z]
    );

    let token = Token::new(VertexIndex::from(10), 3);
    let pattern_loc = PatternLocation::new(token, xyz_id);
    let root = IndexRoot::from(pattern_loc);

    tracing::info!(
        token = %token,
        pattern = %pattern_loc,
        root = %root,
        "Demonstrating token/pattern/root formatting"
    );

    // Show how these appear in paths
    let path = IndexRangePath::new_empty(root) as IndexRangePath;
    tracing::info!(
        path = %Compact(&path),
        "Path containing formatted root"
    );
}
