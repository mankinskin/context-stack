# context-read File Index

## Legacy / positioned split

The row reader (`pipeline/`, `expansion/`, `request.rs`, `segment.rs`,
`input.rs`, its unit tests, `tests/legacy_reader_behavior.rs` and the frozen v2
fixtures) lives in the crate
[`context-read-legacy`](../context-read-legacy/README.md)
(`context_read_legacy::{read, ReadCtx, ReadSequenceIter, SegmentResult,
IntoReadInput}`); its files are not indexed here. This crate is the positioned
reader `context_read::read`.

**Rule:** the legacy crates are frozen (no new code). New (positioned-model) code goes into this crate and must not depend on the legacy crates. Plan: `context-read/agents/plans/20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`.

**Updated:** 2026-11-26 (legacy extraction)

## Module Organization

```
src/
├── lib.rs (41) - exports `read`, `read_observed`, `read_traced`, `Step`, `Commit`
├── read.rs (177) - reader entry points and step trace
├── segment.rs (149) - characters → atoms, known/unknown runs
├── root.rs (412) - the input's own vertex and its commits
├── forward.rs (125) - forward match
├── expand.rs (150) - root expansion
└── tests/ (steps.rs 436, observed.rs 96)
tests/
├── reader_behavior.rs (427) - v3 scenarios, `read_gate`
├── fixture_validation.rs (229) - v3, insert and read lanes
├── support/ (mod.rs 40, positioned.rs 27)
└── fixtures/v3/ - authoritative v3 fixtures (`insert/`, `read/` lanes)
benches/ - oracle_speedup.rs, staging.rs, staging_commit.rs, staging_memory.rs
```
