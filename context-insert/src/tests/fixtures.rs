//! The insert lane: every case extends its base corpus by a query that is
//! not a vertex of the base and is one of the expected graph.

use crate::fixtures::insert_cases;

#[test]
fn insert_cases_extend_their_base_by_a_non_vertex_query() {
    let cases = insert_cases();
    assert!(!cases.is_empty());
    for case in cases {
        let name = &case.name;
        assert!(!case.base.names.contains_key(&case.query), "{name}");
        assert!(case.expected.names.contains_key(&case.query), "{name}");
        let mut corpus = case.base.corpus.clone();
        corpus.push(case.query.clone());
        assert_eq!(case.expected.corpus, corpus, "{name}");
        case.expected
            .graph
            .validate()
            .unwrap_or_else(|error| panic!("{name}: {error}"));
    }
}
