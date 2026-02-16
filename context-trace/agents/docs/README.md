# context-trace Documentation

Foundation crate for context-engine providing graph structures, path representations, and bidirectional tracing capabilities.

## Overview

This crate forms the base layer upon which context-search, context-insert, and context-read are built. It provides:

- **Graph structures** - Hypergraph data types, vertices, patterns, and tokens
- **Path representations** - Data structures for representing traversal paths
- **Bidirectional tracing** - Infrastructure for tracing in both directions through the graph
- **Caching** - Cache management for efficient repeated operations

## Modules

| Module | Description |
|--------|-------------|
| `direction/` | Direction types and pattern direction handling for bidirectional traversal |
| `graph/` | Hypergraph data structures, vertex types, and graph operations |
| `logging/` | Logging utilities including tracing configuration and formatting |
| `path/` | Path data structures and operations for graph traversal |
| `trace/` | Tracing infrastructure, cache management, and state tracking |

## Key Types

- `Hypergraph` - Core hypergraph data structure
- `HypergraphRef` - Reference-counted hypergraph handle
- `Token`, `Pattern`, `Atom` - Graph element types
- `Direction`, `Left`, `Right` - Bidirectional traversal types
- `RootedRolePath`, `RootedSplitPath` - Path representations

## See Also

- [HIGH_LEVEL_GUIDE.md](../../HIGH_LEVEL_GUIDE.md) - Concepts and architecture
- [index.yaml](index.yaml) - Full module and type listing
