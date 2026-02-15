# Trace Module

The `trace` module provides the core tracing infrastructure for bidirectional graph traversal.

## Overview

This module contains:

- **TraceCtx**: The main context object for tracing operations
- **TraceCache**: Caching mechanism for efficient repeated traversals
- **State management**: Types for tracking trace state
- **Traceable trait**: Interface for types that can be traced

## Submodules

| Submodule | Description |
|-----------|-------------|
| `cache/` | Caching for trace operations |
| `child/` | Child iteration and child states |
| `state/` | Trace state tracking |
| `traceable/` | Traceable trait and trace commands |

## Key Concepts

### TraceCtx

The `TraceCtx` holds:
- A reference to the graph
- A cache for trace results
- Configuration for trace behavior

### Bidirectional Tracing

Traces can proceed in either direction:
- **Prefix** (Left): From a position toward the start
- **Postfix** (Right): From a position toward the end

### Caching

The trace cache stores results to avoid redundant calculations during complex traversals.
