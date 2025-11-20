# Context-Search

Advanced pattern search and traversal operations for hierarchical graph structures. Provides sophisticated algorithms for finding patterns, sequences, and relationships within hypergraph data structures.

## Features

### Pattern Finding
- **Multi-input support**: Search with token arrays, patterns, or path cursors
- **Hierarchical matching**: Find patterns within nested graph structures  
- **Ancestor search**: Locate containing patterns that match queries
- **Consecutive search**: Match sequential patterns within larger structures
- **Flexible matching**: Support for partial matches and pattern completion

### Search Strategies
- **Breadth-First Traversal (BFT)**: Explore all possibilities at each level
- **Depth-First Traversal (DFT)**: Follow paths to completion before alternatives
- **Policy-driven**: Configurable search behavior and termination criteria
- **State continuation**: Resume interrupted searches from previous state

### Advanced Navigation
- **Cursor-based traversal**: Precise position tracking within patterns
- **Role-based paths**: Different navigation modes for various search types
- **Bidirectional search**: Bottom-up and top-down pattern exploration
- **Cache-aware**: Leverages trace caching for performance optimization

## Architecture

### Core Components

**Search Engine**
- `search/`: Main search algorithms and context management
- `fold/`: Pattern folding operations that convert inputs to searchable contexts
- `traversal/`: Configurable traversal policies and strategies

**Navigation System** 
- `cursor/`: Position tracking and path-based navigation
- `match/`: Pattern matching algorithms and root cursor management
- `state/`: Search state management (start, end, intermediate states)

**Container Operations**
- `container/`: Collection handling for search results
- `compare/`: Comparison operations for search validation

### How Pattern Finding Works

#### 1. **Input Processing**
```rust
// Various input types can be searched
let tokens: &[Token] = &[a, b, c];
let pattern: &Pattern = &some_pattern;
let cursor: PatternCursor = PatternCursor::new(path, position);

// All implement Foldable trait
let result = tokens.fold::<BreadthFirst>(traversal_context)?;
```

#### 2. **Folding to Search Context**
The `Foldable` trait converts different input types into a unified `SearchState`:
- **Token arrays**: Create cursor from token sequence
- **Patterns**: Use existing pattern structure  
- **Cursors**: Use current position and path
- **Paths**: Convert path definitions to navigable cursors

#### 3. **Hierarchical Traversal**
```rust
// Search strategies
pub trait TraversalKind {
    type Trav: HasGraph;
    // BreadthFirst: explores level-by-level
    // DepthFirst: follows each branch completely
}

// Policy-driven search behavior
pub trait TraversalPolicy {
    fn should_continue(&self, state: &SearchState) -> bool;
    fn handle_mismatch(&self, reason: &ErrorReason) -> Decision;
}
```

#### 4. **State Management**
- **StartCtx**: Initial search configuration with graph reference and policies
- **SearchState**: Active search context with cursor and traversal state  
- **EndState**: Terminal results with success/failure details and exact position
- **InnerKind**: Intermediate states allowing search continuation

#### 5. **Result Types**
```rust
pub enum FinishedKind {
    Complete(Box<CompleteEnd>),     // Successful full match
    Incomplete(Box<EndState>),      // Partial match with details
    // Contains exact failure position and reason
}
```

## Usage Examples

### Basic Pattern Search
```rust
use context_search::*;
use context_trace::*;

// Create search context
let graph_ref = HypergraphRef::new(graph);
let traversal = BreadthFirst::new(graph_ref);

// Search for token sequence
let query = &[token_a, token_b, token_c];
let result = query.fold(traversal)?;

match result.kind {
    FinishedKind::Complete(end) => {
        println!("Found complete match at: {:?}", end.cursor);
    },
    FinishedKind::Incomplete(end) => {
        println!("Partial match, stopped at: {:?}", end.reason);
    }
}
```

### Ancestor Pattern Search
```rust
// Find containing patterns
let ancestors = graph.find_ancestor(&query_pattern)?;
for ancestor in ancestors {
    println!("Found in pattern: {:?}", ancestor.pattern);
}
```

### Advanced Search with Cursors
```rust
// Create cursor for complex navigation
let cursor = PatternCursor::new(
    RootedRolePath::new(pattern, role_path),
    atom_position
);

// Search from specific position
let result = cursor.fold::<DepthFirst>(traversal)?;
```

**Detailed Algorithm Example**: See `PATTERN_MATCHING_EXAMPLE.md` for a complete step-by-step walkthrough of finding the pattern "Hello" in a hierarchical hypergraph, including:
- Input processing and folding operations
- Position-by-position matching with cache updates
- Pattern recognition and hierarchical discovery
- Result construction and state preservation
- Comparison of different search strategies

## Key Concepts

### **Foldable Pattern Matching**
The core abstraction where different input types (tokens, patterns, cursors) can be "folded" into search results through a unified interface.

### **Hierarchical Navigation** 
Patterns can contain other patterns, creating nested structures. The search engine navigates these hierarchies using role-based paths and cursor positioning.

### **State Continuation**
Search operations can be paused and resumed, allowing for incremental processing of large pattern spaces.

### **Cache Integration**
Leverages the trace cache from `context-trace` to avoid redundant computations and speed up repeated searches.

## Performance Features

- **Lazy evaluation**: Only computes necessary search paths
- **Cache integration**: Reuses trace results from previous operations  
- **Policy customization**: Fine-tune search behavior for specific use cases
- **Memory efficient**: Streaming results without building full search trees

## Architectural Considerations

### Strengths
- **Unified Interface**: Foldable trait provides consistent API across input types
- **Flexible Traversal**: Policy-driven search strategies for different use cases
- **State Management**: Comprehensive state tracking and continuation support
- **Cache Integration**: Leverages trace cache for performance optimization
- **Thread Safety**: Arc/RwLock wrappers enable safe concurrent access

### Potential Weak Points
- **Memory Growth**: Cache can grow unbounded without eviction policies
- **Deep Recursion**: May cause stack overflow with very deep pattern hierarchies
- **Lock Contention**: RwLock usage may create bottlenecks under high concurrency
- **Error Context**: Some failures may lose specific diagnostic information
- **Type Complexity**: Generic bounds can allow invalid type combinations

See `TESTING_PLAN.md` for comprehensive analysis and mitigation strategies.

## Dependencies
- **context-trace**: Core graph structures and tracing capabilities
- **derive-new**: Constructor generation for search contexts
- **pretty_assertions**: Enhanced test output formatting

## Testing
```bash
cargo test                    # Run all tests
cargo test search::ancestor   # Test ancestor search specifically  
cargo test traversal         # Test traversal policies
```

The crate includes comprehensive tests for:
- Pattern finding in nested structures
- Search policy behavior
- State continuation and resumption
- Performance with large pattern hierarchies

**Comprehensive Testing Strategy**: See `TESTING_PLAN.md` for detailed analysis of:
- Architectural weak points and mitigation strategies
- Phase-based testing approach covering core functionality through stress testing
- Performance benchmarks and scalability validation
- Concurrency testing and thread safety verification
- Integration testing with other context framework components

## Integration

Works seamlessly with other context framework components:
- **context-trace**: Provides the underlying graph and tracing infrastructure
- **context-insert**: Uses search results to guide insertion operations
- **context-read**: Leverages pattern finding for high-level reading operations

The search engine forms the intelligent core that enables sophisticated pattern-based operations across the entire context framework.
