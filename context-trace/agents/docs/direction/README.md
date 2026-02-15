# Direction Module

The `direction` module provides types and utilities for handling bidirectional traversal in the hypergraph.

## Overview

This module defines the core direction types that enable bidirectional graph traversal:

- **`Direction`**: An enum with `Left` and `Right` variants
- **`Left`** / **`Right`**: Type-level markers for compile-time direction handling
- **`PatternDirection`**: Direction handling specific to pattern traversal

## Key Concepts

### Bidirectional Traversal

The context-trace crate supports traversing patterns in both directions:
- **Left**: Traverses from end to start (prefix direction)
- **Right**: Traverses from start to end (postfix direction)

### Type-Level Direction

Using `Left` and `Right` as type markers enables generic programming over direction, allowing algorithms to be written once and work in both directions.

## Files

| File | Description |
|------|-------------|
| `mod.rs` | Module root, exports core direction types |
| `merge.rs` | Merge operations for direction-aware data structures |
| `pattern.rs` | Pattern-specific direction handling |
