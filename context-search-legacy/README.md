# Context-Search-Legacy

Row-model search of context-search: pattern search over the pattern-row
`Hypergraph` of [`context-trace-legacy`](../context-trace-legacy/README.md).
Extracted from the former `src/legacy` module of context-search; import as
`context_search_legacy::…` (e.g. `Find`, `Searchable`, `Response`). The
positioned search lives in [`context-search`](../context-search/README.md).

**Frozen:** no new code here; only fixes needed to keep the legacy tests
green. New (positioned-model) code goes into `context-search` and must not
depend on this crate.

**Legacy tests:** every legacy test is
`#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]`, so a
plain `cargo test` skips them. Run the legacy suite with
`cargo test -p context-search-legacy --features legacy-tests,test-api` (the
feature propagates to `context-trace-legacy`) whenever a change touches this
crate. Do not use `-- --ignored` for this: it also runs slow sweeps.

## Structure
- **`search/`**: search entry points (`Find`, `Searchable`), context, BFT,
  events
- **`match/`**: match iteration and root cursor (`root_cursor/`)
- **`compare/`**: token comparison and `CompareState` (`state/`)
- **`state/`**: start, end (`end/` prefix/postfix/range), matched states,
  `Response`
- **`cursor/`**: row path cursors, checkpoints; `cursor/shared/` is a frozen
  copy of the row-independent typestates (formerly
  `context_search::cursor`)
- **`container/`**: BFT/DFT state containers and traversal order
- **`policy.rs`**, **`logging/`** (`cursor_format.rs`)
- **`tests/`**: test cases and environments, search and state-advance tests

## Usage Examples

### Basic Pattern Search
```rust
use context_search_legacy::*;
use context_trace_legacy::*;

// Create search context
let graph_ref = HypergraphRef::new(graph);
let traversal = BreadthFirst::new(graph_ref);

// Search for token sequence
let query = &[token_a, token_b, token_c];
let result = query.fold(traversal)?;

match result.kind {
    FinishedKind::Complete(end) => {
        println!("Found complete match at: {:?}", end.cursor);
    },
    FinishedKind::Incomplete(end) => {
        println!("Partial match, stopped at: {:?}", end.reason);
    }
}
```

### Advanced Search with Cursors
```rust
// Create cursor for complex navigation
let cursor = PatternCursor::new(
    RootedRolePath::new(pattern, role_path),
    atom_position
);

// Search from specific position
let result = cursor.fold::<DepthFirst>(traversal)?;
```
