# Context-Trace: High-Level Overview

> **Foundation crate providing graph structures, path operations, and tracing infrastructure for the context framework.**

## What is Context-Trace?

Context-trace is the foundational layer of the context framework. It provides:

1. **Hypergraph Data Structure** - A specialized graph where vertices can represent both simple atoms and complex patterns (collections of other vertices)
2. **Path Navigation System** - Role-based paths for traversing the graph with different access patterns
3. **Bidirectional Tracing** - Cache-aware traversal that tracks both bottom-up and top-down relationships
4. **Thread-Safe Wrappers** - Arc/RwLock-based graph references for concurrent access

Think of it as the "storage layer" - it knows how to store, organize, and navigate graph data, but doesn't implement search or modification algorithms.

---

## Core Concepts

### 1. Hypergraph Structure

**What's a hypergraph?**
A hypergraph is like a regular graph, but edges can connect multiple vertices at once. In our case:
- **Atoms** are leaf nodes (terminal vertices with no children)
- **Patterns** are composite nodes that contain sequences of other vertices
- **Vertices** can be referenced from multiple patterns (shared structure)

```
Example:
Atoms: a, b, c
Patterns:
  - ab = [a, b]
  - abc = [a, b, c] OR [ab, c]  ← Multiple representations!
```

**Key insight:** The same logical sequence can have multiple physical representations in the graph, enabling efficient storage and powerful pattern matching.

### 2. Tokens and Vertices

- **VertexIndex**: A unique identifier (essentially a usize) for each vertex in the graph
- **Token**: `{ index: VertexIndex, width: TokenWidth }` - represents a vertex with its span width
- **Width**: How many atoms a pattern spans (atoms have width 1, patterns aggregate their children's widths)

```rust
// An atom has width 1
Token { index: 5, width: 1 }

// A pattern "abc" (where a,b,c are atoms) has width 3
Token { index: 10, width: 3 }
```

### 3. Paths and Navigation

**Role-Based Paths:**
Different operations need different path representations. The "role" determines what information the path carries:

- **IndexRangePath** - Complete paths with all indices (for finished traversals)
- **PatternRangePath** - Pattern-level paths (for active navigation)
- **RootedRolePath** - Path with explicit root (for hierarchical operations)

**Why roles?** Different operations need different invariants:
- Complete paths must have all positions filled
- Active navigation needs cursor position tracking
- Hierarchical operations need explicit roots

### 4. Bidirectional Tracing

The graph tracks relationships in both directions:

- **Bottom-Up (BU)**: From atoms toward containing patterns
  - "What patterns contain this atom?"
  - Stored per atom position within patterns

- **Top-Down (TD)**: From patterns toward their components
  - "What are the components of this pattern?"
  - Stored per child position in parent patterns

**TraceCache** stores this bidirectional information, enabling efficient traversal without re-computing relationships.

### 5. Directions: Left and Right

Graph operations can proceed in two directions:
- **Left**: Moving toward the start of a pattern (decreasing indices)
- **Right**: Moving toward the end of a pattern (increasing indices)

These are used for pattern matching, path extension, and boundary operations.

---

## Key Types Reference

### Graph Types

```rust
// Core graph
Hypergraph<G: GraphKind>        // The graph structure itself
HypergraphRef                    // Arc<RwLock<Hypergraph>> - thread-safe
BaseGraphKind                    // Default graph type (no special behavior)

// Creating graphs
let graph = Hypergraph::<BaseGraphKind>::default();
let graph_ref = HypergraphRef::new(graph);
```

### Vertex and Token Types

```rust
VertexIndex                      // Unique ID for a vertex (wraps usize)
Token {                          // Vertex with width information
    index: VertexIndex,
    width: TokenWidth,
}
AtomPosition                     // Position within a pattern (wraps usize)
PatternId                        // UUID identifying a specific pattern instance
```

### Path Types

```rust
// Generic role-based path
RolePath<R: RangeRole>          // Path with role R

// Concrete path types
IndexRangePath                   // Complete paths (all indices known)
PatternRangePath                 // Pattern paths (for navigation)
RootedRolePath<R>               // Path with explicit root

// Path operations
path.root_parent() -> Token      // Get root token
path.start_position() -> AtomPosition
path.to_rooted(root) -> RootedRolePath
```

### Cache Types

```rust
TraceCache                       // Bidirectional trace cache
VertexCache {                    // Per-vertex cache entry
    bottom_up: DirectedPositions,   // BU relationships
    top_down: DirectedPositions,    // TD relationships
    index: Token,                   // The vertex token
}

// Access cache
cache.get(&token) -> Option<&VertexCache>
cache.entries.iter() -> impl Iterator<Item = (Token, VertexCache)>
```

### Direction Types

```rust
Direction                        // Trait for Left/Right
Left                            // Direction type
Right                           // Direction type

// Usage
fn move_in_direction<D: Direction>(...)
```

---

## Common Operations

### Creating and Populating Graphs

```rust
use context_trace::{Hypergraph, BaseGraphKind};

// Create empty graph
let mut graph = Hypergraph::<BaseGraphKind>::default();

// Insert atoms (in tests, use insert_atoms! macro)
let a_idx = graph.insert_atom("a");
let b_idx = graph.insert_atom("b");

// Create tokens
let a = Token { index: a_idx, width: TokenWidth(1) };
let b = Token { index: b_idx, width: TokenWidth(1) };

// Insert pattern (in tests, use insert_patterns! macro)
let ab_pattern = vec![a, b];
let ab_idx = graph.insert_pattern(ab_pattern);
```

### Accessing Vertices

```rust
// Get vertex data (panics if not found)
let vertex = graph.expect_vertex(token.index);

// Try get vertex data (returns Option)
let maybe_vertex = graph.get_vertex(token.index);

// Get vertex data reference
let data: &VertexData = graph.vertex_data(token.index);

// Check if vertex is an atom
if vertex.is_atom() {
    println!("This is a leaf node");
}
```

### Navigating Patterns

```rust
// Get children of a pattern
for child_token in vertex.children() {
    println!("Child: {:?}", child_token);
}

// Get parents of a vertex
for parent in vertex.parents() {
    println!("Parent pattern: {:?}", parent);
}

// Get pattern at specific child location
let child_loc = ChildLocation { ... };
let pattern = vertex.get_child_pattern(&child_loc);
```

### Working with Paths

```rust
// Create a rooted path
let role_path = RolePath::new(child_locations);
let rooted = role_path.to_rooted(root_token);

// Navigate path
let root = rooted.root_parent();           // Get root token
let pos = rooted.start_position();         // Starting position
let end = rooted.end_position();           // Ending position

// Convert between path types
let index_path: IndexRangePath = complete_path.into();
```

### Using Trace Cache

```rust
// Build cache during traversal
let cache = TraceCache::new();

// Access cached data
if let Some(vertex_cache) = cache.get(&token) {
    // Check bottom-up relationships
    for (position, pos_cache) in &vertex_cache.bottom_up.entries {
        // Process BU data
    }
    
    // Check top-down relationships
    for (position, pos_cache) in &vertex_cache.top_down.entries {
        // Process TD data
    }
}
```

### Thread-Safe Operations

```rust
use context_trace::HypergraphRef;

// Create thread-safe wrapper
let graph_ref = HypergraphRef::new(graph);

// Read access (shared)
{
    let graph = graph_ref.read();
    let vertex = graph.expect_vertex(idx);
}  // Lock released

// Write access (exclusive)
{
    let mut graph = graph_ref.write();
    graph.insert_atom("new_atom");
}  // Lock released

// Clone ref for sharing across threads
let graph_ref2 = graph_ref.clone();  // Cheap Arc clone
```

---

## Module Structure

### `direction/`
Direction types and operations (Left/Right)
- `match.rs` - Direction matching logic
- `merge.rs` - Direction merging operations  
- `pattern.rs` - Directional pattern operations

### `graph/`
Core graph structure and operations
- `mod.rs` - Hypergraph definition
- `kind.rs` - Graph type system (BaseGraphKind, GraphKind trait)
- `insert.rs` - Vertex and pattern insertion
- `validation.rs` - Graph invariant checking
- **`getters/`** - Data access operations
  - `mod.rs` - ErrorReason and basic getters
  - `child.rs`, `parent.rs` - Relationship access
  - `pattern.rs`, `token.rs` - Pattern and token operations
  - `vertex.rs` - Vertex data access
- **`vertex/`** - Vertex system
  - `data.rs` - VertexData structure
  - `atom.rs` - Atom types and operations
  - `token.rs` - Token types and operations
  - `pattern/` - Pattern types and operations
  - `parent.rs` - Parent relationship data
  - `location/` - Child and pattern locations

### `path/`
Path types and navigation
- **`accessors/`** - Path access patterns
  - `border.rs` - Path boundary operations
  - `complete.rs` - Complete path access
  - `root.rs` - Root access
  - `child/` - Child navigation
  - `has_path.rs` - Path traits
- **`mutators/`** - Path modifications
  - `append.rs`, `pop.rs` - Add/remove elements
  - `lower.rs`, `raise.rs` - Hierarchical movement
  - `simplify.rs` - Path simplification
  - `move_key.rs` - Element movement
- **`structs/`** - Path data structures
  - `query.rs` - Query paths
  - `role.rs` - Role-based paths
  - `rooted/` - Rooted path types

### `trace/`
Tracing operations and caching
- `mod.rs` - TraceCtx definition
- `has_graph.rs` - Graph access traits
- `traceable.rs` - Traceability operations
- `command.rs` - Trace commands (prefix/postfix/range)
- **`cache/`** - Trace caching system
  - `mod.rs` - TraceCache definition
  - `position.rs` - Position-based caching
  - `vertex.rs` - Vertex caching
  - `key/` - Cache key management
- **`child/`** - Child tracing operations
  - `state.rs` - Child tracing states
  - `iterators.rs` - Child iteration
  - `bands/` - Band expansion operations
  - `positions.rs` - Position tracking
- **`state/`** - Tracing state management

### `logging/`
Logging and debugging utilities
- `tracing_utils.rs` - Test tracing setup (init_test_tracing! macro)
- `pretty.rs` - Pretty-printing for debug output

---

## Design Patterns

### Pattern: Safe Graph Modification
```rust
// Always use thread-safe wrapper for modifications
let graph_ref = HypergraphRef::new(graph);

{
    let mut graph = graph_ref.write();  // Acquire write lock
    graph.insert_pattern(pattern);      // Modify
}  // Lock automatically released

// Read operations can happen concurrently
let graph = graph_ref.read();
```

### Pattern: Path Construction
```rust
// Build paths incrementally
let mut locations = Vec::new();
locations.push(child_location_1);
locations.push(child_location_2);

let path = RolePath::new(locations);
let rooted = path.to_rooted(root_token);
```

### Pattern: Cache-Aware Traversal
```rust
// Check cache before computing
if let Some(cached) = cache.get(&token) {
    // Use cached data
} else {
    // Compute and cache
    let result = compute_expensive_operation();
    cache.insert(token, result);
}
```

---

## Testing Utilities

Context-trace provides several macros for testing:

```rust
#[cfg(test)]
use context_trace::{
    insert_atoms,       // Insert multiple atoms easily
    insert_patterns,    // Insert patterns with named bindings
    build_trace_cache,  // Build expected cache structures
    init_test_tracing,  // Initialize tracing for tests
};

#[test]
fn my_test() {
    // Initialize test logging
    let _tracing = init_test_tracing!();
    
    // Create graph
    let mut graph = Hypergraph::<BaseGraphKind>::default();
    
    // Insert atoms - creates variables with atom names
    insert_atoms!(graph, {a, b, c, d});
    
    // Insert patterns - creates pattern variables
    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
        (cd, cd_id) => [c, d],
        (abcd, abcd_id) => [ab, cd]
    );
    
    // Now you can use: a, b, c, d (Tokens)
    //                  ab, cd, abcd (Tokens)
    //                  ab_id, cd_id, abcd_id (PatternIds)
}
```

**Note:** Test logs are automatically written to `target/test-logs/<test_name>.log` and preserved on failure.

---

## Performance Characteristics

### Time Complexity (Estimated)
- **Vertex lookup**: O(1) - direct index access
- **Pattern insertion**: O(k) where k is pattern length
- **Parent lookup**: O(p) where p is number of parents
- **Child iteration**: O(c) where c is number of children
- **Cache lookup**: O(1) - hashmap access

### Space Complexity
- **Per vertex**: O(1) + O(p) for parents + O(c) for children
- **Cache**: O(v) where v is number of cached vertices
- **Paths**: O(d) where d is path depth

### Thread Safety Overhead
- **RwLock**: Read operations can run concurrently
- **Write lock**: Exclusive access, may block readers
- **Arc clone**: Cheap pointer copy

---

## Common Gotchas

### 1. VertexIndex vs Token
```rust
// ❌ Wrong - comparing different types
if vertex_index == token { ... }

// ✅ Correct - compare indices
if vertex_index == token.index { ... }
```

### 2. Forgetting Width
```rust
// ❌ Wrong - atoms need width 1
Token { index: atom_idx, width: 0 }

// ✅ Correct - width must match actual span
Token { index: atom_idx, width: TokenWidth(1) }
```

### 3. Lock Lifetime Issues
```rust
// ❌ Wrong - holding lock too long
let graph = graph_ref.read();
expensive_operation();          // Lock held during this!
let vertex = graph.expect_vertex(idx);

// ✅ Correct - minimize lock scope
let vertex = {
    let graph = graph_ref.read();
    graph.expect_vertex(idx).clone()
};  // Lock released
expensive_operation();
```

### 4. Path Root Access
```rust
// ❌ Wrong - trying to get root from non-rooted path
let root = path.root_parent();  // May not exist!

// ✅ Correct - check if rooted first or convert
let rooted = path.to_rooted(root_token);
let root = rooted.root_parent();
```

---

## Integration with Other Crates

### Used By context-search
- Provides graph structure for search operations
- TraceCache used for efficient search state tracking
- Path types used for search results

### Used By context-insert  
- Graph structure modified by insertion operations
- Paths used to represent insertion targets
- Cache leveraged for split-join operations

### Used By context-read
- Graph structure for reading operations
- Path types for sequence navigation
- Token and vertex types for data access

---

## Next Steps

- **For search operations**: See `context-search` documentation
- **For insertions**: See `context-insert` documentation  
- **For high-level reading**: See `context-read` documentation
- **For algorithm details**: See individual module documentation
- **For examples**: See `src/tests/` directory
