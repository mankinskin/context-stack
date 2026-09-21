# [profiling] Native Criterion benchmark matrix for context-* + ticket/spec APIs

Child of tracker `ef3f4a91`. Add native Criterion benchmarks covering the
hot read/insert/search paths in the context-* crates and the ticket/spec
graph/index APIs, so server-side regressions are caught off-browser (the
WASM benches in `tests/graph3d_bench.rs` only cover the render math).

## Scope

- `context-stack/context-read`, `context-search`, `context-insert`,
  `context-trace` — index build, query, and traversal hot paths.
- `memory-api/crates/ticket-api` and `crates/spec-api` —
  graph subgraph/topgraph/health and Tantivy search.

## Existing infrastructure to reuse / extend

- `context-stack/context-read/benches/grammar.rs` (Criterion already wired).
- `memory-api/crates/ticket-api/benches/graph_ops.rs`
  (Criterion already wired) — extend rather than duplicate.

## Acceptance Criteria

- [ ] Each targeted crate has at least one Criterion bench group covering its
      dominant hot path, registered under `[[bench]]` in its `Cargo.toml`.
- [ ] `cargo bench -p <crate>` runs green and emits Criterion estimates.
- [ ] Bench output (Criterion `target/criterion/**` summary or saved txt) is
      linked as validation evidence before moving to `in-review`.
- [ ] No new bench reuses a fixture that pulls a full real store at runtime;
      use in-memory or temp-dir fixtures.

## Notes

- Run benches sequentially on Windows — do not run multiple `cargo bench`
  against the same target dir in parallel (build-lock hangs).
- ticket-api / spec-api benches live in the `memory-api` submodule; commit
  there first, then bump the submodule pointer.