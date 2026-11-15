# Context-Read

Graph reading and expansion operations for the context framework.
Provides ordered, recursive hypergraph operations for sequenced tokenized
data with complement operations and expansion chains.

## Features
- Ordered recursive hypergraph operations
- Sequenced tokenized data handling
- Graph complement operations
- Expansion chain management with cursors and links
- Block iteration for sequence processing
- Context-aware reading operations

## Structure
- **`context/`**: Reading context management
  - `has_read_context.rs`: HasReadContext trait
  - `root.rs`: Root context operations
- **`expansion/`**: Graph expansion operations
  - `cursor.rs`: Expansion cursor management
  - `link.rs`: Link operations for expansion
  - `stack.rs`: Expansion stack management
  - `chain/`: Expansion chains (band, expand, link, op)
- **`sequence/`**: Sequence processing
  - `block_iter.rs`: Block iteration for sequences
- **`complement.rs`**: Graph complement operations

## Usage
```rust
use context_read::{HasReadContext, expansion::cursor};

// Create reading context
let read_ctx = graph.read_context();

// Process sequences with block iteration
let blocks = sequence.block_iter();

// Expand graph with cursor operations
let cursor = expansion::cursor::new(start_position);
```

## Key Concepts
- **Ordered Hypergraph**: Recursive graph structure for sequenced data
- **Expansion Chains**: Linked expansion operations with stack management
- **Block Iteration**: Efficient sequence processing in blocks
- **Complement Operations**: Graph complement calculations

### Block expansion and accumulation

`ExpansionCtx` searches a known-atom block using block-local anchors. It expands
postfixes only into the remaining atoms of that same block and resolves connected
overlaps before yielding a finalized token.

`BlockExpansionCtx` immediately sends each finalized token to
`RootManager::append_completed`. The root manager accumulates committed input,
extending an owned container when safe or constructing a new root without
mutating existing semantic tokens. It does not own or supply an overlap anchor.
Ownership is independent of known/unknown block boundaries. A newly allocated
accumulation container remains growable until it is shared or structurally
ineligible for extension; adopted tokens are never modified. Appending the root
to itself creates a new container instead of a self-reference.
Each subsequent expansion search sees the sink's graph updates, but cannot
search across a block boundary for overlaps.

## Dependencies
- **context-trace**: Core graph structures and operations
- Standard Rust collections and iterators
- Test framework integration

## Development
```bash
cargo test          # Run tests
cargo doc --open    # Generate documentation
```

## Architecture
Ordered recursive hypergraph system with expansion chains,
complement operations, and efficient sequence processing
for tokenized data structures.

![Graph Visualization](https://user-images.githubusercontent.com/20745737/133164477-8d7237d0-2f24-4b6e-9ddb-ceb7a70da43e.png)
*(rendered with [egui](https://github.com/emilk/egui) in [graph_app](https://github.com/mankinskin/graph_app))*