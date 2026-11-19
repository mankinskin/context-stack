# Context-Insert

## Legacy / positioned split (2026-11-19)

All modules (`insert/`, `interval/`, `join/`, `overlap/`, `split/`,
`visualization.rs`, `tests/`) live in `src/legacy/`, gated by the default-on
`legacy` feature, and are imported as `context_insert::legacy::…`. The module
paths below refer to `src/legacy/…`.

**Rule:** no new code in `legacy`; only fixes needed to keep the legacy tests green. New (positioned-model) code goes into the root modules and must not depend on `legacy` (checked by `cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api`). Plan: `context-read/agents/plans/20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`.

**Legacy tests:** since the Step 2 test-suite cutover every legacy test is
`#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]`, so a
plain `cargo test` runs only the positioned and model-agnostic tests. Run the
legacy suite with `cargo test -p context-insert --features legacy-tests` (the feature
propagates to the lower crates) whenever a change touches `legacy` code or
promotes something out of it. Do not use `-- --ignored` for this: it also runs
the pending positioned reader lane and slow sweeps.

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

## Structure
- **`insert/`**: Main insertion interface, context, results
  - `context.rs`: InsertCtx with InsertTraversal kind
  - `direction.rs`: Directional insertion logic
  - `result.rs`: InsertResult trait and result extraction
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

## Usage
```rust
use context_insert::legacy::{ToInsertCtx, InitInterval};

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