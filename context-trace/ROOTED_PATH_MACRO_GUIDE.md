# rooted_path! Macro Guide

## Overview

The `rooted_path!` macro provides a convenient way to construct `RootedRolePath` variants (like `IndexRangePath`, `PatternRangePath`, `IndexStartPath`, `PatternEndPath`, etc.) with clear, concise syntax.

## Motivation

**Before** the macro, creating rooted paths was verbose:
```rust
let pattern_path = PatternRangePath::new(
    Pattern::from(vec![a, b, c]),
    RolePath::new_empty(0),
    RolePath::new_empty(2),
);
```

**After** the macro, it's much cleaner:
```rust
let pattern_path: PatternRangePath = rooted_path!(
    Range: Pattern::from(vec![a, b, c]),
    start: 0,
    end: 2
);
```

## Syntax

### Range Paths (start and end)

Basic range path:
```rust
rooted_path!(Range: root, start: entry, end: exit)
```

With child locations on both sides:
```rust
rooted_path!(Range: root,
    start: (entry, [child1, child2]),
    end: (exit, [child3])
)
```

With children on one side only:
```rust
rooted_path!(Range: root, start: (0, [child]), end: 2)
rooted_path!(Range: root, start: 0, end: (2, [child]))
```

### Single-Role Paths (Start or End)

Basic single-role path:
```rust
rooted_path!(Start: root, entry)
rooted_path!(End: root, entry)
```

With child locations:
```rust
rooted_path!(Start: root, (entry, [child1, child2]))
rooted_path!(End: root, (entry, [child]))
```

## Examples

### IndexRangePath

```rust
use context_trace::*;

let root = IndexRoot::from(
    ChildLocation::new(token, pattern_id, 0).into_pattern_location()
);

// Simple range
let path: IndexRangePath = rooted_path!(Range: root, start: 0, end: 2);

// With single child on one side
let child_loc = ChildLocation::new(token, pattern_id, 1);
let path: IndexRangePath = rooted_path!(Range: root,
    start: (0, [child_loc]),
    end: 2
);

// With multiple children on both sides
let child1 = ChildLocation::new(token, pattern_id, 0);
let child2 = ChildLocation::new(token, pattern_id, 1);
let child3 = ChildLocation::new(token, pattern_id, 2);
let path: IndexRangePath = rooted_path!(Range: root,
    start: (0, [child1, child2]),
    end: (2, [child2, child3])
);
```

### PatternRangePath

```rust
use context_trace::*;

let pattern = Pattern::from(vec![a, b, c]);

// Simple range
let path: PatternRangePath = rooted_path!(
    Range: pattern,
    start: 0,
    end: 2
);

// Covering entire pattern
let path: PatternRangePath = rooted_path!(
    Range: pattern,
    start: 0,
    end: (pattern.len() - 1)
);
```

### IndexStartPath / IndexEndPath

```rust
use context_trace::*;

let root = IndexRoot::from(
    ChildLocation::new(token, pattern_id, 0).into_pattern_location()
);

// Start path
let start: IndexStartPath = rooted_path!(Start: root, 0);

// End path  
let root2 = /* ... */;
let end: IndexEndPath = rooted_path!(End: root2, 2);
```

### PatternEndPath / PatternStartPath

```rust
use context_trace::*;

let pattern = Pattern::from(vec![a, b, c]);

// End path at last token
let end: PatternEndPath = rooted_path!(End: pattern, 2);

// Start path at first token
let pattern2 = Pattern::from(vec![x, y, z]);
let start: PatternStartPath = rooted_path!(Start: pattern2, 0);
```

## Type Annotations

⚠️ **Important:** Due to Rust's type inference limitations with `Into<Root>`, you usually need to provide explicit type annotations:

```rust
// ✅ CORRECT - with type annotation
let path: IndexRangePath = rooted_path!(Range: root, start: 0, end: 2);
let start: IndexStartPath = rooted_path!(Start: root, 0);

// ❌ ERROR - without annotation (ambiguous type)
let path = rooted_path!(Range: root, start: 0, end: 2);  // Which root type?
```

The compiler can't infer which implementation of `Into<Root>` to use because `IndexRoot` implements `Into<IndexRangePath>` and `Into<PatternLocation>`.

## Common Use Cases

### In State Advance Tests

```rust
#[test]
fn test_parent_state() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph, (abc, abc_id) => [a, b, c]);

    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location()
    );
    
    // Create parent path easily
    let parent_path: IndexStartPath = rooted_path!(Start: root, 0);
    
    let parent_state = ParentState {
        path: parent_path,
        prev_pos: AtomPosition::from(0),
        root_pos: AtomPosition::from(0),
    };
    
    // ... rest of test
}
```

### In Cursor Construction

```rust
use context_search::cursor::{PatternCursor, Candidate};
use std::marker::PhantomData;

let pattern = Pattern::from(vec![a, b, c]);
let pattern_path: PatternRangePath = rooted_path!(
    Range: pattern,
    start: 0,
    end: 2
);

let cursor = PatternCursor {
    path: pattern_path,
    atom_position: AtomPosition::from(0),
    _state: PhantomData::<Candidate>,
};
```

## Implementation Details

The macro is defined in `context-trace/src/tests/macros.rs` and exported with `#[macro_export]`, making it available at the crate root:

```rust
use context_trace::rooted_path;
```

It supports:
- Both range paths (with `start` and `end`) and single-role paths (`Start` or `End`)
- Optional child location vectors using tuple syntax `(entry, [child1, child2])`
- Any root type that implements `Into<Root>`
- Automatic `RolePath` construction with `new()` or `new_empty()`

## Related Types

- `RootedRangePath<Root>` - Generic range path with start and end roles
- `RootedRolePath<R, Root>` - Generic single-role path
- `IndexRangePath` = `RootedRangePath<IndexRoot>`
- `PatternRangePath` = `RootedRangePath<Pattern>`
- `IndexStartPath` = `RootedRolePath<Start, IndexRoot>`
- `IndexEndPath` = `RootedRolePath<End, IndexRoot>`
- `PatternStartPath` = `RootedRolePath<Start, Pattern>` (internal)
- `PatternEndPath` = `RootedRolePath<End, Pattern>`

## See Also

- `CHEAT_SHEET.md` - Quick reference with more examples
- `context-trace/HIGH_LEVEL_GUIDE.md` - Path concepts explained
- `context-trace/src/tests/macros.rs` - Macro implementation and tests
