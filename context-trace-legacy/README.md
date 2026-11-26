# Context-Trace-Legacy

Row model of context-trace: the pattern-row `Hypergraph` (`ChildPatterns`),
row paths, the trace cache and their tests. Extracted from the former
`src/legacy` module of context-trace; import everything as
`context_trace_legacy::…`.
The crate re-exports the shared items of
[`context-trace`](../context-trace/README.md) (`pub use context_trace::*`:
vertex ids, `Token`, atoms, `GraphKind`, trace-cache keys, logging, macros),
so legacy code and its consumers (`context-search-legacy`, `ngrams`) import
from one namespace. Local items shadow the mirrored root items.

**Frozen:** no new code here; only fixes needed to keep the legacy tests
green. New (positioned-model) code goes into `context-trace` and must not
depend on this crate.

**Legacy tests:** every legacy test is
`#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]`, so a
plain `cargo test` skips them. Run the legacy suite with
`cargo test -p context-trace-legacy --features legacy-tests,test-api` whenever
a change touches this crate or promotes something out of it. Do not use
`-- --ignored` for this: it also runs slow sweeps. The two running legacy
doctests (`graph::test_graph`, `HasVertexStringRepr`) switch their fence to
`rust,ignore` via `cfg_attr(not(feature = "legacy-tests"), doc = …)`.

**Features:** `test-api` (forwards `context-trace/test-api`), `legacy-tests`
(un-ignores the legacy suite).

## Structure
- **`direction/`**: `PatternDirection`, direction merging
  - `base.rs`, `merge.rs`, `pattern.rs`
- **`graph/`**: `Hypergraph`, `HypergraphRef`, getters, insertion
  - `insert/`: graph insertion operations (atom, pattern, patterns, range,
    replace, vertex, parents)
  - `getters/`: data access (atom, child, parent, pattern, vertex)
  - `vertex/`: row vertex data (`data/`, `entry`, `has_vertex_data`,
    `location/`, `parent`, `pattern/`, `token`, `traversal`, `wide`)
  - `snapshot.rs`, `validation.rs`, `search_path.rs`, `visualization.rs`,
    `test_graph.rs`, `child_strings.rs`
- **`path/`**: path accessors, mutators, role-based paths
  - `accessors/`: path access (border, role, root, child navigation)
  - `mutators/`: path modification (append, lower, pop, raise, simplify, move)
  - `structs/`: path data structures (role, rooted, sub paths)
- **`trace/`**: trace context, caching, state management
  - `has_graph.rs`: graph access trait (`HasGraph`, `TravKind`)
  - `traceable/`: traceability operations
  - `cache/`: trace caching (position, vertex, key management; the
    directed keys are a frozen copy of `context_search::trace::key`)
  - `child/`: child tracing (iterators, states)
  - `state/`: tracing state management (base state, parent states)
- **`logging/`**: `path_format.rs` (path formatting for logs)
- **`tests/`**: test environments (`env/`), macros, `test_case/`
  (`TestEnv`), row tests

## Usage
```rust
use context_trace_legacy::{Hypergraph, HypergraphRef, RolePath};

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
