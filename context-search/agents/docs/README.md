# context-search

Pattern matching and search functionality for the context-engine.

## Overview

This crate provides:

- **Unified Response API**: Consistent interface for all search results
- **Search algorithms**: BFT and DFT traversal strategies
- **Pattern matching**: Find patterns within the hypergraph
- **State management**: Track search progress and results

## Modules

| Module | Description |
|--------|-------------|
| `compare/` | Comparison logic for search states |
| `container/` | Search queue containers (BFT, DFT) |
| `cursor/` | Position tracking during search |
| `match/` | Match handling |
| `policy.rs` | Search configuration |
| `search/` | Core algorithms |
| `state/` | State management and Response API |
| `logging/` | Search-specific logging |

## Key Concepts

### Response API

The `Response` type provides a unified interface for search results, encapsulating:
- Match status
- Position information
- Continuation state

### Search Strategies

- **BFT (Breadth-First)**: Level-by-level exploration
- **DFT (Depth-First)**: Deep exploration before backtracking

### Searchable Trait

Types implementing `Searchable` can be searched using the `Find` trait, which provides the main entry point.
