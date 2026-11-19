use pretty_assertions::assert_eq;

use crate::positioned::{
    diff,
    interchange::PositionedGraphData,
};

fn data(vertices: &[(&str, &[(usize, &str)])]) -> PositionedGraphData {
    PositionedGraphData {
        vertices: vertices
            .iter()
            .map(|(owner, cover)| {
                let cover = cover.iter().map(|(o, c)| (*o, c.to_string())).collect();
                (owner.to_string(), cover)
            })
            .collect(),
    }
}

#[test]
fn diff_lists_vertices_and_occurrences_per_owner() {
    let expected = data(&[
        ("a", &[]),
        ("b", &[]),
        ("aba", &[(0, "a"), (1, "b"), (2, "a")]),
        ("ababa", &[(0, "aba"), (2, "aba")]),
    ]);
    let actual = data(&[
        ("a", &[]),
        ("b", &[]),
        ("ab", &[(0, "a"), (1, "b")]),
        ("ababa", &[(0, "ab"), (2, "ab"), (4, "a")]),
    ]);
    assert!(diff(&expected, &expected).is_empty());
    let result = diff(&expected, &actual);
    assert_eq!(
        result.to_string(),
        "graph differs from expected:\n  \
         missing vertices: \"aba\"\n  \
         extra vertices: \"ab\"\n  \
         \"ababa\": missing [\"aba\"@0, \"aba\"@2] extra [\"ab\"@0, \"ab\"@2, \"a\"@4]\n"
    );
}
