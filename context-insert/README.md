# Context-Insert

## Legacy / positioned split

The row-model insertion (`insert/`, `interval/`, `join/`, `overlap/`,
`split/`, `visualization.rs`, `tests/` over the row `Hypergraph`) lives in the
crate [`context-insert-legacy`](../context-insert-legacy/README.md)
(`context_insert_legacy::…`). This crate holds the positioned insertion only.

**Rule:** the legacy crates are frozen (no new code). New (positioned-model) code goes into this crate and must not depend on the legacy crates. Plan: `context-read/agents/plans/20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`.

## Positioned insertion

Plan: `context-read/agents/plans/20261122_PLAN_POSITIONED_INSERT.md`
(revision 3). Modules (no legacy dependency):

- **`insert/`**: `insert_match(&PositionedGraphRef, Response) ->
  Result<InsertOutcome, InsertError>` makes the span `[o, e)` of one search
  response a token, so that the graph equals the oracle graph of the corpus
  plus the span. Under one write guard: generation check
  (`InsertError::StaleResponse` if the graph changed after the search),
  `EntireRoot` → `Complete` (query exhausted) or `NoExpansion` (graph
  unchanged), otherwise split, join and `GraphEdit::finish` →
  `Created { token, response, publication }`. It never searches. Wrappers
  `insert_next_match(graph, query)` and `insert(graph, query) -> Token`
  search first (`InsertError::Search`) and insert only the matched part.
- **`split/`**: `SplitGraph::from_response(&PositionedGraph, &Response)`
  = `SplitGraph::from_trace(graph, root, span, &response.cache)`
  (read-only, legacy `InitInterval`). The root gets the span borders as
  split positions with a role (`Roles { start, end }`); the positions of
  the traced edges (the search's start and end paths in its `TraceCache`)
  are read from the cache, and top-down in decreasing `(width, index)` every
  position is pushed into every child occurrence strictly containing it
  (`Composition::crossing`). `SplitVertex::intervals(is_root)` derives the
  required intervals: the target at the root; postfixes, prefixes and their
  overlaps elsewhere.
- **`join/`**: `join(&mut GraphEdit, &SplitGraph) -> Token`. Split nodes in
  increasing `(width, index)`, intervals in increasing width: an interval
  inside a child resolves to that child or the child's resolved interval;
  otherwise `maximal(pieces)` is created with `GraphEdit::create`. Each node
  then gets its new tokens through one `GraphEdit::add_occurrences`. The
  target `N` is created last.

```rust
use context_insert::{insert_match, InsertOutcome};
use context_search::Find;

let response = graph.find_ancestor(query)?;
match insert_match(&graph, response)? {
    InsertOutcome::Created { token, publication, .. } => { /* publication.created ends with token */ },
    InsertOutcome::Complete { token, .. } | InsertOutcome::NoExpansion { token, .. } => { /* existing root */ },
}
```

No content- or composition-keyed lookups and no `read_interval` in the
algorithm; vertices are addressed by `VertexIndex`. Tests (`src/tests/`)
run every case of the insert lane (`context-read/tests/fixtures/v3/insert`)
against its oracle graph.

Graph insertion operations for the context framework. Handles the 
complex process of inserting new patterns into existing graph structures 
through sophisticated splitting, joining, and interval management operations.

## Features
- Pattern insertion into existing graph structures
- Split-join architecture for safe graph modifications
- Interval management for insertion state tracking
- Multi-phase processing (pre-visit, in-visit, post-visit)
- Comprehensive caching for split and join operations
- Result extraction with different insertion modes

The row-model structure, usage and concepts (split-join with
`InitInterval`, join plans, local retention) are described in
[`context-insert-legacy`](../context-insert-legacy/README.md).

## Dependencies
- **context-trace**: Core graph structures and tracing
- **context-search**: Search operations and foldable patterns
- **itertools**: Iterator utilities for complex operations
- **derive-new**: Simplified struct construction
- **tracing**: Logging and debugging support

## Development
```bash
cargo test          # Run tests
cargo doc --open    # Generate documentation
```

**Features**: `test-api` (testing utilities), default (standard configuration)

## Architecture
Multi-phase insertion pipeline with split-join architecture, 
comprehensive state management, and sophisticated caching for 
safe and efficient graph modifications.