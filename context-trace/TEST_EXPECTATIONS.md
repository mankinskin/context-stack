# Test Expectations and Assumptions

This document explains what each test validates and the assumptions they make about the `context-trace` API behavior.

## Overview

The test suite contains 39 tests organized into the following categories:
- **TraceCache Tests (10)**: Core cache operations and data structure behavior
- **MoveRootIndex Tests (5)**: Navigation through pattern children at the root level
- **MoveLeaf Tests (7)**: Navigation through pattern children at leaf locations
- **Path Operations Tests (7)**: Combined operations with TraceCtx
- **Pattern Strings Tests (3)**: Pattern string conversion and verification
- **Move Key Tests (9)**: AtomPosition arithmetic and movement
- **Path Append Tests (1)**: SubPath construction

---

## TraceCache Tests

### Core Assumptions
1. **TraceCache is a hierarchical data structure** with two levels:
   - **Vertex level**: Maps `TokenIndex` → `VertexCache`
   - **Position level**: Maps `DirectedKey` (index + position + direction) → `PositionCache`

2. **Cache entries are created lazily** using `force_mut()`

3. **Directed keys distinguish between bottom-up and top-down traversal directions**

### Test: `trace_cache_new_contains_start_vertex_but_no_positions`

**What it tests:**
- `TraceCache::new(index)` creates the vertex entry but no position entries

**Assumptions validated:**
```rust
let cache = TraceCache::new(ab);
assert!(cache.exists_vertex(&ab));        // ✓ Vertex exists
assert!(!cache.exists(&DirectedKey::from(ab))); // ✓ No positions yet
```

**Why this matters:** Distinguishes between vertex-level and position-level existence. A vertex can exist without having any traced positions yet.

---

### Test: `trace_cache_force_mut_creates_position_and_is_gettable`

**What it tests:**
- `force_mut(&key)` creates a `PositionCache` if it doesn't exist
- The created cache is retrievable via `get(&key)`
- Default `PositionCache` starts empty

**Assumptions validated:**
```rust
let _ = cache.force_mut(&key);
assert!(cache.exists(&key));                    // ✓ Position now exists
let pos_cache = cache.get(&key).unwrap();
assert_eq!(pos_cache.num_parents(), 0);         // ✓ Empty by default
assert_eq!(pos_cache.num_bu_edges(), 0);        // ✓ No edges yet
```

**Why this matters:** Confirms the lazy initialization pattern and default state of position caches.

---

### Test: `trace_cache_add_state_creates_new_entries`

**What it tests:**
- `add_state(edit, add_edges)` creates new cache entries from trace edges
- Returns `(key, was_new)` where `was_new=true` for new entries

**Assumptions validated:**
```rust
let edit = NewTraceEdge::<BottomUp> { /* ... */ };
let (key, was_new) = cache.add_state(edit, true);
assert!(was_new);                    // ✓ First time = new
assert_eq!(key.index, ab);           // ✓ Key points to target
assert!(cache.exists(&key));         // ✓ Entry was created
```

**Why this matters:** Confirms that `add_state` is the primary way to populate the cache during tracing, and it correctly reports whether an entry was newly created.

---

### Test: `trace_cache_add_state_idempotent_for_existing_entries`

**What it tests:**
- Adding the same state twice doesn't duplicate entries
- Second call returns `was_new=false`

**Assumptions validated:**
```rust
let (_, was_new1) = cache.add_state(edit.clone(), true);
assert!(was_new1);                   // ✓ First time = new

let (_, was_new2) = cache.add_state(edit, true);
assert!(!was_new2);                  // ✓ Second time = not new
```

**Why this matters:** Ensures cache operations are idempotent, preventing duplicate entries during traversal.

---

### Test: `trace_cache_add_state_with_edges_creates_bottom_edges`

**What it tests:**
- When `add_edges=true`, `add_state` creates bottom-up edges in the `PositionCache`
- Edges connect related positions in the trace graph

**Assumptions validated:**
```rust
let edit = NewTraceEdge::<BottomUp> { /* from ab to abc */ };
let (key, _) = cache.add_state(edit, true);  // add_edges=true
let pos_cache = cache.get(&key).unwrap();
assert!(pos_cache.num_bu_edges() > 0);       // ✓ Edges created
```

**Why this matters:** Confirms that the `add_edges` flag controls whether edge relationships are stored, which is essential for path reconstruction.

---

### Test: `trace_cache_add_state_without_edges_creates_no_bottom_edges`

**What it tests:**
- When `add_edges=false`, no edges are created even though the position exists

**Assumptions validated:**
```rust
let (key, _) = cache.add_state(edit, false); // add_edges=false
let pos_cache = cache.get(&key).unwrap();
assert_eq!(pos_cache.num_bu_edges(), 0);     // ✓ No edges
```

**Why this matters:** Allows performance optimization when only position existence matters, not the full edge graph.

---

### Test: `trace_cache_extend_merges_entries`

**What it tests:**
- `TraceCache` implements `Extend` trait
- Merging two caches preserves entries from both
- Uses the `build_trace_cache!` macro for complex cache construction

**Assumptions validated:**
```rust
let mut a_clone = a.clone();
a_clone.extend(b.entries.into_iter());

for (k, _) in a.entries.iter() {
    assert!(a_clone.entries.contains_key(k)); // ✓ Original entries preserved
}
```

**Why this matters:** Enables combining traces from different search branches or parallel operations.

---

### Test: `trace_cache_extend_merges_positions_for_same_vertex`

**What it tests:**
- When merging caches with the same vertex but different positions, both positions are preserved
- Position-level merging works correctly

**Assumptions validated:**
```rust
// cache_a has: ab at position 1 (from 'a')
// cache_b has: ab at position 2 (from 'b')
let mut merged = cache_a.clone();
merged.extend(cache_b.entries.into_iter());

let vertex_cache = merged.get_vertex(&ab).unwrap();
assert!(vertex_cache.bottom_up.get(&1.into()).is_some()); // ✓ Position 1 kept
assert!(vertex_cache.bottom_up.get(&2.into()).is_some()); // ✓ Position 2 kept
```

**Why this matters:** Ensures that different traversal paths to the same vertex are both preserved, which is critical for finding all matches.

---

### Test: `trace_cache_multiple_directed_positions`

**What it tests:**
- A single vertex can have multiple positions in each direction
- Bottom-up and top-down positions are stored separately

**Assumptions validated:**
```rust
cache.force_mut(&DirectedKey::up(abc, 1));
cache.force_mut(&DirectedKey::up(abc, 2));
cache.force_mut(&DirectedKey::down(abc, 1));

let vertex_cache = cache.get_vertex(&abc).unwrap();
assert!(vertex_cache.bottom_up.get(&1.into()).is_some());  // ✓ BU pos 1
assert!(vertex_cache.bottom_up.get(&2.into()).is_some());  // ✓ BU pos 2
assert!(vertex_cache.top_down.get(&1.into()).is_some());   // ✓ TD pos 1
```

**Why this matters:** Validates the bidirectional nature of the cache, supporting both forward and backward traversal.

---

## MoveRootIndex Tests

### Core Assumptions
1. **Root index points to a child in a pattern** (e.g., in pattern `[a, b, c]`, valid indices are 0, 1, 2)
2. **Right direction advances** (0→1→2), **Left direction retracts** (2→1→0)
3. **ControlFlow::Continue** indicates successful movement
4. **ControlFlow::Break** indicates boundary reached (no more movement possible)
5. **Index remains unchanged when Break occurs**

### Test: `move_root_index_right_advances_through_pattern`

**What it tests:**
- Moving right increments the root_entry index
- Each move returns Continue until the end is reached

**Assumptions validated:**
```rust
// Pattern: [a, b, c, d] at indices 0, 1, 2, 3
let mut path = IndexEndPath::new_location(loc_at_index_0);

MoveRootIndex::<Right>::move_root_index(&mut path, &graph);
assert_eq!(path.root_entry, 1); // ✓ Advanced to 'b'

MoveRootIndex::<Right>::move_root_index(&mut path, &graph);
assert_eq!(path.root_entry, 2); // ✓ Advanced to 'c'
```

**Why this matters:** Confirms that root index movement correctly traverses pattern children in order.

---

### Test: `move_root_index_right_breaks_at_pattern_end`

**What it tests:**
- Moving right at the last valid index returns Break
- Index remains unchanged after Break

**Assumptions validated:**
```rust
// Start at last index (2 in [a, b, c])
let mut path = IndexEndPath::new_location(loc_at_index_2);

let result = MoveRootIndex::<Right>::move_root_index(&mut path, &graph);
assert_eq!(result, ControlFlow::Break(()));  // ✓ Can't go further
assert_eq!(path.root_entry, 2);              // ✓ Index unchanged
```

**Why this matters:** Ensures boundary detection prevents out-of-bounds access.

---

### Test: `move_root_index_left_retracts_through_pattern`

**What it tests:**
- Moving left decrements the root_entry index
- Movement works in reverse direction

**Assumptions validated:**
```rust
// Start at index 3 in [a, b, c, d]
let mut path = IndexEndPath::new_location(loc_at_index_3);

MoveRootIndex::<Left>::move_root_index(&mut path, &graph);
assert_eq!(path.root_entry, 2); // ✓ Retracted to 'c'

MoveRootIndex::<Left>::move_root_index(&mut path, &graph);
assert_eq!(path.root_entry, 1); // ✓ Retracted to 'b'
```

**Why this matters:** Confirms bidirectional navigation capability.

---

### Test: `move_root_index_left_breaks_at_pattern_start`

**What it tests:**
- Moving left at index 0 returns Break
- Index remains at 0 after Break

**Assumptions validated:**
```rust
// Start at first index (0)
let mut path = IndexEndPath::new_location(loc_at_index_0);

let result = MoveRootIndex::<Left>::move_root_index(&mut path, &graph);
assert_eq!(result, ControlFlow::Break(()));  // ✓ Can't go before 0
assert_eq!(path.root_entry, 0);              // ✓ Index unchanged
```

**Why this matters:** Ensures safe traversal boundaries in both directions.

---

### Test: `move_root_index_works_with_compound_patterns`

**What it tests:**
- Root index movement works on patterns containing compound children
- Example: `[ab, cd]` has two valid indices (0, 1) even though children are themselves patterns

**Assumptions validated:**
```rust
// Pattern: [ab, cd] where ab=[a,b] and cd=[c,d]
let mut path = IndexEndPath::new_location(loc_at_index_0); // Points to 'ab'

MoveRootIndex::<Right>::move_root_index(&mut path, &graph);
assert_eq!(path.root_entry, 1); // ✓ Now points to 'cd'

MoveRootIndex::<Right>::move_root_index(&mut path, &graph);
assert_eq!(result, ControlFlow::Break(())); // ✓ At end
```

**Why this matters:** Validates that movement operates at the correct level of granularity (pattern children, not atomic elements).

---

## MoveLeaf Tests

### Core Assumptions
1. **Leaf position (sub_index) points to a child within a ChildLocation's pattern**
2. **Similar to MoveRootIndex but operates on ChildLocation instead of RootedPath**
3. **Same ControlFlow semantics: Continue = success, Break = boundary**

### Test: `move_leaf_right_advances_through_pattern`

**What it tests:**
- Moving right increments sub_index within the ChildLocation
- Returns Continue for valid moves

**Assumptions validated:**
```rust
// Pattern: [a, b, c, d]
let mut loc = ChildLocation::new(abcd, abcd_id, 0); // At 'a'

MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
assert_eq!(loc.sub_index, 1); // ✓ Advanced to 'b'

MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
assert_eq!(loc.sub_index, 2); // ✓ Advanced to 'c'
```

**Why this matters:** Confirms leaf-level navigation through pattern children.

---

### Test: `move_leaf_right_breaks_at_pattern_end`

**What it tests:**
- Moving right at the last index returns Break
- sub_index unchanged after Break

**Assumptions validated:**
```rust
let mut loc = ChildLocation::new(abc, abc_id, 2); // At last position

let result = MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
assert_eq!(result, ControlFlow::Break(()));  // ✓ Boundary reached
assert_eq!(loc.sub_index, 2);                // ✓ Index unchanged
```

**Why this matters:** Ensures safe boundary detection at leaf level.

---

### Test: `move_leaf_left_retracts_through_pattern` & `move_leaf_left_breaks_at_pattern_start`

**What they test:**
- Leftward movement decrements sub_index
- Break at index 0 when moving left

**Assumptions validated:**
Same semantics as MoveRootIndex but for ChildLocation.sub_index instead of RootedPath.root_entry.

**Why this matters:** Validates bidirectional leaf navigation.

---

### Test: `move_leaf_works_with_compound_children`

**What it tests:**
- Leaf movement works when pattern children are themselves patterns
- Example: `[ab, cd]` where ab and cd are compound

**Assumptions validated:**
```rust
// Pattern: [ab, cd]
let mut loc = ChildLocation::new(abcd, abcd_id, 0); // At 'ab'

MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
assert_eq!(loc.sub_index, 1); // ✓ Now at 'cd'

MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
assert_eq!(result, ControlFlow::Break(())); // ✓ At end
```

**Why this matters:** Confirms correct granularity of leaf-level operations.

---

### Test: `move_leaf_sequential_movements`

**What it tests:**
- Multiple sequential moves work correctly
- Tests the entire traversal through a 5-element pattern

**Assumptions validated:**
```rust
// Pattern: [a, b, c, d, e] (indices 0-4)
let mut loc = ChildLocation::new(abcde, abcde_id, 0);

for expected_idx in 1..=4 {
    let result = MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
    assert_eq!(result, ControlFlow::Continue(())); // ✓ All moves succeed
    assert_eq!(loc.sub_index, expected_idx);       // ✓ Correct index
}

let result = MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
assert_eq!(result, ControlFlow::Break(())); // ✓ Break after last
```

**Why this matters:** Stress tests the movement logic across longer patterns.

---

## Path Operations Tests (TraceCtx Integration)

### Core Assumptions
1. **TraceCtx combines a graph traversal interface with a TraceCache**
2. **Commands (PostfixCommand, PrefixCommand, RangeCommand) perform trace operations**
3. **PostfixCommand traces bottom-up** (from leaf to root)
4. **PrefixCommand traces top-down** (from root to leaf)
5. **add_edges flag controls whether edge relationships are stored**

### Test: `trace_ctx_postfix_traces_path_upward`

**What it tests:**
- PostfixCommand traces from a child position upward to parent patterns
- Cache is populated with the traced path

**Assumptions validated:**
```rust
// Path: 'b' is child[1] of 'ab'
let start_path = IndexStartPath::new_location(child_loc_at_b);
let command = PostfixCommand { path: start_path, ... };

command.trace(&mut ctx);

assert!(ctx.cache.exists_vertex(&b));   // ✓ Leaf vertex cached
assert!(ctx.cache.exists_vertex(&ab));  // ✓ Parent vertex cached
```

**Why this matters:** Confirms PostfixCommand correctly implements bottom-up tracing, which is essential for finding patterns that contain a given token.

---

### Test: `trace_ctx_prefix_traces_path_downward`

**What it tests:**
- PrefixCommand traces from a pattern downward to its children
- Cache accumulates the downward path

**Assumptions validated:**
```rust
// Path: 'a' is child[0] of 'ab'
let end_path = IndexEndPath::new_location(child_loc_at_a);
let command = PrefixCommand { path: end_path, ... };

command.trace(&mut ctx);

assert!(ctx.cache.exists_vertex(&ab)); // ✓ Parent cached
assert!(ctx.cache.exists_vertex(&a));  // ✓ Child cached
```

**Why this matters:** Validates top-down tracing for pattern decomposition queries.

---

### Test: `trace_ctx_range_demonstrates_basic_usage`

**What it tests:**
- IndexRangePath construction with start and end paths
- Bidirectional range specification

**Assumptions validated:**
```rust
// Range: from child[0] to child[2] in pattern [a, b, c]
let range_path = IndexRangePath {
    root,
    start: start.role_path,
    end: end.role_path,
};

assert_eq!(range_path.start.root_entry, 0); // ✓ Starts at 'a'
assert_eq!(range_path.end.root_entry, 2);   // ✓ Ends at 'c'
```

**Why this matters:** Demonstrates how to specify subranges within patterns, which is used for subsequence matching.

---

### Test: `path_append_and_trace_creates_nested_path`

**What it tests:**
- `path_append` adds a ChildLocation to a path's sub_path
- Creates nested path structures for multi-level patterns

**Assumptions validated:**
```rust
let mut path = IndexEndPath::new_location(loc_at_abcd);
path.path_append(child_loc_at_ab); // Descend into 'ab'

assert_eq!(path.sub_path.path.len(), 1);           // ✓ One level deeper
assert_eq!(path.sub_path.path[0].parent, ab);      // ✓ Correct parent
assert_eq!(path.sub_path.path[0].sub_index, 0);    // ✓ Correct position
```

**Why this matters:** Enables representing paths through nested pattern hierarchies, essential for deep pattern matching.

---

### Test: `move_root_and_leaf_combined`

**What it tests:**
- MoveRootIndex and MoveLeaf can be used together on nested paths
- Combined operations work correctly

**Assumptions validated:**
```rust
// Path: abcd[0] -> ab[0] (root at first child of abcd, leaf at first child of ab)
let mut path = IndexEndPath::new_location(loc_at_abcd_0);
path.path_append(child_loc_at_ab_0);

// Move root: abcd[0] -> abcd[1]
MoveRootIndex::<Right>::move_root_index(&mut path, &graph);
assert_eq!(path.root_entry, 1); // ✓ Root moved to 'cd'

// Move leaf: ab[0] -> ab[1]
MoveLeaf::<Right>::move_leaf(path.sub_path.path.last_mut().unwrap(), &graph);
assert_eq!(path.sub_path.path[0].sub_index, 1); // ✓ Leaf moved to 'b'
```

**Why this matters:** Validates that nested path operations are independent and composable.

---

### Test: `trace_cache_accumulates_across_multiple_commands`

**What it tests:**
- Multiple trace commands accumulate results in the same TraceCache
- Cache merges results from different trace operations

**Assumptions validated:**
```rust
// Trace 1: a -> ab
cmd1.trace(&mut ctx);
assert!(ctx.cache.exists_vertex(&a));
assert!(ctx.cache.exists_vertex(&ab));

// Trace 2: c -> cd
cmd2.trace(&mut ctx);
assert!(ctx.cache.exists_vertex(&c));   // ✓ New vertices added
assert!(ctx.cache.exists_vertex(&cd));

// Original vertices still present
assert!(ctx.cache.exists_vertex(&a));   // ✓ Previous vertices kept
assert!(ctx.cache.exists_vertex(&ab));
```

**Why this matters:** Confirms that TraceCtx supports incremental cache building across multiple operations, which is how real search algorithms work.

---

## Pattern Strings Tests

### Core Assumptions
1. **`to_pattern_strings()` converts pattern structures to human-readable string representations**
2. **Patterns with multiple decompositions return multiple string representations**
3. **String representation preserves the hierarchical structure**

### Test: `pattern_strings_single_pattern`

**What it tests:**
- Single-pattern vertices return their child sequence as strings
- Example: `ab = [a, b]` → `["a", "b"]`

**Assumptions validated:**
```rust
// Pattern: ab = [a, b]
let pats_ab = HasVertexData::vertex(ab, g).to_pattern_strings(g);
let expected = vec!["a".to_string(), "b".to_string()];
assert_eq!(pats_ab, expected); // ✓ Correct string representation
```

**Why this matters:** Enables verification and debugging of graph structure.

---

### Test: `pattern_strings_multiple_patterns`

**What it tests:**
- Vertices with multiple patterns return all alternatives
- Example: `abc = [[ab, c], [a, bc]]` → both decompositions

**Assumptions validated:**
```rust
// abc has two patterns: [ab, c] and [a, bc]
let pats_abc = vertex(abc, g).to_pattern_strings(g);
let expected = vec![
    vec!["ab".to_string(), "c".to_string()],
    vec!["a".to_string(), "bc".to_string()],
];
assert_eq!(pats_abc, expected); // ✓ All decompositions present
```

**Why this matters:** Validates that the API correctly handles vertices with multiple pattern alternatives.

---

### Test: `pattern_strings_complex_decomposition`

**What it tests:**
- Complex nested patterns are correctly represented
- Example: `abcd = [[ab, cd], [a, bcd]]` where children are themselves patterns

**Assumptions validated:**
```rust
// abcd has two patterns with compound children
let pats = vertex(abcd, g).to_pattern_strings(g);
let expected = vec![
    vec!["ab".to_string(), "cd".to_string()],
    vec!["a".to_string(), "bcd".to_string()],
];
assert_eq!(pats, expected); // ✓ Correct representation of nested patterns
```

**Why this matters:** Ensures pattern strings work for deeply nested structures.

---

## Move Key Tests

### Core Assumptions
1. **AtomPosition wraps a usize representing a position in a token sequence**
2. **MoveKey trait enables direction-aware movement** (Right = add, Left = subtract)
3. **AdvanceKey = move right**, **RetractKey = move left**
4. **Supports standard arithmetic operations** (+, -, +=, -=)

### Test: `atom_position_basic_creation`

**What it tests:**
- AtomPosition can be created from usize
- Can be converted back to usize

**Assumptions validated:**
```rust
let pos = AtomPosition::from(5);
assert_eq!(*pos, 5);                    // ✓ Dereference works
assert_eq!(Into::<usize>::into(pos), 5); // ✓ Conversion works
```

**Why this matters:** Validates the newtype wrapper works correctly.

---

### Test: `atom_position_add_operations` & `atom_position_sub_operations`

**What they test:**
- Standard arithmetic operators work correctly
- Both value and assignment variants

**Assumptions validated:**
```rust
let mut pos = AtomPosition::from(10);
let new_pos = pos + 5;
assert_eq!(*new_pos, 15); // ✓ Add works
assert_eq!(*pos, 10);     // ✓ Original unchanged

pos += 3;
assert_eq!(*pos, 13);     // ✓ AddAssign works
```

**Why this matters:** Ensures arithmetic operations follow Rust conventions.

---

### Test: `atom_position_move_key_right` & `atom_position_move_key_left`

**What they test:**
- MoveKey trait implementation for directional movement
- Right adds, Left subtracts

**Assumptions validated:**
```rust
let mut pos = AtomPosition::from(5);
<AtomPosition as MoveKey<Right>>::move_key(&mut pos, 3);
assert_eq!(*pos, 8); // ✓ Right adds 3

let mut pos = AtomPosition::from(10);
<AtomPosition as MoveKey<Left>>::move_key(&mut pos, 4);
assert_eq!(*pos, 6); // ✓ Left subtracts 4
```

**Why this matters:** Validates the direction-generic movement API used in search algorithms.

---

### Test: `atom_position_advance_key` & `atom_position_retract_key`

**What they test:**
- Convenience methods for directional movement
- AdvanceKey = move right, RetractKey = move left

**Assumptions validated:**
```rust
let mut pos = AtomPosition::from(0);
pos.advance_key(7);
assert_eq!(*pos, 7); // ✓ Advance = move right

let mut pos = AtomPosition::from(20);
pos.retract_key(5);
assert_eq!(*pos, 15); // ✓ Retract = move left
```

**Why this matters:** Provides ergonomic API for common movement patterns.

---

### Test: `atom_position_zero_moves`

**What it tests:**
- Moving by zero leaves position unchanged
- Works for all movement variants

**Assumptions validated:**
```rust
let mut pos = AtomPosition::from(10);
<AtomPosition as MoveKey<Right>>::move_key(&mut pos, 0);
assert_eq!(*pos, 10); // ✓ No change

pos.advance_key(0);
assert_eq!(*pos, 10); // ✓ No change
```

**Why this matters:** Edge case validation for movement algorithms.

---

### Test: `atom_position_chain_operations`

**What it tests:**
- Multiple operations can be chained
- Results compose correctly

**Assumptions validated:**
```rust
let mut pos = AtomPosition::from(0);
pos += 5;
pos += 3;
pos -= 2;
assert_eq!(*pos, 6); // ✓ 0+5+3-2 = 6

pos = pos + 10 - 4;
assert_eq!(*pos, 12); // ✓ 6+10-4 = 12
```

**Why this matters:** Ensures operations compose predictably.

---

## Path Append Test

### Test: `subpath_append_pushes_childlocation`

**What it tests:**
- SubPath::path_append adds a ChildLocation to the path vector
- Empty path grows correctly

**Assumptions validated:**
```rust
let mut sp = SubPath::new_empty(0);
assert_eq!(sp.path.len(), 0);        // ✓ Starts empty

sp.path_append(child);
assert_eq!(sp.path.len(), 1);        // ✓ Length increased
assert_eq!(sp.path[0], child);       // ✓ Correct child stored
```

**Why this matters:** Validates the fundamental operation for building nested paths.

---

## Key Design Patterns Validated

### 1. **Lazy Initialization**
- `force_mut()` creates entries on demand
- Distinguishes between vertex existence and position existence

### 2. **Bidirectional Traversal**
- Bottom-up and top-down positions stored separately
- Directional movement (Left/Right) with ControlFlow semantics

### 3. **Hierarchical Paths**
- Root + sub_path structure for nested patterns
- Independent movement at each level

### 4. **Incremental Cache Building**
- Multiple trace commands accumulate in same cache
- Extend trait for merging caches

### 5. **Idempotency**
- Adding same state twice doesn't duplicate
- `was_new` flag indicates whether entry was created

### 6. **Edge Management**
- `add_edges` flag controls relationship storage
- Performance optimization for existence-only queries

### 7. **Pattern Decomposition**
- Multiple representations for ambiguous patterns
- String conversion for verification

### 8. **Movement Abstraction**
- Generic MoveKey/MoveLeaf/MoveRootIndex traits
- Direction-aware operations with ControlFlow

---

## Summary

These tests validate that the `context-trace` API provides:

1. **Correct cache semantics** for hierarchical vertex/position storage
2. **Safe boundary detection** in all movement operations
3. **Proper composition** of nested path operations
4. **Idempotent and incremental** cache building
5. **Bidirectional traversal** with clear direction semantics
6. **Flexible edge management** for performance optimization
7. **Accurate pattern representation** for verification

The test suite serves as both **validation** of implementation correctness and **documentation** of expected API behavior for external crates.
