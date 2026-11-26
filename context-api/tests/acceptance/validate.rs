//! `validate` and `stats` on graphs built by `create`.

use std::collections::BTreeMap;

use context_api::{
    Command,
    CommandResult,
};
use context_trace::positioned::fixtures::fixture_snapshots;

use crate::{
    common::*,
    corpora::random_corpora,
};

fn corpora() -> Vec<Vec<String>> {
    let mut corpora: Vec<Vec<String>> = fixture_snapshots()
        .into_iter()
        .map(|(_, fixture)| fixture.corpus)
        .step_by(4)
        .collect();
    corpora.extend(random_corpora(0x5eed_0003, 20));
    corpora
}

#[test]
fn created_graphs_validate() {
    let env = Env::new();
    let mut failures = Failures::default();
    for (i, corpus) in corpora().iter().enumerate() {
        let graph = format!("c{i}");
        create(&env.session, &graph, corpus);
        match exec(&env.session, Command::Validate { graph: graph.clone() }) {
            CommandResult::Validation(report) => failures.check(
                report.valid && report.errors.is_empty() && report.retention_violations.is_empty(),
                || format!("{corpus:?}: {report:?}"),
            ),
            other => panic!("validate: {other:?}"),
        }
    }
    failures.assert_none("validate");
}

#[test]
fn stats_histograms_match_the_graph() {
    let env = Env::new();
    let mut failures = Failures::default();
    for (i, corpus) in corpora().iter().enumerate() {
        let graph = format!("c{i}");
        create(&env.session, &graph, corpus);
        let stats = match exec(&env.session, Command::Stats { graph: graph.clone() }) {
            CommandResult::Stats(stats) => stats,
            other => panic!("stats: {other:?}"),
        };
        let vertices = export(&env.session, &graph).graph.vertices;
        let mut widths = BTreeMap::new();
        for key in vertices.keys() {
            *widths.entry(key.chars().count()).or_insert(0) += 1;
        }
        let summary = &stats.summary;
        failures.check(
            stats.width_histogram.values().sum::<usize>() == summary.vertex_count
                && stats.parent_count_histogram.values().sum::<usize>() == summary.vertex_count,
            || format!("{corpus:?}: histograms do not sum to {}: {stats:?}", summary.vertex_count),
        );
        failures.check(stats.width_histogram == widths, || {
            format!("{corpus:?}: width histogram {:?}, expected {widths:?}", stats.width_histogram)
        });
        failures.check(
            stats.width_histogram.get(&1).copied().unwrap_or(0) == summary.atom_count,
            || format!("{corpus:?}: atoms {stats:?}"),
        );
        let longest = corpus.iter().map(|t| t.chars().count()).max().unwrap();
        failures.check(stats.max_width == longest, || {
            format!("{corpus:?}: max width {}", stats.max_width)
        });
    }
    failures.assert_none("stats");
}
