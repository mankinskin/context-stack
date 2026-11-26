# Context-Trace

## Legacy / positioned split

The row model (pattern-row `Hypergraph`, row paths, trace cache, row tests)
lives in the crate [`context-trace-legacy`](../context-trace-legacy/README.md)
(`context_trace_legacy::…`), which depends on this crate for the shared items.
This crate holds only the shared, model-agnostic items and the positioned model:

- **Shared:** `logging/` (tracing setup, test log capture),
  `graph/kind.rs` (`GraphKind`), `graph/vertex/{atom, id, key, token, wide,
  has_vertex_index}`, `graph/vertex/name_resolver.rs` (thread-local
  `TokenNameResolver` hook used by `Token`/`VertexIndex` display),
  `position.rs` (`AtomPosition`), deterministic `HashMap`/`HashSet`, the
  proc-macro re-exports.
- **`positioned/`:** the positioned model (below).

**Rule:** the legacy crates are frozen (no new code). New (positioned-model)
code goes into this crate and must not depend on `context-trace-legacy`.
Plan: `context-read/agents/plans/20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`.

**Layering:** trace < search < insert < read; a crate depends only on
lower layers (test `crates_depend_only_on_lower_layers` in
`src/positioned/tests/guard.rs`). Moved out of this crate:

| Item | Owner |
|---|---|
| search paths (`RolePath`, `StartPath`, `EndPath`, `QueryPath`, `RangePath`, `PathNavigate`) | [`context_search::path`](../context-search/README.md#paths-and-trace) |
| trace states, `TraceCache`, cache keys (`DirectedKey`, `UpKey`, `DownKey`) | [`context_search::trace`](../context-search/README.md#paths-and-trace) |
| mutation (`GraphEdit`, `EditExt`, dissolution, stabilization, `Stage`), insert fixture lane, `insert_manifest` example | [`context_insert::{edit, fixtures}`](../context-insert/README.md#mutation-edit) |
| overlay stage (`StageHandle`, `OverlayHandle`, `StagingGraph`), read fixture lane, `read_manifest` example | [`context_read::stage`](../context-read/README.md#overlay-stage-stage) |

**Positioned model** (`src/positioned/`, no legacy dependency):

- `model/`: `Occurrence`, `Composition` (`composition.rs`),
  `OccurrenceLocation`, `IntervalView` (`location.rs`), `PositionedVertex`
  (`vertex.rs`), `PositionedError`.
- `store/`: read traits `GraphRead`/`VertexRead` (`read.rs`),
  `CompositionRead` (`composition.rs`), `HasGraph`/`HasGraphMut`
  (`has_graph.rs`), `Navigate` (`navigate.rs`: `start_at`, `prefix_child`,
  `postfix_child`, `prefixes`, `postfixes`, `parent_occurrences`; debug and
  `test-api` builds: `read_interval`, content of an `IntervalView`, each
  position once), `GraphWrite` over the sealed raw write primitives
  (`write.rs`, `context_trace::positioned::raw`, exported only with the
  hidden feature `edit`, enabled only by context-insert and context-read).
- `committed/`: `PositionedGraph`, `PositionedGraphRef = Arc<RwLock<_>>`
  (`graph.rs`), store trait impls (`access.rs`), validated (de)serialization
  (`serde.rs`, feature `serde`), table construction (`table.rs`).
- `check.rs`: hidden store-generic structural checks shared with the higher
  crates; `validate_store` (`test-api`).
- `interchange/`: v3 `PositionedGraphData`/`PositionedGraphSnapshot` and
  the structural validator (`mod.rs`, re-exported by `ngrams::output`);
  `convert.rs` (feature `interchange`: `from_interchange`/`to_interchange`
  via table construction, `store_interchange` for any store;
  `from_interchange_named` also returns the fixture key → token name map);
  `diff.rs` (`test-api`: `diff` → `GraphDiff`); `fixtures.rs` (`test-api`:
  `Scenario`, `fixtures_dir`, `manifest`, `snapshot`, `fixture_snapshots`,
  `fixture_graphs` → `FixtureGraph { name, corpus, graph, names }`, shared
  with the higher crates).

Tests (`src/positioned/tests/`) round-trip and validate every v3 fixture of
`context-read/tests/fixtures/v3` and check `read_interval` and the
`Navigate` primitives on every fixture vertex and position
(`navigation.rs`).

**Binding rules** (`context-read/agents/plans/20261120_PLAN_POSITIONED_READER_OPEN_WORK.md`):

- **No content-keyed index.** The only value-keyed index is atom value →
  atom vertex (`PositionedGraph::atoms`); composites are reached
  structurally, by search through parent links and compositions. Enforced by
  `tests/guard.rs`: the fields of `PositionedGraph` and `PositionedVertex`
  (read from the `committed/graph.rs` and `model/vertex.rs` sources) must equal a structural allowlist, and only
  `#[cfg(debug_assertions)]` fields may hold content. A new field fails the
  test and must be reviewed against these rules.
- **Expanded content is debug-only.** `PositionedVertex::label` (built from
  the children's labels at insertion; `PositionedGraph::name_resolver` /
  `register_name_resolver` install it as `TokenNameResolver`) exists only
  under `debug_assertions`; release `Display` shows index and width only.
  `read_interval`, `interval` and `IntervalView` exist only under
  `debug_assertions` or `test-api` (export, tests, diagnostics and debug/test
  validation), never for algorithms; `to_interchange`, `diff`,
  `contains_path` and `register_name_resolver` are `test-api` only.
- **Validation.** `validate()` always checks the structural rules and
  reciprocal parent links. Under `debug_assertions` or `test-api` it also
  checks one vertex per content (`DuplicateContent`) and overlap agreement
  (`OverlapMismatch`) by traversal into a temporary set dropped afterwards.
  Retention (G6: every composite that is not an input has ≥ 2 parent
  occurrences) is checked separately: `validate_retention(inputs)`
  (`test-api`), and at runtime only by `OverlayHandle::commit`, which panics
  on a staged composite other than the root with < 2 parent occurrences
  (`context_read::stage::overlay::OverlayHandle`).
- **Expectations come from the oracle.** Tests address vertices only through
  the `test-api` name map and derive expectations from v3 fixture strings,
  never from graph content lookups or reader output.

Foundational component of the context framework: vertex ids, atoms,
widths, logging and the positioned graph store with its read traits,
navigation core and interchange. Paths and tracing are in
[`context-search`](../context-search/README.md), mutation in
[`context-insert`](../context-insert/README.md).

## Features
- Positioned graph store (`PositionedGraph`) and store traits
- Thread-safe graph references (Arc/RwLock)
- Navigation core (`Navigate`) for search
- v3 interchange, diff and shared fixtures (`test-api`)
- Test tracing and log capture

## Structure
- **`graph/`**: shared vertex ids, `Token`, atoms, widths, `GraphKind`
- **`logging/`**: tracing setup, formatters, test log capture
- **`positioned/`**: positioned graph model (see above)

The row-model structure (`direction/`, `graph/` `Hypergraph`, `path/`,
`trace/`) and its usage are described in
[`context-trace-legacy`](../context-trace-legacy/README.md).

## Dependencies
- **petgraph**: Graph algorithms and data structures
- **indexmap**: Ordered maps for consistent iteration
- **uuid**: Unique identifiers for vertices and patterns
- **tracing**: Logging and debugging support
- **serde**: Serialization and deserialization

## Development
```bash
cargo test          # Run tests
cargo doc --open    # Generate documentation
```

### Testing with Logging

By default, test logs are written to files in `target/test-logs/` and automatically
deleted when tests pass. To enable stdout logging for debugging:

```bash
# Enable stdout logging for all tests
RUST_TEST_LOG_STDOUT=1 cargo test

# Run specific test with stdout logging
RUST_TEST_LOG_STDOUT=1 cargo test my_test_name -- --nocapture

# Combine with RUST_LOG for custom log levels
RUST_TEST_LOG_STDOUT=1 RUST_LOG=debug cargo test
```

Failed tests always preserve their log files in `target/test-logs/` for inspection.

**Features**: `interchange` (conversion), `serde`, `test-api` (implies
`interchange`; diff, fixtures, name map), hidden `edit` (raw write
primitives for context-insert and context-read).

## Architecture
Positioned graph store with read traits and a navigation core; higher
layers (search, insert, read) build on it and never on each other's
internals in reverse.
