# Context-Trace

## Legacy / positioned split (2026-11-19)

The row model (pattern-row `Hypergraph`) lives in `src/legacy/`, gated by the
default-on `legacy` feature, and is imported as `context_trace::legacy::…`.
There is no root `pub use legacy::*`; `legacy` mirrors the root items
(`pub use crate::*`), so legacy code imports everything from one namespace.

- **Root (model-agnostic):** `direction/` (`Direction`, `Left`, `Right`),
  `logging/`, `graph/kind.rs` (`GraphKind`), `graph/vertex/{atom, key, token,
  vertex_index, wide, has_vertex_index, has_vertex_key}`,
  `graph/vertex/name_resolver.rs` (thread-local `TokenNameResolver` hook used by
  `Token`/`VertexIndex` display), `position.rs` (`AtomPosition`),
  `tests/test_case/`. Impls of root items for row types carry
  `#[cfg(feature = "legacy")]`.
- **`legacy/`:** `direction/pattern.rs` (`PatternDirection`), `graph/`
  (`Hypergraph`, `HypergraphRef`, getters, insert, snapshot, validation,
  search_path, visualization, test_graph, child_strings), `graph/vertex/{data,
  entry, has_vertex_data, location, parent, pattern, traversal}`, `path/`,
  `trace/`, `logging/path_format.rs`, `tests/` (env, macros, row tests).
  The paths in the tables below refer to `src/legacy/…` for these modules.

**Rule:** no new code in `legacy`; only fixes needed to keep the legacy tests green. New (positioned-model) code goes into the root modules and must not depend on `legacy` (checked by `cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api`). Plan: `context-read/agents/plans/20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`.

**Legacy tests:** since the Step 2 test-suite cutover every legacy test is
`#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]`, so a
plain `cargo test` runs only the positioned and model-agnostic tests. Run the
legacy suite with `cargo test -p context-trace --features legacy-tests` (the feature
propagates to the lower crates) whenever a change touches `legacy` code or
promotes something out of it. Do not use `-- --ignored` for this: it also runs
the pending positioned reader lane and slow sweeps. The two running legacy
doctests (`legacy::graph::test_graph`, `HasVertexStringRepr`) switch their
fence to `rust,ignore` via `cfg_attr(not(feature = "legacy-tests"), doc = …)`.

**Positioned model** (`src/positioned/`, root, no legacy dependency):
`interchange.rs` (v3 `PositionedGraphData`/`PositionedGraphSnapshot` and the
structural validator, re-exported by `ngrams::output`), `composition.rs`
(`Occurrence`, `Composition`), `location.rs` (`OccurrenceLocation`,
`OccurrencePath`, `IntervalView`), `graph.rs` (`PositionedGraph`,
`PositionedVertex`, `PositionedGraphRef = Arc<RwLock<_>>`), `convert.rs`
(`from_interchange`/`to_interchange`; `test-api` `from_interchange_named`
also returns the fixture key → token name map), `diff.rs` (`diff` → `GraphDiff`),
read-only navigation `read.rs` (`PositionedGraph::read_interval`: content of an
`IntervalView`, each position once) and `enumerate.rs`
(`PositionedGraph::occurrences_in`: every retained occurrence inside an
`IntervalView`, deduplicated by `(token, offset)`), search navigation
`navigate.rs` (`start_at`, `prefix_child`, `parent_occurrences`,
`atom_token`, `advance_end_path`, `token`; `Composition::covering`/`prefix`),
search paths `path.rs` (`StartPath` bottom-up, `EndPath`, `RangePath` with
absolute `span`, `QueryPath` into composite query tokens, `SpanCoverage`),
trace states and cache `trace/` (mirrors `legacy/trace`: `ParentState`,
`ChildState`/`RootChildState`, `TraceCache` with offset-labelled edges and
query-position keys, `Traceable` commands; search plan 2.2), `fixtures.rs` (`test-api`:
`fixture_snapshots` and `fixture_graphs` → `FixtureGraph { name, corpus, graph, names }`,
shared with context-search). Tests round-trip and validate every v3 fixture
of `context-read/tests/fixtures/v3` and check `read_interval`,
`occurrences_in` and the search navigation primitives on every fixture vertex
and position (`tests/navigation.rs`), and paths, trace states and trace
cache on every fixture (`tests/trace.rs`). Boundary search lives in
`context_search::boundary`. Trace-cache keys (`DirectedKey`, `UpKey`,
`DownKey`, `UpPosition`, `DownPosition`) live in the root module `key.rs`,
re-exported from `legacy::trace::cache::key::directed`.

**Binding rules** (`context-read/agents/plans/20261120_PLAN_POSITIONED_READER_OPEN_WORK.md`):

- **No content-keyed index.** The only value-keyed index is atom value →
  atom vertex (`PositionedGraph::atoms`); composites are reached
  structurally, by search through parent links and compositions. Enforced by
  `tests/guard.rs`: the fields of `PositionedGraph` and `PositionedVertex`
  (read from `graph.rs` source) must equal a structural allowlist, and only
  `#[cfg(debug_assertions)]` fields may hold content. A new field fails the
  test and must be reviewed against these rules.
- **Expanded content is debug-only.** `PositionedVertex::label` (built from
  the children's labels at insertion; `PositionedGraph::name_resolver` /
  `register_name_resolver` install it as `TokenNameResolver`) exists only
  under `debug_assertions`; release `Display` shows index and width only.
  `read_interval` is for export, tests, diagnostics and debug/test validation,
  never for algorithms.
- **Validation.** `validate()` always checks the structural rules and
  reciprocal parent links. Under `debug_assertions` or `test-api` it also
  checks one vertex per content (`DuplicateContent`) and overlap agreement
  (`OverlapMismatch`) by traversal into a temporary set dropped afterwards.
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
- **`direction/`**: Left/Right directions, pattern matching
  - `match.rs`: Direction matching logic
  - `merge.rs`: Direction merging operations
  - `pattern.rs`: Directional pattern operations
- **`graph/`**: Hypergraph, vertices, getters, insertion
  - `kind.rs`: Graph type system (BaseGraphKind, GraphKind)
  - `insert.rs`: Graph insertion operations
  - `validation.rs`: Graph validation logic
  - `getters/`: Data access (advanced, child, parent, pattern, token, vertex)
  - `vertex/`: Vertex system (child, parent, token, pattern, location, wide)
- **`path/`**: Path accessors, mutators, role-based paths
  - `accessors/`: Path access (border, complete, role, root, child navigation)
  - `mutators/`: Path modification (append, lower, pop, raise, simplify, move)
  - `structs/`: Path data structures (query, role, rooted, sub paths)
- **`trace/`**: TraceCtx, caching, commands, state management
  - `has_graph.rs`: Graph access trait (HasGraph, TravKind)
  - `traceable.rs`: Traceability operations
  - `command.rs`: Trace commands (prefix, postfix, range)
  - `cache/`: Trace caching (position, vertex, key management)
  - `child/`: Child tracing (iterators, states, bands, positions)
  - `state/`: Tracing state management (base state, parent states)

## Usage
```rust
use context_trace::legacy::{Hypergraph, HypergraphRef, RolePath};

// Create hypergraph
let graph = Hypergraph::new();

// Thread-safe wrapper
let graph_ref = graph.to_ref();

// Access vertices and create paths
let vertex = graph.expect_vertex(index);
let path = RolePath::new(child_locations);
```

## Key Concepts
- **Hypergraph**: Indexed graph with vertex and token management
- **HypergraphRef**: Thread-safe Arc/RwLock wrapper
- **Role-based Paths**: Different path types for traversal roles
- **Bidirectional Tracing**: State tracking in both directions

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