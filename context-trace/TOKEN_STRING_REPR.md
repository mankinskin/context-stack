# Token String Representations in Tests

This feature allows tokens to display their string representation (e.g., "abc") in test builds, making it much easier to identify tokens when reading logs and debugging.

## How It Works

In test builds, tokens can access a globally registered hypergraph to compute their string representation dynamically. This approach:

- **No overhead in production**: The feature is only enabled in `#[cfg(test)]` builds
- **Zero storage cost**: Tokens remain `Copy` and lightweight (just index + width)
- **Flexible**: Works with any `GraphKind` that has `Display` atoms
- **Safe**: Uses interior mutability with `RwLock` for thread-safety

## Usage

### Basic Usage

```rust
use context_trace::{Hypergraph, graph::test_graph};

#[test]
fn my_test() {
    let mut graph = Hypergraph::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph, abc => [a, b, c]);
    
    // Register the graph for string representations
    register_test_graph!(graph);
    
    // Now tokens show their content in logs
    println!("{}", a);    // Prints: T0w1("a")
    println!("{}", abc);  // Prints: T3w3("abc")
    
    // Clean up when test is done (optional, happens automatically at end)
    graph::test_graph::clear_test_graph();
}
```

### With Existing Test Macros

The `register_test_graph!` macro works seamlessly with existing test utilities:

```rust
#[test]
fn test_with_string_repr() {
    let mut graph = HypergraphRef::default();
    
    // Use existing test macros
    insert_atoms!(graph, {h, e, l, o});
    insert_patterns!(graph,
        hello => [h, e, l, l, o]
    );
    
    // Enable string representations
    register_test_graph!(graph);
    
    // All logging and assertions now show readable token content
    let result = some_operation(&graph, hello);
    // Logs will show: "Processing T4w5(\"hello\")" instead of "Processing T4w5"
}
```

### Manual Registration

You can also register graphs manually without the macro:

```rust
use context_trace::graph::test_graph;

let graph = Hypergraph::default();
// ... populate graph ...

test_graph::register_test_graph(&graph);
```

## API Reference

### Functions

- **`register_test_graph(graph: &Hypergraph<G>)`**: Register a graph for token lookups
- **`get_token_string_from_test_graph(index: usize) -> Option<String>`**: Get string for a token index
- **`clear_test_graph()`**: Clear the registered graph

### Token Methods (Test Only)

- **`token.get_string_repr() -> Option<String>`**: Get the string representation if a graph is registered

### Display Format

When a graph is registered, tokens display as:
```
T{index}w{width}("{string}")
```

For example:
- `T0w1("a")` - An atom 'a'
- `T5w3("cat")` - A pattern "cat" 
- `T10w5("hello")` - A pattern "hello"

Without a registered graph, tokens display as:
```
T{index}w{width}
```

## Implementation Details

### Architecture

1. **Global Registry**: A thread-safe `RwLock<Option<Box<dyn GraphStringGetter>>>` stores the graph
2. **Type Erasure**: The `GraphStringGetter` trait allows any `Hypergraph<G>` to be stored
3. **Dynamic Lookup**: `Token::fmt()` checks for a registered graph and computes strings on-demand
4. **Cloning**: The graph is cloned when registered (acceptable overhead for tests)

### Thread Safety

The implementation uses `RwLock` for thread-safe access, allowing multiple tests to read the graph concurrently. Only registration and clearing require exclusive write access.

### Performance

- **Token size**: Unchanged (16 bytes: 8 for index, 8 for width)
- **Token operations**: No overhead (Copy, Hash, Eq all unchanged)
- **Display overhead**: Only pays cost when formatting tokens for output
- **Memory**: One cloned graph stored globally per test registration

## Best Practices

### When to Use

✅ **Use string representations when:**
- Debugging complex graph operations
- Writing tests with many tokens
- Generating detailed trace logs
- Debugging pattern matching issues

❌ **Don't use when:**
- Performance is critical (though overhead is minimal)
- Tests are purely focused on graph structure
- The added output verbosity is distracting

### Cleaning Up

The registered graph persists across tests in the same process. For isolation:

```rust
#[test]
fn my_test() {
    let graph = Hypergraph::default();
    // ... test code ...
    
    register_test_graph!(graph);
    // ... more test code ...
    
    // Explicit cleanup ensures next test starts fresh
    graph::test_graph::clear_test_graph();
}
```

### Debugging Tips

If tokens aren't showing string representations:

1. Check that `register_test_graph!` is called **after** inserting atoms/patterns
2. Verify you're in a test build (`#[cfg(test)]`)
3. Re-register if you modify the graph after registration
4. Check that the graph has `Display` atoms (e.g., `char`)

## Examples

See `context-trace/src/tests/test_string_repr.rs` for comprehensive examples.
