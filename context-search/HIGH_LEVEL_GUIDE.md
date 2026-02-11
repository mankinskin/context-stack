# Context-Search: High-Level Overview

> **Search and traversal engine for pattern matching in hierarchical hypergraphs.**

## What is Context-Search?

Context-search builds on context-trace to provide sophisticated pattern search and matching capabilities. It enables:

1. **Pattern Finding** - Locate sequences of tokens in complex hierarchical structures
2. **Flexible Matching** - Handle complete matches, partial matches, and search failures gracefully
3. **Multiple Search Strategies** - Breadth-first, depth-first, and custom traversal policies
4. **State Continuation** - Resume interrupted searches or use results for further operations

Think of it as the "query engine" - it knows how to find patterns in the graph and report detailed results about what was found or where the search stopped.

---

## Core Concepts

### 1. The Searchable Pattern

**What can be searched?**
- **Token arrays**: `&[Token]` - sequences of vertices
- **Patterns**: Existing pattern structures
- **Cursors**: Current position in a pattern with context
- **Paths**: Path definitions that can be navigated

All implement the `Searchable` trait, which provides a unified interface:

```rust
pub trait Searchable {
    fn search<K: TraversalKind>(
        self,
        graph: HypergraphRef,
    ) -> Result<Response, ErrorState>;
}
```

### 2. The Response Type (Unified Result)

**Recent API Change:** The old `CompleteState` / `IncompleteState` split was replaced with a unified `Response` type.

```rust
pub struct Response {
    pub cache: TraceCache,      // Trace cache built during search
    pub end: MatchedEndState,   // Terminal state with path and cursor
}
```

**Why unified?** Whether a search fully matches or partially matches, you get back a `Response`. Two orthogonal properties:
- **Query exhausted**: `response.query_exhausted() == true` - entire query was matched
- **Full token**: `response.is_entire_root() == true` - result is complete pre-existing token

**Key accessor methods:**
```rust
// Check query and match status (two independent properties)
response.query_exhausted() -> bool   // Has entire query been matched?
response.is_entire_root() -> bool     // Is result a complete token?
response.as_complete() -> Option<&IndexRangePath>  // Some if both true

// Get data (works for all result types)
response.root_token() -> Token
response.query_pattern() -> &PatternRangePath  
response.query_cursor() -> &PatternCursor
response.cursor_position() -> AtomPosition

// Unwrap complete (panics if not query_exhausted && is_entire_root)
response.expect_complete(msg) -> IndexRangePath
response.unwrap_complete() -> IndexRangePath
```

### 3. Search Strategies

Different search patterns require different traversal strategies:

#### Ancestor Search
Finds the largest containing pattern that matches the query:
```rust
let result = graph.find_ancestor(query)?;
// Searches upward through pattern hierarchies
```

Use when: You want to find what patterns contain your sequence

#### Traversal Kinds
- **InsertTraversal** - Used for insertion operations (finds where to insert)
- **AncestorSearchTraversal** - Finds containing patterns

### 4. How Search Works

**Step 1: Fold to Search Context**
```rust
// Input (various types)
let query = vec![a, b, c];

// Converted to search context internally
Searchable::search::<TraversalType>(query, graph)
```

**Step 2: Hierarchical Navigation**
The search engine:
1. Starts at query positions
2. Navigates pattern hierarchies using cached trace data
3. Tries to match patterns at increasing abstraction levels
4. Stops when:
   - Complete match found (success)
   - No more patterns to try (incomplete)
   - Error encountered (error state)

**Step 3: Result Construction**
```rust
Response {
    cache: TraceCache,           // What was learned during search
    end: MatchedEndState {       // Where/how search terminated
        path: PathCoverage,      // EntireRoot/Range/Postfix/Prefix
        cursor: PatternCursor,   // Current position in query
    }
}
```

### 5. Pattern Hierarchies

A key insight: sequences can exist at multiple levels of abstraction.

```
Example:
Atoms: h, e, l, l, o
Patterns:
  - "ll" = [l, l]
  - "ell" = [e, ll]
  - "hello" = [h, ell, o]
  
Query: [h, e, l, l, o]
Search finds: "hello" pattern (abstract representation)
Not just: sequence of individual atoms
```

The search engine explores these hierarchies efficiently using the trace cache.

---

## Key Types Reference

### Search Input Types

```rust
// Various ways to initiate search
&[Token]                         // Token arrays
Pattern                          // Existing patterns
PatternCursor                    // Cursor with position
RootedRolePath                   // Path definitions

// All implement Searchable
let result = query.search::<K>(graph)?;
```

### Search Result Types

```rust
// Unified result type
Response {
    cache: TraceCache,           // What was learned
    end: EndState,               // Terminal state
}

// End state variants
PathEnum::Complete(IndexRangePath)  // Full match
PathEnum::Range                     // Partial range match
PathEnum::Postfix                   // Matched postfix
PathEnum::Prefix                    // Matched prefix

// Error type
ErrorState {
    reason: ErrorReason,         // Why it failed
    // Additional context
}
```

### Traversal Types

```rust
// Traversal kind trait
pub trait TraversalKind {
    type Trav: HasGraph;
}

// Concrete traversal implementations
InsertTraversal                  // For insertion operations
AncestorSearchTraversal         // For ancestor finding

// Usage
Searchable::search::<InsertTraversal>(query, graph)
```

### Cursor and Position Types

```rust
PatternCursor {
    path: PatternRangePath,      // Current path
    position: AtomPosition,      // Current position
}

AtomPosition(usize)              // Position within pattern
```

---

## Common Operations

### Basic Pattern Search

```rust
use context_search::{Searchable, Response};
use context_trace::{Hypergraph, Token};

// Setup graph (abbreviated)
let mut graph = Hypergraph::default();
insert_atoms!(graph, {h, e, l, o});
insert_patterns!(graph,
    (hello, _) => [h, e, l, l, o]
);

// Search for sequence
let query = vec![h, e, l, l, o];
let result: Result<Response, ErrorState> = 
    Searchable::search::<InsertTraversal>(query, graph.clone())?;

// Handle result - check both properties
if result.query_exhausted() {
    if result.is_entire_root() {
        println!("Found exact match!");
        let path = result.expect_complete("query exhausted and full token");
        let root = path.root_parent();
        println!("Root token: {:?}", root);
    } else {
        println!("Query exhausted within token: {:?}", 
                 result.root_token());
    }
} else {
    println!("Query not exhausted at position: {:?}", 
             result.cursor_position());
}
```

### Find Ancestor Pattern

```rust
use context_search::Find;

// Find containing pattern
let query = vec![a, b, c];
let result = graph.find_ancestor(query)?;

// Check if query was exhausted and result is full token
if result.query_exhausted() && result.is_entire_root() {
    let path = result.as_complete().unwrap();
    println!("Found exact ancestor: {:?}", path.root_parent());
} else if result.query_exhausted() {
    println!("Query exhausted within: {:?}", result.root_token());
} else {
    println!("Query not exhausted");
    // Can still use the partial result
    let partial = result.query_pattern();
    println!("Got up to: {:?}", partial);
}
```

### Handle Partial Searches

```rust
// Search that might not exhaust query
let result = Searchable::search::<InsertTraversal>(query, graph)?;

if !result.query_exhausted() {
    // Extract information about where search stopped
    let position = result.cursor_position();
    let pattern = result.query_pattern();
    let root = result.root_token();
    
    println!("Stopped at position {} in pattern {:?}",
             position.0, root);
    
    // Use for insertion (see context-insert)
    let init = InitInterval::from(result);
}
```

### Access Search Cache

```rust
// The cache contains trace information
let result = graph.find_ancestor(query)?;

// Inspect what was learned
for (token, vertex_cache) in result.cache.entries.iter() {
    println!("Cached vertex {}: ", token.index);
    
    // Check bottom-up relationships
    for (pos, pos_cache) in &vertex_cache.bottom_up.entries {
        println!("  BU at position {:?}", pos);
    }
    
    // Check top-down relationships  
    for (pos, pos_cache) in &vertex_cache.top_down.entries {
        println!("  TD at position {:?}", pos);
    }
}
```

### Use Search Results for Insertion

```rust
use context_insert::InitInterval;

// Search first
let result = graph.find_ancestor(query)?;

// If query not exhausted, prepare for insertion
if !result.query_exhausted() {
    // Convert response to insertion initialization
    let init = InitInterval::from(result);
    
    // Now can insert (see context-insert docs)
    // ...
}
```

---

## API Patterns

### Pattern: Check Before Unwrap

```rust
// ✅ Safe pattern - check both properties
if response.query_exhausted() && response.is_entire_root() {
    let path = response.expect_complete("checked both above");
    let token = path.root_parent();
    // Use token
}

// ❌ Unsafe - might panic
let path = response.expect_complete("hope it's exhausted and exact!");
```

### Pattern: Extract Data Without Consuming

```rust
// ✅ Use references when possible
let pattern = response.query_pattern();  // Returns &PatternRangePath
let cursor = response.query_cursor();    // Returns &PatternCursor

// Only consume if needed
let path = response.expect_complete("msg");  // Consumes response
```

### Pattern: Handle Both Cases

```rust
// ✅ Comprehensive handling
match (response.query_exhausted(), response.is_entire_root()) {
    (true, true) => {
        // Perfect match: query exhausted and exact token
        println!("Exact: {:?}", response.as_complete().unwrap().root_parent());
    },
    (true, false) => {
        // Query matched but within token (intersection path)
        println!("Exhausted within: {:?}", response.root_token());
    },
    (false, _) => {
        // Query not fully matched
        println!("Not exhausted at: {:?}", response.cursor_position());
        // Can still use response for other operations
    }
}
```

---

## Module Structure

### `compare/`
Comparison operations for search validation
- `iterator.rs` - Compare iterators
- `state.rs` - Compare state management

### `container/`
Collection handling for search results
- `mod.rs` - Container traits
- `bft.rs` - Breadth-first queue (BftQueue)
- `extend.rs` - Container extension operations

### `cursor/`
Position tracking and path-based navigation
- `mod.rs` - PatternCursor definition and operations

### `match/`
Pattern matching algorithms
- `iterator.rs` - Match iteration
- `root_cursor.rs` - Root-level cursor management

### `search/`
Main search algorithms and context
- `mod.rs` - Find trait and search result type
- `context.rs` - AncestorPolicy and search context
- `searchable.rs` - Searchable trait and error states
- `final_state.rs` - Final state handling

### `state/`
Search state management
- `mod.rs` - TraversalState
- `start.rs` - Searchable trait implementation
- `result.rs` - Response type and methods
- `end/` - EndState and PathEnum
  - `mod.rs` - EndState definition
  - `path.rs` - PathEnum variants

### `traversal/`
Configurable traversal policies
- `mod.rs` - TraversalKind trait

---

## Search Algorithms Explained

### Ancestor Search Algorithm

**Goal:** Find the largest pattern that contains the query sequence.

**Steps:**
1. Start with query tokens
2. Build trace cache tracking parent-child relationships
3. Navigate up the pattern hierarchy
4. Try to match at each level
5. Return the highest matching pattern

**Example:**
```
Query: [a, b, c]
Graph:
  - ab = [a, b]
  - abc = [ab, c]
  - abcd = [abc, d]

Result: abc (not abcd, because d isn't in query)
```

### Pattern Matching Flow

```
Input Tokens → Fold to Context → Traverse Graph
    ↓                ↓                  ↓
[a,b,c]    →  PatternCursor  →  Navigate hierarchy
                                         ↓
                                   Try matches
                                         ↓
                                 ┌───────┴────────┐
                                 ↓                ↓
                           Complete         Incomplete
                                 ↓                ↓
                          Return path      Return state
```

---

## Performance Characteristics

### Time Complexity
- **Pattern search**: O(d * p) where d = pattern depth, p = patterns per level
- **Ancestor search**: O(h) where h = height of pattern hierarchy
- **Cache lookup**: O(1) - hashmap access
- **Result construction**: O(k) where k = result path length

### Space Complexity
- **Search state**: O(d) where d = current depth
- **Cache**: O(v) where v = visited vertices
- **Result**: O(k) where k = path length

### Optimization Strategies
- **Cache reuse**: Leverage existing trace caches from previous searches
- **Early termination**: Stop as soon as a match is found or impossible
- **Lazy evaluation**: Only compute what's needed for current operation

---

## Common Gotchas

### 1. Forgetting to Check query_exhausted() and is_entire_root()

```rust
// ❌ Wrong - might panic
let path = response.expect_complete("found");

// ✅ Correct - check both properties
if response.query_exhausted() && response.is_entire_root() {
    let path = response.expect_complete("checked");
}
// Or handle all cases
if response.query_exhausted() {
    if response.is_entire_root() {
        // Exact token match
    } else {
        // Query exhausted but intersection path
    }
}
```

### 2. Wrong Traversal Type

```rust
// ❌ Wrong - not a TraversalKind
Searchable::search::<BreadthFirst>(query, graph)

// ✅ Correct - use actual traversal implementation
Searchable::search::<InsertTraversal>(query, graph)
```

### 3. Accessing Private EndState Fields

```rust
// ❌ Wrong - fields are private
let path = response.end.path;      // Error!
let cursor = response.end.cursor;  // Error!

// ✅ Correct - use accessor methods
let token = response.root_token();
let cursor = response.query_cursor();
```

### 4. Consuming Response Too Early

```rust
// ❌ Wrong - response consumed, can't use again
let path = response.expect_complete("msg");
let token = response.root_token();  // Error: response moved

// ✅ Correct - get data before consuming
let token = response.root_token();  // Borrows
let path = response.expect_complete("msg");  // Consumes
```

### 5. Not Using root_parent() After expect_complete()

```rust
// ❌ Wrong - expect_complete returns IndexRangePath, not Token
let token: Token = response.expect_complete("msg");  // Type error!

// ✅ Correct - call root_parent()
let path = response.expect_complete("msg");
let token: Token = path.root_parent();

// Or chain them
let token: Token = response.expect_complete("msg").root_parent();
```

---

## Testing Patterns

### Test Structure

```rust
#[test]
fn test_pattern_search() {
    // Initialize tracing
    let _tracing = context_trace::init_test_tracing!();
    
    // Setup graph
    let mut graph = Hypergraph::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph, (abc, _) => [a, b, c]);
    
    // Perform search
    let query = vec![a, b, c];
    let result = Searchable::search::<InsertTraversal>(
        query, 
        graph.clone()
    ).unwrap();
    
    // Assert expectations
    assert!(result.query_exhausted());
    assert!(result.is_entire_root());
    assert_eq!(result.root_token(), abc);
}
```

### Testing Partial Searches

```rust
#[test]
fn test_partial_search() {
    let _tracing = context_trace::init_test_tracing!();
    
    let mut graph = Hypergraph::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph, (ab, _) => [a, b]);
    
    // Query includes c, d which aren't in ab
    let query = vec![a, b, c, d];
    let result = Searchable::search::<InsertTraversal>(
        query,
        graph.clone()
    ).unwrap();
    
    // Query should not be exhausted
    assert!(!result.query_exhausted());
    
    // Can still get useful information
    let position = result.cursor_position();
    assert_eq!(position, AtomPosition(2));  // Stopped at 'c'
}
```

---

## Integration with Other Crates

### Depends On context-trace
- Uses Hypergraph for graph structure
- Uses TraceCache for efficient navigation
- Uses Path types for results
- Uses Token and VertexIndex for identification

### Used By context-insert
- Search results guide insertion decisions
- Response converted to InitInterval for insertion
- Cache reused during insertion operations

### Used By context-read
- High-level reading operations use search to locate patterns
- Results interpreted for sequence extraction

---

## Debugging Search Operations

### Enable Detailed Logging

```bash
# All search logging
RUST_LOG=context_search=debug cargo test

# Specific module
RUST_LOG=context_search::search=trace cargo test

# With stdout
RUST_TEST_LOG_STDOUT=1 RUST_LOG=debug cargo test my_test
```

### Inspect Search Results

```rust
// Add debug output
if !response.is_complete() {
    eprintln!("Search stopped at: {:?}", response.cursor_position());
    eprintln!("Current pattern: {:?}", response.query_pattern());
    eprintln!("Root so far: {:?}", response.root_token());
}

// Pretty-print cache
use context_trace::logging::pretty;
eprintln!("Cache: {}", pretty(&response.cache));
```

### Common Issues

**Search returns not exhausted when expected:**
- Check if all patterns exist in graph
- Verify query tokens are correct
- Inspect cache to see what was found
- Check if pattern hierarchy is as expected

**Search panics on expect_complete():**
- Always check `query_exhausted() && is_entire_root()` first
- Use `as_complete()` for safe Option handling
- Add logging to see why not exhausted or not full token

**Query exhausted but not full token:**
- Result is an intersection path within a token
- Query matched but doesn't align with token boundaries
- May need different search strategy or pattern structure

**Search is slow:**
- Check cache size (might be very large)
- Profile to find hot spots
- Consider simpler traversal policy
- Check for redundant pattern structures

---

## Advanced Topics

### Custom Traversal Policies

You can implement custom traversal strategies:

```rust
pub trait TraversalKind {
    type Trav: HasGraph;
}

// Implement for your type
struct MyTraversal;

impl TraversalKind for MyTraversal {
    type Trav = MyTravImpl;
}
```

### Search Continuation

Search can be resumed from previous state:

```rust
// First search
let result1 = query.search::<K>(graph)?;

// Use result1.cache in next operation
// (specific API depends on use case)
```

### Cache Management

```rust
// Create cache
let mut cache = TraceCache::new();

// Populate during search
let result = query.search::<K>(graph)?;
cache = result.cache;

// Reuse in next search
// (cache is part of Response)
```

---

## Next Steps

- **For graph operations**: See `context-trace` documentation
- **For insertion operations**: See `context-insert` documentation
- **For algorithm details**: See `PATTERN_MATCHING_EXAMPLE.md`
- **For testing strategies**: See `TESTING_PLAN.md`
- **For examples**: See `src/tests/search/` directory
