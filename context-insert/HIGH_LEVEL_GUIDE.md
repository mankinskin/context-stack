# Context-Insert: High-Level Overview

> **Graph modification engine for inserting new patterns into existing hypergraph structures.**

## What is Context-Insert?

Context-insert builds on context-trace and context-search to enable safe, efficient insertion of new patterns into hypergraphs. It provides:

1. **Pattern Insertion** - Add new patterns while maintaining graph invariants
2. **Split-Join Architecture** - Safe graph modification through staging
3. **Interval Management** - Track insertion state for incomplete searches
4. **Multi-Phase Processing** - Pre-visit, in-visit, post-visit modes
5. **Result Extraction** - Different modes for handling insertion outcomes

Think of it as the "write engine" - it knows how to safely modify the graph by inserting new patterns without breaking existing structures.

---

## Core Concepts

### 1. When Do You Need Insertion?

Insertion is needed when a search doesn't exhaust the query:

```rust
// Search for a sequence
let query = vec![a, b, c, d];
let result = graph.find_ancestor(query)?;

if !result.query_exhausted() {
    // Query not fully matched - need to insert remaining part
    let init = InitInterval::from(result);
    // Perform insertion...
}
```

**Use cases:**
- Building up graph structure incrementally
- Adding new patterns discovered in data
- Completing partial matches from searches
- Constructing complex hierarchies

### 2. The InitInterval Type

When a search doesn't exhaust the query, you can convert the `Response` to an `InitInterval`:

```rust
pub struct InitInterval {
    pub root: Token,            // Where to start insertion
    pub cache: TraceCache,      // What was learned during search
    pub end_bound: AtomPosition, // Where insertion should extend to
}

// Conversion from search result
let init = InitInterval::from(incomplete_response);
```

**What does it mean?**
- **root**: The pattern that partially matched
- **cache**: Trace information from the search
- **end_bound**: Position in the query where we need to extend

### 3. Split-Join Architecture

This is the core insight of safe graph modification:

**The Problem:**
- Can't directly modify patterns (other parts of graph might reference them)
- Need to maintain all existing references
- Must preserve graph invariants

**The Solution:**
```
Original Pattern → Split into pieces → Insert new content → Join pieces back
```

**Example:**
```
Have: abc = [a, b, c]
Want: abcd = [a, b, c, d]

Step 1 (Split): abc → [a, b, c] (individual pieces)
Step 2 (Insert): Add 'd' to the pieces
Step 3 (Join): [a, b, c, d] → abcd (new pattern)

Result: Both abc and abcd exist, no references broken
```

### 4. Insertion Modes

Different insertion scenarios require different handling:

**InsertResult trait** extracts results based on mode:
- Extract complete patterns that were found
- Extract intervals that need further processing
- Handle various completion states

### 5. IntervalGraph

During insertion, an `IntervalGraph` manages the intermediate state:

```rust
pub struct IntervalGraph {
    pub root: Token,              // Root of insertion
    pub states: SplitStates,      // Split state management
    pub cache: SplitCache,        // Cache for split operations
}
```

This tracks the insertion as it progresses through split-join phases.

---

## Key Types Reference

### Insertion Context Types

```rust
// Main insertion interface
pub trait ToInsertCtx {
    fn insert(&self, pattern) -> Result<...>;
    fn insert_init(extract, init) -> Result<...>;
    fn insert_or_get_complete(pattern) -> Result<...>;
}

// Insertion context
InsertCtx<G: HasGraph>           // Context for insertion operations

// Initialization from search
InitInterval {
    root: Token,
    cache: TraceCache,
    end_bound: AtomPosition,
}
```

### Split-Join Types

```rust
// Split caching
SplitCache                       // Cache for split operations
SplitVertexCache                 // Per-vertex split cache
SplitPositionCache              // Position-specific cache

// Split state
SplitStates {
    leaves: BTreeSet<PosKey>,    // Leaf positions
    queue: VecDeque<...>,        // Processing queue
}

// Position key
PosKey {
    token: Token,
    position: usize,
}

// Interval graph
IntervalGraph {
    root: Token,
    states: SplitStates,
    cache: SplitCache,
}
```

### Result Types

```rust
// Result extraction
pub trait InsertResult {
    // Extract complete patterns
    fn extract_complete(...) -> ...;
    // Extract intervals for further processing
    fn extract_interval(...) -> ...;
}
```

---

## Common Operations

### Basic Pattern Insertion

```rust
use context_insert::{ToInsertCtx, InitInterval};
use context_search::Searchable;

// First, search to see if pattern exists
let query = vec![a, b, c, d];
let search_result = graph.find_ancestor(query.clone())?;

// If incomplete, insert it
if !search_result.is_complete() {
    // Convert to initialization interval
    let init = InitInterval::from(search_result);
    
    // Perform insertion
    let insert_result = graph.insert_init(extract_mode, init)?;
    
    println!("Insertion completed");
}
```

### Insert or Get Existing Pattern

```rust
// Will insert if needed, or return existing
let result = graph.insert_or_get_complete(pattern)?;

match result {
    Some(token) => println!("Pattern exists or was inserted: {:?}", token),
    None => println!("Insertion incomplete"),
}
```

### Progressive Insertion

```rust
// Start with search
let result = graph.find_ancestor(query)?;

// Convert to interval
let init = InitInterval::from(result);

// Create interval graph
let mut interval = IntervalGraph::from((&mut graph, init));

// Process interval (internal operations)
// interval represents intermediate state during insertion
```

---

## Insertion Flow

### Step-by-Step Process

```
1. Search Phase
   ↓
   Query → Search → Response
                        ↓
                    is_complete()?
                    /            \
                  Yes             No
                   ↓               ↓
              Use existing    Need insertion
                                   ↓
2. Initialization Phase
   ↓
   Response → InitInterval
                 ↓
3. Split Phase
   ↓
   Identify split points
   Create split states
   Build split cache
                 ↓
4. Join Phase
   ↓
   Merge split components
   Create new patterns
   Update graph
                 ↓
5. Result Phase
   ↓
   Extract results
   Return to caller
```

### Internal Processing

**Pre-Visit Mode:**
- Preparation before processing nodes
- Setup state for traversal

**In-Visit Mode:**
- Active processing of nodes
- Main insertion logic

**Post-Visit Mode:**
- Cleanup after processing
- Finalize insertions

---

## Module Structure

### `insert/`
Main insertion interface and logic
- `mod.rs` - ToInsertCtx trait
- `context.rs` - InsertCtx implementation
- `direction.rs` - Directional insertion logic
- `result.rs` - InsertResult trait and extraction

### `interval/`
Interval management for insertion state
- `mod.rs` - IntervalGraph definition
- `init.rs` - InitInterval type and conversions
- **`partition/`** - Complex partitioning logic
  - `mod.rs` - Partition operations
  - `delta.rs` - Delta calculations (PatternSubDeltas)
  - **`info/`** - Partition metadata
    - `borders.rs` - Border information
    - `ranges.rs` - Range information
    - **`range/`** - Range-specific details
      - `mode.rs` - InVisitMode and processing modes
      - **`role/`** - Range roles
        - `mod.rs` - RangeRole trait
        - Types: `Pre`, `Post`, `In`, `BooleanPerfectOf`, `OffsetsOf`

### `split/`
Graph splitting operations
- `mod.rs` - Split traits and utilities
- `context.rs` - Split context management (SplitCacheCtx)
- `pattern.rs` - Pattern-specific splitting
- `run.rs` - Split execution logic
- **`cache/`** - Split caching system
  - `mod.rs` - SplitCache definition
  - `leaves.rs` - Leaf position tracking
  - `position.rs` - Position cache (PosKey, SplitPositionCache)
  - `vertex.rs` - Vertex cache (SplitVertexCache)
- **`trace/`** - Split tracing
  - `mod.rs` - Split trace context
  - **`states/`** - State management
    - `mod.rs` - Split states (SplitStates)
    - `context.rs` - SplitTraceStatesCtx
- **`vertex/`** - Vertex-specific splits
  - `mod.rs` - Vertex split operations
  - `output.rs` - Split output (CompleteLocations, InnerNode, RootMode)
  - `position.rs` - Position handling (SubSplitLocation, Offset, HasInnerOffset)

### `join/`
Graph joining operations
- `mod.rs` - Join operations
- **`context/`** - Join context
  - `mod.rs` - Join context types
  - `frontier.rs` - Frontier management
  - **`node/`** - Node handling
    - `kind.rs` - JoinKind trait
    - `mod.rs` - Node operations
- **`joined/`** - Post-join structures
  - `partitions.rs` - Joined partitions
  - `patterns.rs` - Joined patterns
- **`partition/`** - Join-specific partitioning
  - `inner.rs` - Inner range handling
  - `pattern.rs` - Pattern information

---

## Split-Join Architecture Deep Dive

### Why Split-Join?

**Problem:** Can't modify patterns in-place:
```
Pattern abc = [a, b, c]
Can't just add 'd' because:
- Other patterns might reference abc
- Graph invariants must be maintained
- Need to preserve existing structure
```

**Solution:** Create new patterns without modifying existing ones:
```
1. Split: Break pattern into constituent parts
2. Insert: Add new content alongside parts
3. Join: Create new pattern with new content
4. Result: Both old and new patterns coexist
```

### Split Phase Details

**What gets split:**
- The root pattern from InitInterval
- Constituent child patterns as needed
- Tracked via SplitStates

**Split cache tracks:**
- Which positions have been split
- Split boundaries and offsets
- Vertex-specific split information

**Example:**
```
Pattern: abc = [a, b, c]
Split result: 
- Position 0: a (leaf)
- Position 1: b (leaf)
- Position 2: c (leaf)
- Cache: Records split locations
```

### Join Phase Details

**What gets joined:**
- Split components
- New content to insert
- Creates new pattern instances

**Join handles:**
- Merging overlapping patterns
- Creating pattern references
- Updating parent-child relationships

**Example:**
```
Split components: [a, b, c]
New content: d
Join result: abcd = [a, b, c, d]
```

---

## Range Roles Explained

**What are roles?** Different phases of insertion need different boundary information.

### Pre Role
- Represents content **before** the insertion point
- Used for prefix handling

### Post Role
- Represents content **after** the insertion point
- Used for postfix handling

### In Role
- Represents content **at** the insertion point
- Active insertion region

### Combined Roles
- **BooleanPerfectOf<R>**: Role with boolean perfect boundary
- **OffsetsOf<R>**: Role with offset information

**Why multiple roles?** Different insertion scenarios need different boundary calculations and validation.

---

## Performance Characteristics

### Time Complexity
- **Pattern search**: O(d * p) - prerequisite
- **Split phase**: O(k) where k = split points
- **Join phase**: O(m) where m = join operations
- **Overall insertion**: O(d * p + k + m)

### Space Complexity
- **InitInterval**: O(1) + cache size
- **SplitCache**: O(s) where s = split points
- **IntervalGraph**: O(s + q) where q = queue size
- **Overall**: O(cache + splits)

### Optimization Strategies
- **Cache reuse**: Leverage search cache in split phase
- **Lazy splitting**: Only split what's necessary
- **Queue management**: Process splits efficiently

---

## Common Gotchas

### 1. Converting Response Before Checking

```rust
// ❌ Wrong - converting query-exhausted response
let init = InitInterval::from(response);  // Should check query_exhausted() first!

// ✅ Correct - only convert if query not exhausted
if !response.query_exhausted() {
    let init = InitInterval::from(response);
    // Now insert
}
```

### 2. Wrong end_bound Expectations

**Current issue:** `cursor_position()` might not match expected `end_bound`

```rust
// Implementation in init.rs
let end_bound = response.cursor_position();

// But tests might expect:
// end_bound = cursor_position() + 1?  // Or something else?
```

*See QUESTIONS_FOR_AUTHOR.md for clarification needed*

### 3. Forgetting to Handle Insertion Failure

```rust
// ❌ Wrong - assuming insertion always succeeds
let result = graph.insert_init(extract, init)?;  // What if it fails?

// ✅ Correct - handle potential failure
match graph.insert_init(extract, init) {
    Ok(result) => /* use result */,
    Err(e) => /* handle error */,
}
```

### 4. Modifying Graph During Insertion

```rust
// ❌ Wrong - graph might be locked or in inconsistent state
let mut graph = graph_ref.write();
let init = /* ... */;
graph.insert_init(extract, init)?;  // Might deadlock!
graph.insert_atom("something");     // Dangerous!

// ✅ Correct - release lock between operations
{
    let mut graph = graph_ref.write();
    graph.insert_init(extract, init)?;
}  // Lock released
{
    let mut graph = graph_ref.write();
    graph.insert_atom("something");
}
```

---

## Testing Patterns

### Test Insertion from Search

```rust
#[test]
fn test_insert_incomplete() {
    let _tracing = context_trace::init_test_tracing!();
    
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph, (ab, _) => [a, b]);
    
    // Search for longer sequence
    let query = vec![a, b, c, d];
    let result = Searchable::search::<InsertTraversal>(
        query,
        graph.clone()
    ).unwrap();
    
    // Should be incomplete
    assert!(!result.is_complete());
    
    // Convert to init interval
    let init = InitInterval::from(result);
    
    // Verify init interval structure
    assert_eq!(init.root, ab);  // Matched up to 'ab'
    assert_eq!(init.end_bound, AtomPosition(2));  // Need to extend from position 2
}
```

### Test Expected InitInterval Structure

```rust
#[test]
fn test_init_interval_structure() {
    let _tracing = context_trace::init_test_tracing!();
    
    // Setup
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {h, e, l, d});
    insert_patterns!(graph,
        (ld, ld_id) => [l, d],
        (heldld, _) => [h, e, ld, ld]
    );
    
    // Search for partial match
    let result = Searchable::search::<InsertTraversal>(
        vec![h, e, l, l],
        graph.clone()
    ).unwrap();
    
    assert!(!result.is_complete());
    let init = InitInterval::from(result);
    
    // Verify structure with expected cache
    assert_eq!(
        init,
        InitInterval {
            root: heldld,
            cache: build_trace_cache!(
                // Expected cache structure
                heldld => (BU {...}, TD {...}),
                // ... more entries
            ),
            end_bound: AtomPosition(3),  // Expected boundary
        }
    );
}
```

---

## Integration with Other Crates

### Depends On context-trace
- Uses Hypergraph for graph structure
- Uses TraceCache for state tracking
- Uses Token, Path types for identification
- Uses direction types for traversal

### Depends On context-search
- Uses search results to guide insertion
- Converts Response to InitInterval
- Leverages search cache for efficiency

### Used By context-read
- High-level read operations may trigger insertions
- Pattern construction uses insertion operations

---

## Debugging Insertion Operations

### Enable Detailed Logging

```bash
# All insertion logging
RUST_LOG=context_insert=debug cargo test

# Specific module
RUST_LOG=context_insert::split=trace cargo test

# With test output
RUST_TEST_LOG_STDOUT=1 RUST_LOG=debug cargo test my_test -- --nocapture
```

### Inspect Insertion State

```rust
// Add debug output
eprintln!("Init interval: {:#?}", init);
eprintln!("Root: {:?}, end_bound: {:?}", init.root, init.end_bound);

// Check cache contents
for (token, vertex_cache) in init.cache.entries.iter() {
    eprintln!("Cached: {:?}", token);
}

// During insertion (if accessible)
eprintln!("Split states: {:#?}", interval.states);
```

### Common Issues

**Insertion fails with panic:**
- Check if InitInterval is valid
- Verify end_bound is correct
- Check cache consistency
- Look for graph invariant violations

**Wrong patterns created:**
- Verify split points are correct
- Check join logic for boundary issues
- Inspect split cache for errors
- Validate role assignments

**Performance issues:**
- Check split cache size
- Look for redundant splits
- Profile split and join phases
- Consider simpler insertion strategies

---

## Advanced Topics

### Custom Result Extraction

```rust
pub trait InsertResult {
    fn extract_complete(...) -> ...;
    fn extract_interval(...) -> ...;
}

// Implement for your type to customize extraction
```

### Custom Split Strategies

Split behavior can be customized through:
- **Split position selection**: Where to split patterns
- **Cache management**: How split information is cached
- **Join strategies**: How components are merged

### Multi-Phase Insertion

Complex insertions may go through multiple rounds:
1. Initial search → InitInterval
2. First insertion → Partial result
3. Additional search → New InitInterval
4. Final insertion → Complete

---

## Best Practices

### 1. Always Search First
```rust
// ✅ Check before inserting
let result = graph.find_ancestor(query)?;
if !result.is_complete() {
    // Only insert if needed
}
```

### 2. Handle Both Complete and Incomplete
```rust
// ✅ Comprehensive handling
match graph.find_ancestor(query)? {
    r if r.is_complete() => {
        // Use existing pattern
    },
    incomplete => {
        // Insert new pattern
        let init = InitInterval::from(incomplete);
        graph.insert_init(extract, init)?;
    }
}
```

### 3. Validate InitInterval
```rust
// ✅ Check structure before using
let init = InitInterval::from(response);
assert!(init.end_bound.0 > 0);  // Sanity check
assert!(!init.cache.entries.is_empty());  // Has cache data
```

### 4. Use Appropriate Extraction Mode
```rust
// ✅ Choose right extraction for use case
let result = graph.insert_init(
    extract_complete,  // vs extract_interval
    init
)?;
```

---

## Next Steps

- **For graph operations**: See `context-trace` documentation
- **For search operations**: See `context-search` documentation  
- **For algorithm details**: See module-specific documentation
- **For examples**: See `src/tests/` directory
- **For questions**: See `QUESTIONS_FOR_AUTHOR.md` in root

---

## Known Issues / Questions

See `QUESTIONS_FOR_AUTHOR.md` for:
1. **end_bound semantics** - Clarification needed on cursor_position vs end_bound
2. **PathEnum variants** - How to handle Range/Postfix/Prefix
3. **RangeRole system** - Complete explanation of role purposes
4. **Split-join details** - Deeper algorithm documentation needed
