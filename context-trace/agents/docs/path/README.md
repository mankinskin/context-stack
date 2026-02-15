# Path Module

The `path` module provides data structures and operations for representing and manipulating paths in the hypergraph.

## Overview

Paths in context-trace represent traversal routes through the hypergraph. They can be:

- **Rooted**: Starting from a specific vertex
- **Role-based**: Associated with Start or End roles
- **Range-based**: Spanning a range of tokens

## Submodules

| Submodule | Description |
|-----------|-------------|
| `accessors/` | Traits for reading path properties |
| `mutators/` | Operations for modifying paths |
| `structs/` | Path type definitions |

## Key Concepts

### Path Roles

Paths can have a **role** indicating whether they represent the start or end of a range:
- `Start`: Beginning of a range
- `End`: End of a range

### Rooted Paths

A **rooted path** is anchored at a specific vertex in the graph, providing a reference point for all path operations.

### Path Operations

- **Advance**: Move the path forward
- **Retract**: Move the path backward
- **Lower**: Go down in the hierarchy
- **Raise**: Go up in the hierarchy
- **Append**: Extend the path
- **Simplify**: Reduce path complexity
