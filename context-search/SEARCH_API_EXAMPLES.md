# Search API Usage Examples

This document demonstrates how to use the context-search API to search for patterns in a hypergraph.

## Overview

The search API provides methods to find patterns and their hierarchical relationships in a hypergraph structure. The main trait is `Find` which provides three search methods:

1. **`find_sequence`** - Search for a sequence of atoms (characters, elements, etc.)
2. **`find_parent`** - Find the largest matching direct parent of a pattern
3. **`find_ancestor`** - Find the largest matching ancestor of a pattern (can be multiple levels up)

## Basic Usage

```rust
use context_search::search::Find;
use context_trace::*;

// Create a hypergraph and insert some patterns
let mut graph = Hypergraph::<BaseGraphKind>::default();

// Insert atoms (individual characters)
let a = graph.insert_atom(Atom::Element('a'));
let b = graph.insert_atom(Atom::Element('b'));
let c = graph.insert_atom(Atom::Element('c'));

// Insert patterns (sequences of atoms)
let ab = graph.insert_pattern(vec![a, b]);
let bc = graph.insert_pattern(vec![b, c]);
let abc = graph.insert_pattern(vec![ab, c]); // Or alternatively [a, b, c]

// Convert to searchable reference
let graph_ref = HypergraphRef::from(graph);
```

## 1. Searching for Sequences

The simplest way to search is using `find_sequence`, which takes any iterable of atoms:

```rust
// Search for the sequence "abc"
match graph_ref.find_sequence("abc".chars()) {
    Ok(response) => {
        // Check if the search was complete
        if response.is_complete() {
            let path = response.unwrap_complete();
            let found_token = path.root_parent();
            println!("Found complete match: {:?}", found_token);
            // found_token would be the 'abc' pattern
        }
    }
    Err(ErrorReason::SingleIndex(_)) => {
        println!("Pattern is too simple (single atom)");
    }
    Err(e) => println!("Search error: {:?}", e),
}
```

## 2. Response Types

The `Response` struct contains:
- **`cache: TraceCache`** - Information about all vertices visited during the search
- **`end: EndState`** - The final state of the search

### Response States

The response can be in several states via `end.path`:

#### Complete (`PathEnum::Complete`)
The pattern was fully matched and found in the graph:

```rust
if let Ok(response) = graph_ref.find_ancestor(&query) {
    if response.is_complete() {
        // Extract the complete path
        let path = response.unwrap_complete();
        
        // Get the root token (the found pattern)
        let found_pattern = path.root_parent();
        
        // Access the pattern location
        let pattern_location = path.path_root().pattern_location();
        println!("Found at parent: {:?}, pattern: {:?}", 
                 pattern_location.parent, 
                 pattern_location.pattern_id);
    }
}
```

#### Incomplete - Postfix (`PathEnum::Postfix`)
Pattern was partially matched starting from the beginning:

```rust
use context_search::state::end::PathEnum;

match response.end.path {
    PathEnum::Postfix(postfix_end) => {
        println!("Found partial match from start");
        println!("Matched up to position: {:?}", postfix_end.root_pos);
        // postfix_end.path contains the matched portion
    }
    _ => {}
}
```

#### Incomplete - Prefix (`PathEnum::Prefix`)
Pattern was partially matched from the end:

```rust
match response.end.path {
    PathEnum::Prefix(prefix_end) => {
        println!("Found partial match from end");
        // prefix_end.path contains the matched portion
    }
    _ => {}
}
```

#### Incomplete - Range (`PathEnum::Range`)
Pattern was partially matched in the middle:

```rust
match response.end.path {
    PathEnum::Range(range_end) => {
        println!("Found partial match in middle");
        println!("Position: {:?}", range_end.root_pos);
        // range_end.path contains the matched portions
    }
    _ => {}
}
```

## 3. Searching with Token Patterns

Instead of searching by atoms, you can search with pre-constructed token patterns:

```rust
// Create a query from tokens
let query = vec![
    Token::new(a, 1),  // token index 'a', width 1
    Token::new(b, 1),
    Token::new(c, 1),
];

// Search for ancestor
match graph_ref.find_ancestor(&query) {
    Ok(response) => {
        if let Some(path) = response.as_complete() {
            println!("Found pattern: {:?}", path.root_parent());
        }
    }
    Err(e) => println!("Not found: {:?}", e),
}
```

## 4. Using the Trace Cache

The `TraceCache` in the response contains information about all vertices explored during the search:

```rust
let response = graph_ref.find_ancestor(&query)?;

// Access the cache
for (vertex_index, vertex_cache) in response.cache.entries.iter() {
    println!("Vertex {}: ", vertex_index);
    
    // Bottom-up edges (parent patterns containing this vertex)
    for (width, position_cache) in vertex_cache.bottom_up.iter() {
        println!("  At width {}: {} parent edges", width, position_cache.num_bu_edges());
    }
    
    // Top-down edges (child patterns this vertex contains)
    for (width, position_cache) in vertex_cache.top_down.iter() {
        println!("  At width {}: {} child edges", width, position_cache.num_td_edges());
    }
}
```

## 5. Advanced: Cursor-based Search

You can use cursors for more control over the search process:

```rust
use context_search::cursor::PatternCursor;

// Create a cursor from a pattern path
let cursor = PatternCursor { /* ... */ };

// Search using the cursor
let response = graph_ref.find_ancestor(&cursor)?;

// After a search, you can extract the cursor from the response
// and use it for consecutive searches
let next_cursor = response.end.cursor;
let next_response = graph_ref.find_ancestor(&next_cursor)?;
```

## 6. Helper Methods

The `Response` type provides several helper methods:

```rust
let response = graph_ref.find_ancestor(&query)?;

// Check if complete
if response.is_complete() {
    // Get the path (consuming the response)
    let path = response.unwrap_complete();
    
    // Or with a custom error message
    let path = response.expect_complete("Expected to find complete match");
}

// Non-consuming check
if let Some(path) = response.as_complete() {
    // Use path reference without consuming response
    println!("Found: {:?}", path.root_parent());
}
```

## 7. Practical Example: Finding Hierarchical Patterns

```rust
// Build a more complex graph
let mut graph = Hypergraph::<BaseGraphKind>::default();

// Insert atoms for "hello world"
let h = graph.insert_atom(Atom::Element('h'));
let e = graph.insert_atom(Atom::Element('e'));
let l = graph.insert_atom(Atom::Element('l'));
let o = graph.insert_atom(Atom::Element('o'));
let w = graph.insert_atom(Atom::Element('w'));
let r = graph.insert_atom(Atom::Element('r'));
let d = graph.insert_atom(Atom::Element('d'));

// Insert "hello" pattern
let hello = graph.insert_pattern(vec![h, e, l, l, o]);

// Insert "world" pattern  
let world = graph.insert_pattern(vec![w, o, r, l, d]);

// Insert "hello world" as a higher-level pattern
let hello_world = graph.insert_pattern(vec![hello, world]);

let graph_ref = HypergraphRef::from(graph);

// Now search for "hello"
match graph_ref.find_sequence("hello".chars()) {
    Ok(response) if response.is_complete() => {
        let path = response.unwrap_complete();
        println!("Found 'hello' pattern at: {:?}", path.root_parent());
        
        // The root_parent token is the 'hello' pattern node
        assert_eq!(path.root_parent().index, hello.index);
    }
    _ => panic!("Should have found 'hello'"),
}

// Search for "world"
match graph_ref.find_sequence("world".chars()) {
    Ok(response) if response.is_complete() => {
        let path = response.unwrap_complete();
        println!("Found 'world' pattern at: {:?}", path.root_parent());
        assert_eq!(path.root_parent().index, world.index);
    }
    _ => panic!("Should have found 'world'"),
}
```

## Error Handling

The search can fail with these error types:

```rust
use context_trace::graph::getters::ErrorReason;

match graph_ref.find_sequence("x".chars()) {
    Err(ErrorReason::SingleIndex(info)) => {
        println!("Pattern is a single atom, can't search for ancestors");
        println!("Index: {:?}", info.index);
    }
    Err(ErrorReason::VertexNotFound(token)) => {
        println!("Token not found in graph: {:?}", token);
    }
    Ok(response) => { /* handle response */ }
}
```

## Key Concepts

1. **Complete Match**: The entire query pattern was found as a single node in the graph
2. **Incomplete Match**: Parts of the pattern were found but not as a complete unit
3. **Trace Cache**: Records all the graph traversal information, useful for understanding how the search explored the graph
4. **Root Parent**: The top-level token that represents the found pattern
5. **Pattern Location**: Identifies where in the graph a pattern exists (parent token + pattern ID)

## Best Practices

1. Always check if the response is complete before calling `unwrap_complete()`
2. Use `as_complete()` for non-consuming checks
3. Use `expect_complete(msg)` with descriptive messages for debugging
4. Examine the trace cache to understand partial matches
5. Handle `ErrorReason::SingleIndex` for single-atom queries
