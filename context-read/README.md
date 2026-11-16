# Context-Read

Graph reading and expansion operations for the context framework.
Provides ordered, recursive hypergraph operations for sequenced tokenized
data with structural overlap closure and ownership-safe accumulation.

## Features
- Ordered recursive hypergraph operations
- Sequenced tokenized data handling
- Graph complement operations
- Root-postfix expansion with immediate commits
- Block iteration for sequence processing
- Context-aware reading operations

## Structure
- **`pipeline/`**: Reading and accumulation
  - `mod.rs`: Read context and segment orchestration
  - `root.rs`: Ownership-safe accumulation sink
- **`expansion/`**: Graph expansion operations
  - `mod.rs`: Root-first postfix search and forward steps
  - `closure.rs`: Path-based factored overlap closure
  - `block.rs`: Immediate commit orchestration
- **`input.rs`**: Input conversion
- **`segment.rs`**: Known/unknown atom block iteration

## Usage
```rust
use context_trace::HypergraphRef;

let graph = HypergraphRef::default();
let root = context_read::read(&graph, "aabbaabb").unwrap();
```

## Key Concepts
- **Ordered Hypergraph**: Recursive graph structure for sequenced data
- **Root Anchor**: Structural postfixes of the latest committed root supply overlaps
- **Block Iteration**: Efficient sequence processing in blocks
- **Complement Operations**: Graph complement calculations

### Block expansion and accumulation

See [Block expansion documentation](docs/README.md) for notation, examples,
graph changes, and algorithm responsibilities.

`ExpansionCtx` searches eligible postfix tokens of the latest committed root,
largest-first, for an expansion into the remaining known block. Candidate
searches are read-only; only the accepted match is materialized. Without an
extending postfix, expansion consumes the largest forward match.

`BlockExpansionCtx` immediately commits each span through `RootManager::commit`
and feeds the returned `CommitReceipt` back to the expansion. The root manager
accumulates committed input, extending an owned container when safe or
constructing a new root without mutating existing semantic tokens. It applies
`Append`, `ReplaceTail` or `ReplaceRoot`; it never searches or resolves overlaps.
Expansion reuses canonical combined roots where available. Ownership is
independent of known/unknown block
boundaries. A newly allocated accumulation container remains growable until it
is shared or structurally ineligible for extension; adopted tokens are never
modified. Appending the root to itself creates a new container instead of a
self-reference.

The commit seam is the entire committed section of the current block.
It bounds eligible root postfix widths; there is no seam-position scan,
separate local anchor or pending overlap chain.

After materialization, closure recomputes editability. For an editable root it
closes around the entire last child; otherwise around the root. With
`X=L+P` and `E=P+T`, the closed token is `[X,T...] = [L...,E]`.
Complements are canonical sibling patterns, not newly allocated tokens.
Whole-child overlaps can replace the tail directly. Self-overlap freezes the
root and uses whole-root closure, preventing cycles.

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
Ordered recursive hypergraph system with root-postfix expansion,
complement operations, and efficient sequence processing
for tokenized data structures.

![Graph Visualization](https://user-images.githubusercontent.com/20745737/133164477-8d7237d0-2f24-4b6e-9ddb-ceb7a70da43e.png)
*(rendered with [egui](https://github.com/emilk/egui) in [graph_app](https://github.com/mankinskin/graph_app))*