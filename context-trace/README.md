# Context-Trace

Foundational component of the context framework providing core graph 
structures, path operations, and tracing capabilities. Implements 
hypergraph data structures with sophisticated path navigation and 
bidirectional tracing.

## Features
- Hypergraph structure with vertex and token management
- Thread-safe graph references (Arc/RwLock)
- Role-based path operations with accessors/mutators
- Bidirectional tracing (bottom-up and top-down)
- Comprehensive cache management
- State continuation support

## Structure
- **`direction/`**: Left/Right directions, pattern matching
  - `match.rs`: Direction matching logic
  - `merge.rs`: Direction merging operations
  - `pattern.rs`: Directional pattern operations
- **`graph/`**: Hypergraph, vertices, getters, insertion
  - `kind.rs`: Graph type system (BaseGraphKind, GraphKind)
  - `insert.rs`: Graph insertion operations
  - `validation.rs`: Graph validation logic
  - `getters/`: Data access (advanced, child, parent, pattern, token, vertex)
  - `vertex/`: Vertex system (child, parent, token, pattern, location, wide)
- **`path/`**: Path accessors, mutators, role-based paths
  - `accessors/`: Path access (border, complete, role, root, child navigation)
  - `mutators/`: Path modification (append, lower, pop, raise, simplify, move)
  - `structs/`: Path data structures (query, role, rooted, sub paths)
- **`trace/`**: TraceCtx, caching, commands, state management
  - `has_graph.rs`: Graph access trait (HasGraph, TravKind)
  - `traceable.rs`: Traceability operations
  - `command.rs`: Trace commands (prefix, postfix, range)
  - `cache/`: Trace caching (position, vertex, key management)
  - `child/`: Child tracing (iterators, states, bands, positions)
  - `state/`: Tracing state management (base state, parent states)

## Usage
```rust
use context_trace::{Hypergraph, HypergraphRef, RolePath};

// Create hypergraph
let graph = Hypergraph::new();

// Thread-safe wrapper
let graph_ref = graph.to_ref();

// Access vertices and create paths
let vertex = graph.expect_vertex(index);
let path = RolePath::new(child_locations);
```

## Key Concepts
- **Hypergraph**: Indexed graph with vertex and token management
- **HypergraphRef**: Thread-safe Arc/RwLock wrapper
- **Role-based Paths**: Different path types for traversal roles
- **Bidirectional Tracing**: State tracking in both directions

## Dependencies
- **petgraph**: Graph algorithms and data structures
- **indexmap**: Ordered maps for consistent iteration
- **uuid**: Unique identifiers for vertices and patterns
- **tracing**: Logging and debugging support
- **serde**: Serialization and deserialization

## Development
```bash
cargo test          # Run tests
cargo doc --open    # Generate documentation
```

### Testing with Logging

By default, test logs are written to files in `target/test-logs/` and automatically
deleted when tests pass. To enable stdout logging for debugging:

```bash
# Enable stdout logging for all tests
RUST_TEST_LOG_STDOUT=1 cargo test

# Run specific test with stdout logging
RUST_TEST_LOG_STDOUT=1 cargo test my_test_name -- --nocapture

# Combine with RUST_LOG for custom log levels
RUST_TEST_LOG_STDOUT=1 RUST_LOG=debug cargo test
```

Failed tests always preserve their log files in `target/test-logs/` for inspection.

**Features**: `test-api` (deterministic testing), default (logging)

## Architecture
Layered design with graph data structures, path navigation system, 
and comprehensive tracing capabilities for state management and 
search operations.