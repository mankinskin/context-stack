//! Plan 1.8 test 2: the graph built by `Create` equals the `ngrams` oracle
//! (`parse_corpus`) for small generated corpora.

use ngrams::{
    graph::{
        parse_corpus,
        Corpus,
        Status,
        StatusHandle,
    },
    Cancellation,
};

use crate::{
    common::*,
    corpora::oracle_corpora,
};

#[test]
fn create_equals_the_ngrams_oracle_on_small_corpora() {
    // No skip list: the positioned reader has no known oracle deviations
    // (context-read `tests/reader_behavior.rs` and the exhaustive lane in
    // `src/tests/steps.rs` ignore none), and every corpus here was checked
    // against the reader directly (see `corpora::oracle_corpora`).
    let env = Env::new();
    let mut failures = Failures::default();
    for (i, corpus) in oracle_corpora().iter().enumerate() {
        let oracle = parse_corpus(
            Corpus::new(format!("oracle{i}"), corpus),
            StatusHandle::from(Status::new(corpus)),
            Cancellation::None,
        )
        .unwrap_or_else(|error| panic!("{corpus:?}: oracle failed: {error:?}"))
        .graph;
        let graph = format!("c{i}");
        create(&env.session, &graph, corpus);
        let exported = export(&env.session, &graph);
        failures.check(exported.graph.vertices == oracle.vertices, || {
            format!(
                "{corpus:?}:\n  api:    {:?}\n  oracle: {:?}",
                exported.graph.vertices, oracle.vertices
            )
        });
        failures.check(&exported.corpus == corpus, || {
            format!("{corpus:?}: exported corpus {:?}", exported.corpus)
        });
    }
    failures.assert_none("graph != oracle");
}
