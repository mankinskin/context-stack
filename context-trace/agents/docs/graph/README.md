# Graph Module

The `graph` module provides the core hypergraph data structure and related types for the context-engine.

## Overview

This module contains:

- **`Hypergraph`**: The primary data structure storing vertices, patterns, and their relationships
- **`HypergraphRef`**: A reference-counted handle to a hypergraph
- **Vertex types**: Various vertex representations and associated data
- **Graph operations**: Query and modification operations

## Submodules

| Submodule | Description |
|-----------|-------------|
| `getters/` | Query operations for retrieving graph data |
| `insert/` | Insertion operations for modifying the graph |
| `vertex/` | Vertex types, indices, and vertex data structures |

## Key Concepts

### Hypergraph Structure

The hypergraph stores:
- Vertices with associated data
- Patterns (sequences of tokens)
- Parent-child relationships
- Width information for efficient traversal

### Graph Kinds

The `GraphKind` and `BaseGraphKind` traits define the type parameters for graphs, allowing different atom types and configurations.

## Files

| File | Description |
|------|-------------|
| `mod.rs` | Module root |
| `child_strings.rs` | String conversion for children |
| `kind.rs` | Graph kind traits |
| `test_graph.rs` | Test utilities |
| `validation.rs` | Graph validation |
