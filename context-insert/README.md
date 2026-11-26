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

## Mutation (`edit/`)

`GraphEdit` is the only mutation layer of the positioned graph (moved here
from context-trace; insert plan rev 3, §8). It writes through the sealed raw
primitives `context_trace::positioned::raw` (hidden feature `edit` of
context-trace, enabled only by context-insert and context-read). Every
operation changes one vertex plus the parent links of the children it adds
or removes and checks its preconditions first; a rejected operation leaves
the graph unchanged.

- `edit/mod.rs`: `GraphEdit<S: GraphWrite>`; `EditExt` gives `.edit()` on
  any `GraphWrite` store (`use context_insert::edit::EditExt;`), or use
  `GraphEdit::new`; `finish` → `Publication`.
- `edit/operations.rs`: `atom`, `create`, `add_occurrences`, `extend`.
- `edit/publication.rs`: `Publication { created, … }`, `VertexEdit`.
- `edit/dissolve.rs`: `dissolve`/`discard`/`settle` of staged tokens with
  ≤ 1 parent occurrence (`Candidates`, `Removal`, `Settled`; dissolution
  plan D4).
- `edit/stable.rs`: early stabilization by fixed parent occurrences
  (`stabilize`, `stabilize_candidates`).
- `edit/stage.rs`: the `Stage` contract (a `GraphWrite<Index = StageIndex>`
  store with staged vertices) that dissolution and stabilization need;
  implemented by `context_read::stage`.

```rust
use context_insert::edit::EditExt;
use context_trace::positioned::{Occurrence, PositionedGraph};

let mut graph = PositionedGraph::default(); // or `graph_ref.write()`
let mut edit = graph.edit();
let (a, b) = (edit.atom('a'), edit.atom('b'));
let ab = edit.create(vec![Occurrence::new(0, a), Occurrence::new(1, b)])?;
let publication = edit.finish(); // `publication.created` ends with `ab`
```

## Insert fixture lane (`fixtures`, feature `test-api`)

`src/fixtures.rs` (moved from context-trace): `insert_dir`,
`insert_queries`, `insert_manifest`, `InsertCase`, `insert_cases` over
`context-read/tests/fixtures/v3/insert` (built on the shared
`context_trace::positioned::fixtures`). Rewrite the lane manifest with

```bash
cargo run -p context-insert --features test-api --example insert_manifest
```

then generate the cases with the ngrams CLI (see
[context-read](../context-read/README.md#insert-fixtures-v3insert)).
Tests: `src/tests/edit.rs` (`GraphEdit` operations and rejections),
`src/tests/fixtures.rs` (insert lane), `src/tests/interchange.rs` (table
import equals the import through one `GraphEdit`).

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
- **context-trace**: Positioned graph store, raw write primitives (feature `edit`)
- **context-search**: Search, `Response`, paths and `TraceCache`
- **itertools**: Iterator utilities for complex operations
- **derive-new**: Simplified struct construction
- **tracing**: Logging and debugging support

## Development
```bash
cargo test          # Run tests
cargo doc --open    # Generate documentation
```

**Features**: `test-api` (insert fixture lane, `insert_manifest` example), default (standard configuration)

## Architecture
Multi-phase insertion pipeline with split-join architecture, 
comprehensive state management, and sophisticated caching for 
safe and efficient graph modifications.