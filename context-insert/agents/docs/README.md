# context-insert

Insertion operations for the context-engine using split-join architecture.

## Overview

This crate handles graph modifications through a split-join approach:

1. **Split**: Decompose existing patterns at the insertion point
2. **Insert**: Add new content
3. **Join**: Merge results back into the graph

## Modules

| Module | Description |
|--------|-------------|
| `insert/` | High-level insertion API |
| `interval/` | Interval-based graph representation |
| `join/` | Join operations |
| `split/` | Split operations |

## Key Concepts

### Split-Join Architecture

The split-join approach ensures consistent graph state during modifications:

1. **Splitting** breaks patterns at precise positions
2. **Joining** reconstructs valid patterns after insertion

### InitInterval

`InitInterval` provides the initial interval configuration for setting up graph intervals before operations.

### InsertCtx

The `InsertCtx` type holds all necessary context for performing insertions, including:
- Graph reference
- Position information
- Caching state
