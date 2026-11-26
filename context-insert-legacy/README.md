# Context-Insert-Legacy

Row-model insertion of context-insert: split-join insertion into the
pattern-row `Hypergraph` of
[`context-trace-legacy`](../context-trace-legacy/README.md), driven by
[`context-search-legacy`](../context-search-legacy/README.md) responses.
Extracted from the former `src/legacy` module of context-insert; import as
`context_insert_legacy::…`. The positioned insertion lives in
[`context-insert`](../context-insert/README.md); the detailed walkthrough of
this crate is [`context-insert/HIGH_LEVEL_GUIDE.md`](../context-insert/HIGH_LEVEL_GUIDE.md).

**Frozen:** no new code here; only fixes needed to keep the legacy tests
green. New (positioned-model) code goes into `context-insert` and must not
depend on this crate.

**Legacy tests:** every legacy test is
`#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]`, so a
plain `cargo test` skips them. Run the legacy suite with
`cargo test -p context-insert-legacy --features legacy-tests,test-api` (the
feature propagates to the lower legacy crates) whenever a change touches this
crate. Do not use `-- --ignored` for this: it also runs slow sweeps.

## Structure
- **`insert/`**: Main insertion interface, context, results
  - `context.rs`: InsertCtx with InsertTraversal kind
  - `direction.rs`: Directional insertion logic
  - `result.rs`: InsertResult trait and result extraction
  - `outcome.rs`, `publication.rs`: `InsertOutcome`, `Publication` of
    structural edits
  - `scoped.rs`: scope-restricted insertion of boundary complements
- **`interval/`**: Interval management, partitioning, initialization
  - `init.rs`: InitInterval for initialization operations
  - `partition/`: Complex partitioning logic with delta calculations
    - `info/`: Partition information (borders, ranges, roles, modes)
- **`split/`**: Graph splitting operations, caching, tracing
  - `context.rs`: Split context management
  - `pattern.rs`: Pattern-specific splitting operations
  - `run.rs`: Split execution logic
  - `cache/`: Split caching (leaves, position, vertex caches)
  - `trace/`: Split tracing with state management
  - `vertex/`: Vertex-specific split operations and output
- **`join/`**: Graph joining operations, context, partitions
  - `context/`: Join context with frontier and node handling
  - `joined/`: Post-join structures (partitions, patterns)
  - `partition/`: Join-specific partitioning (inner ranges, pattern info)
  - `plan.rs`, `select.rs`: operation-local `JoinPlan` and its selection
- **`overlap/`**: `Overlap` closure executor and `ClosurePolicy` (used by
  `context-read-legacy`)
- **`visualization.rs`**: split/join visualization events
- **`tests/`**: insertion test cases (`cases/`) and environments (`env/`)

## Usage
```rust
use context_insert_legacy::{ToInsertCtx, InitInterval};

// Insert foldable patterns
let result = graph.insert(foldable_pattern)?;

// Insert with initialization interval
let result = graph.insert_init(extract, init_interval);

// Insert or get if already complete
let result = graph.insert_or_get_complete(pattern)?;
```

## Key Concepts
- **Split-Join Architecture**: Safe graph modification through splitting and joining
- **Interval Management**: State tracking during complex insertions
- **Same-Child Intervals**: Bottom-up joins retain materialized intervals by
  child token and half-open atom offsets. When both boundaries lie inside one
  child, the parent reuses that interval instead of concatenating overlapping
  suffix and prefix fragments.
- **Planned Joins**: The join phase builds an operation-local `JoinPlan`
  (`join/plan.rs`) instead of mutating the graph: candidate vertices
  (`CandidateId`, never vertex indices) with origin-tagged decompositions,
  proposed edits to existing vertices, and split provenance
  (`split/provenance.rs`). Nodes are read through an overlay of proposed
  replacements recorded against original child ranges. `select`
  (`join/select.rs`) and `publish` form the single point where the plan is
  materialized.
- **Local Retention**: `select` keeps only locally justified structure.
  Evidence decompositions (joined, perfect replacement, root pattern) count as
  parents; binary sub-merges are proposals, accepted only when all children
  are retained independently and they add a new border. A candidate with one
  evidence parent and one decomposition is inlined into that parent (unless
  that would make decompositions share a border); one without parents is
  dropped. Overlap wrappers (two or more evidence decompositions) are kept.
- **Multi-Phase Processing**: Pre/in/post visit modes for insertion stages
- **Result Extraction**: Different modes for handling insertion results
