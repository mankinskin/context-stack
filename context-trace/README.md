# Context-Trace

## Legacy / positioned split

The row model (pattern-row `Hypergraph`, row paths, trace cache, row tests)
lives in the crate [`context-trace-legacy`](../context-trace-legacy/README.md)
(`context_trace_legacy::…`), which depends on this crate for the shared items.
This crate holds only the shared, model-agnostic items and the positioned model:

- **Shared:** `logging/`, `graph/kind.rs` (`GraphKind`), `graph/vertex/{atom,
  id, key, token, wide, has_vertex_index}`, `graph/vertex/name_resolver.rs`
  (thread-local `TokenNameResolver` hook used by `Token`/`VertexIndex`
  display), `key/` (trace-cache keys), `position.rs` (`AtomPosition`).
- **`positioned/`:** the positioned model (below).

**Rule:** the legacy crates are frozen (no new code). New (positioned-model)
code goes into this crate and must not depend on `context-trace-legacy`.
Plan: `context-read/agents/plans/20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`.

**Positioned model** (`src/positioned/`, root, no legacy dependency; layout:
`model/` data types, `store/` store traits, `committed/` the committed
store, `edit/` the `GraphEdit` mutation layer, `stage/` the overlay
stage, `interchange/` snapshots and test data, `path/`, `trace/`):
`interchange/` (v3 `PositionedGraphData`/`PositionedGraphSnapshot` and the
structural validator, re-exported by `ngrams::output`), `model/composition.rs`
(`Occurrence`, `Composition`), `model/location.rs` (`OccurrenceLocation`,
`IntervalView`), `committed/graph.rs` (`PositionedGraph`,
`PositionedGraphRef = Arc<RwLock<_>>`), `model/vertex.rs`
(`PositionedVertex`), `interchange/convert.rs`
(`test-api`: `from_interchange`/`to_interchange`, `store_interchange` for
any store; `from_interchange_named` also returns the fixture key → token
name map), `edit/dissolve.rs` (`GraphEdit::dissolve`/`discard`/`settle` of
staged tokens with ≤ 1 parent occurrence, `Candidates`, `Removal`,
`Settled`; dissolution plan D4), `interchange/diff.rs` (`test-api`:
`diff` → `GraphDiff`), content reading `store/navigate.rs` (debug and `test-api`
builds: `PositionedGraph::read_interval`, content of an `IntervalView`, each
position once), search navigation
`store/navigate.rs` (`start_at`, `prefix_child`, `parent_occurrences`,
`atom_token`, `advance_end_path`, `token`; `Composition::covering`/`prefix`),
search paths `path/` (role-generic `RolePath<R: PathRole, Root: PathRoot>`
mirroring legacy `RolePath`/`RootedRolePath`: roles `Start`/`End` carry
the trace direction, storage order (start bottom-up, end top-down), span
boundary, canonical border predicate, `locate` and trace key positions;
roots are a graph `Token` or a `QueryRoot` pattern; aliases `StartPath`,
`EndPath`, `QueryPath = RolePath<End, QueryRoot>`; `RangePath<Root>` with
absolute `span` and `HasRolePath<Start/End>`; `SpanCoverage`; position
arithmetic confined to `path/role.rs` for later multi-dimensional inputs),
trace states and cache `trace/` (mirrors the legacy `trace/`: `ParentState`,
`ChildState`/`RootChildState`, `TraceCache` with offset-labelled edges and
query-position keys, `Traceable` commands; search plan 2.2), `interchange/fixtures.rs` (`test-api`:
`fixture_snapshots` and `fixture_graphs` → `FixtureGraph { name, corpus, graph, names }`,
shared with context-search). Tests round-trip and validate every v3 fixture
of `context-read/tests/fixtures/v3` and check `read_interval` and the
search navigation primitives on every fixture vertex and position
(`tests/navigation.rs`), and paths, trace states and trace cache on every
fixture (`tests/trace/`). Trace-cache keys (`DirectedKey`, `UpKey`,
`DownKey`, `UpPosition`, `DownPosition`) live in the root module `key.rs`,
re-exported by `context_trace_legacy::trace::cache::key::directed`.

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
  on a staged composite other than the root with < 2 parent occurrences.
- **Expectations come from the oracle.** Tests address vertices only through
  the `test-api` name map and derive expectations from v3 fixture strings,
  never from graph content lookups or reader output.

Foundational component of the context framework providing core graph 
structures, path operations, and tracing capabilities. Implements 
hypergraph data structures with sophisticated path navigation and 
bidirectional tracing.

## Features
- Hypergraph structure with vertex and token management
- Thread-safe graph references (Arc/RwLock)
- Role-based path operations with accessors/mutators
- Bidirectional tracing (bottom-up and top-down)
- Comprehensive cache management
- State continuation support

## Structure
- **`graph/`**: shared vertex ids, `Token`, atoms, `GraphKind`
- **`key/`**: trace-cache keys (`DirectedKey`, `UpKey`, `DownKey`)
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

**Features**: `test-api` (deterministic testing), default (logging)

## Architecture
Layered design with graph data structures, path navigation system, 
and comprehensive tracing capabilities for state management and 
search operations.