//! Plan 1.8 test 1: `Create` from every reader fixture corpus equals the
//! fixture graph, before and after reopening the store.

use context_api::{
    types::*,
    Command,
    CommandResult,
};
use context_trace::positioned::fixtures::fixture_snapshots;

use crate::common::*;

#[test]
fn every_fixture_round_trips_through_create_export_and_reopen() {
    let env = Env::new();
    let mut failures = Failures::default();
    for (name, fixture) in fixture_snapshots() {
        let corpus = &fixture.corpus;
        create(&env.session, &name, corpus);

        let exported = export(&env.session, &name);
        failures.check(exported.graph.vertices == fixture.graph.vertices, || {
            format!("{name}: exported graph differs from the fixture")
        });
        failures.check(&exported.corpus == corpus, || {
            format!("{name}: exported corpus {:?} != {corpus:?}", exported.corpus)
        });

        let log = inputs(&env.session, &name);
        failures.check(log.len() == corpus.len(), || {
            format!("{name}: {} inputs logged for {} texts", log.len(), corpus.len())
        });
        for (record, text) in log.iter().zip(corpus) {
            failures.check(record.width == text.chars().count(), || {
                format!("{name}: input {} width {} for {text:?}", record.id, record.width)
            });
            let token_text = label(&env.session, &name, record.token);
            failures.check(&token_text == text, || {
                format!("{name}: input {} token reads {token_text:?}, not {text:?}", record.id)
            });
        }

        let summary = info(&env.session, &name);
        let atoms = fixture.graph.vertices.keys().filter(|k| k.chars().count() == 1).count();
        failures.check(
            summary.vertex_count == fixture.graph.vertices.len()
                && summary.atom_count == atoms
                && summary.composite_count == summary.vertex_count - atoms
                && summary.input_count == corpus.len()
                && summary.total_input_chars
                    == corpus.iter().map(|t| t.chars().count()).sum::<usize>()
                && summary.name == name,
            || format!("{name}: inconsistent info {summary:?}"),
        );
        let rendered = render_json(&env.session, &name);

        // A new session has nothing in memory: everything comes from the file.
        let reopened = env.session(|_| {});
        failures.check(export(&reopened, &name) == exported, || {
            format!("{name}: export differs after reopening")
        });
        failures.check(inputs(&reopened, &name) == log, || {
            format!("{name}: input log differs after reopening")
        });
        let reinfo = info(&reopened, &name);
        failures.check(
            (reinfo.vertex_count, reinfo.input_count, reinfo.total_input_chars, reinfo.generation)
                == (
                    summary.vertex_count,
                    summary.input_count,
                    summary.total_input_chars,
                    summary.generation,
                ),
            || format!("{name}: info differs after reopening: {reinfo:?} vs {summary:?}"),
        );
        // Persistence preserves vertex indices (D2).
        failures.check(render_json(&reopened, &name) == rendered, || {
            format!("{name}: rendered graph (indices) differs after reopening")
        });
    }
    failures.assert_none("fixture round trip");
}

fn render_json(
    session: &context_api::Session,
    graph: &str,
) -> RenderGraph {
    match exec(
        session,
        Command::Render {
            graph: graph.into(),
            format: RenderFormat::Json,
            root: None,
            depth: None,
            gate: force(),
            limits: unlimited(),
        },
    ) {
        CommandResult::Render(output) => {
            // The order of vertices and edges is not part of the contract.
            let mut graph = output.graph.expect("json render has a graph");
            graph.vertices.sort_by_key(|v| v.index);
            graph.edges.sort_by_key(|e| (e.parent, e.offset, e.child));
            graph
        },
        other => panic!("render: unexpected result {other:?}"),
    }
}
