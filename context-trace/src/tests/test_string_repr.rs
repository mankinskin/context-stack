#[cfg(test)]
mod tests {
    use crate::{
        graph::{
            kind::BaseGraphKind,
            test_graph::*,
            vertex::atom::Atom,
        },
        *,
    };

    #[test]
    fn test_token_string_representation() {
        let graph: Hypergraph<BaseGraphKind> = Hypergraph::default();

        // Insert some atoms
        let a = graph.insert_atom(Atom::Element('a'));
        let b = graph.insert_atom(Atom::Element('b'));
        let c = graph.insert_atom(Atom::Element('c'));

        // Without registering the graph, tokens show basic format
        let a_str_before = format!("{}", a);
        assert!(a_str_before.contains("w1"));
        assert!(!a_str_before.contains("\"a\""));

        // Register the graph
        register_test_graph(&graph);

        // Now tokens should show their string representation
        let a_str_after = format!("{}", a);
        assert!(
            a_str_after.contains("\"a\""),
            "Token should show string representation: {}",
            a_str_after
        );

        let b_str = format!("{}", b);
        assert!(
            b_str.contains("\"b\""),
            "Token should show string representation: {}",
            b_str
        );

        // Insert a pattern and test it
        let abc = graph.insert_pattern(vec![a, b, c]);

        // Re-register to pick up the new pattern
        register_test_graph(&graph);

        let abc_str = format!("{}", abc);
        assert!(
            abc_str.contains("abc"),
            "Pattern token should show string representation: {}",
            abc_str
        );

        // Clean up
        clear_test_graph();

        // After clearing, tokens go back to basic format
        let a_str_cleared = format!("{}", a);
        assert!(!a_str_cleared.contains("\"a\""));
    }

    #[test]
    fn test_get_string_repr_method() {
        // Clear any previously registered graph
        clear_test_graph();

        let graph: Hypergraph<BaseGraphKind> = Hypergraph::default();
        let x = graph.insert_atom(Atom::Element('x'));
        let y = graph.insert_atom(Atom::Element('y'));

        // Before registration
        assert_eq!(x.get_string_repr(), None);

        // After registration
        register_test_graph(&graph);
        assert_eq!(x.get_string_repr(), Some("x".to_string()));
        assert_eq!(y.get_string_repr(), Some("y".to_string()));

        // After clearing
        clear_test_graph();
        assert_eq!(x.get_string_repr(), None);
    }

    #[test]
    fn test_pattern_formatting_with_string_repr() {
        // Clear any previously registered graph
        clear_test_graph();

        let graph: Hypergraph<BaseGraphKind> = Hypergraph::default();
        let h = graph.insert_atom(Atom::Element('h'));
        let e = graph.insert_atom(Atom::Element('e'));
        let l = graph.insert_atom(Atom::Element('l'));
        let o = graph.insert_atom(Atom::Element('o'));

        let pattern = Pattern::from(vec![h, e, l, l, o]);

        // Without registered graph, pattern shows basic token format
        let pattern_str_before = format!("{}", pattern);
        println!("Pattern without graph: {}", pattern_str_before);
        assert!(pattern_str_before.contains("T"));
        assert!(pattern_str_before.contains("w"));

        // Register the graph
        register_test_graph(&graph);

        // Now pattern should show string representations
        let pattern_str_after = format!("{}", pattern);
        println!("Pattern with graph: {}", pattern_str_after);
        assert!(
            pattern_str_after.contains("\"h\""),
            "Pattern should show 'h': {}",
            pattern_str_after
        );
        assert!(
            pattern_str_after.contains("\"e\""),
            "Pattern should show 'e': {}",
            pattern_str_after
        );
        assert!(
            pattern_str_after.contains("\"l\""),
            "Pattern should show 'l': {}",
            pattern_str_after
        );
        assert!(
            pattern_str_after.contains("\"o\""),
            "Pattern should show 'o': {}",
            pattern_str_after
        );

        clear_test_graph();
    }
}
