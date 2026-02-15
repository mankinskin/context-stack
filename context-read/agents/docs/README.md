# context-read

Context reading and expansion operations for the context-engine.

## Overview

This crate provides functionality for:

- **Reading context**: Extract context around positions in the hypergraph
- **Expanding results**: Grow context based on configurable policies
- **Band-based representation**: Organize context into hierarchical bands

## Modules

| Module | Description |
|--------|-------------|
| `bands/` | Band-based context representation |
| `complement.rs` | Complement operations |
| `context/` | Read context management |
| `expansion/` | Context expansion |
| `request.rs` | Request handling |
| `segment.rs` | Segment representation |

## Key Concepts

### Band-Based Context

Context is organized into **bands** - hierarchical layers representing different levels of context around a position.

### Expansion

Context **expansion** grows the initial context outward based on policies, allowing flexible control over how much surrounding context to include.

### Read Context

The `ReadContext` type manages all state needed for reading operations, including:
- Graph reference
- Position tracking
- Expansion state
