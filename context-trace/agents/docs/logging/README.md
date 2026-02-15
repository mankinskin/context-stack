# Logging Module

The `logging` module provides utilities for tracing, debugging, and formatted output in context-trace.

## Overview

This module includes:

- **CompactFormat**: A trait for condensed string representations
- **Formatting utilities**: Helper functions for pretty-printing
- **Tracing infrastructure**: Configuration and setup for the tracing crate

## Submodules

| Submodule | Description |
|-----------|-------------|
| `tracing_utils/` | Tracing configuration and custom formatting |

## Key Features

### CompactFormat Trait

Types implementing `CompactFormat` provide a condensed representation suitable for logging without cluttering output.

### Tracing Integration

The module integrates with the `tracing` crate for structured logging with:
- Configurablelevel filtering
- Custom formatters for graph-specific types
- Token-aware output formatting

## Files

| File | Description |
|------|-------------|
| `mod.rs` | Module root |
| `compact_format.rs` | CompactFormat trait definition |
| `format_utils.rs` | Pretty printing utilities |
| `path_format.rs` | Path-specific formatting |
