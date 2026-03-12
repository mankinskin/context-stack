# context-api

Unified API for hypergraph workspace management and operations.

This crate provides the single public interface for all hypergraph operations across the context-engine workspace. It wraps `context-trace`, `context-search`, `context-insert`, and (in Phase 2) `context-read` behind a workspace-oriented, command-based API with feature-gated adapters for CLI, MCP, HTTP, and future protocols.

## Architecture

```text
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ context-cli  │  │ context-mcp  │  │ context-http │
│ (bin)        │  │ (bin)        │  │ (bin)        │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       └─────────────────┴────────┬────────┘
                                  │
                           ┌──────┴───────┐
                           │  context-api │  ← you are here
                           └──────┬───────┘
                                  │
        ┌─────────────────────────┼─────────────────────────┐
        │                         │                         │
 context-insert ──► context-search ──► context-trace
```

## Usage

### Rust consumers — use the `WorkspaceApi` trait

```rust
use context_api::prelude::*;
use context_api::workspace::manager::WorkspaceManager;

let mut mgr = WorkspaceManager::current_dir().unwrap();

// Create a workspace
let info = mgr.create_workspace("demo").unwrap();

// Add atoms (single characters)
mgr.add_atom("demo", 'a').unwrap();
mgr.add_atom("demo", 'b').unwrap();
mgr.add_atom("demo", 'c').unwrap();

// Create a simple pattern from existing atoms
let pattern = mgr.add_simple_pattern("demo", vec!['a', 'b']).unwrap();
println!("Created pattern: {} (index {})", pattern.label, pattern.index);

// Inspect the graph
let stats = mgr.get_statistics("demo").unwrap();
println!("Vertices: {}, Atoms: {}, Patterns: {}",
    stats.vertex_count, stats.atom_count, stats.pattern_count);

// Save and close
mgr.save_workspace("demo").unwrap();
mgr.close_workspace("demo").unwrap();
```

### Adapter layers — use the `Command` / `CommandResult` enums

```rust
use context_api::commands::{Command, execute};
use context_api::workspace::manager::WorkspaceManager;

let mut mgr = WorkspaceManager::current_dir().unwrap();

// Deserialize a command from JSON
let cmd: Command = serde_json::from_str(
    r#"{"command":"create_workspace","name":"demo"}"#
).unwrap();

// Execute and serialize the result
let result = execute(&mut mgr, cmd).unwrap();
let json = serde_json::to_string(&result).unwrap();
println!("{json}");
```

## Feature Flags

| Feature   | Description                                          | Default |
|-----------|------------------------------------------------------|---------|
| `ts-gen`  | Enable TypeScript type generation via `ts-rs`        | off     |
| `dev`     | Extra debug commands and verbose internal logging    | off     |

## Workspace Storage

Workspaces are stored under a `.context-engine/` directory relative to the base directory (typically the project root):

```text
.context-engine/<workspace-name>/
├── graph.bin       # bincode-serialized Hypergraph
├── metadata.json   # human-readable workspace metadata (timestamps, description)
└── .lock           # advisory file lock (fs2)
```

- **Persistence**: Explicit save semantics — changes are only written to disk when `save_workspace` is called.
- **Concurrency**: Multi-reader, single-writer model using advisory file locks (`fs2`). Only one process should have a workspace open for writing at a time.
- **Atomic writes**: Graph and metadata files are written atomically (write to `.tmp`, then rename) to prevent corruption on crash.

## API Types

All public types are in the `types` module and are serializable via `serde`:

| Type              | Description                                          |
|-------------------|------------------------------------------------------|
| `AtomInfo`        | A single atom (character) vertex                     |
| `TokenInfo`       | Lightweight info about any vertex (index, label, width) |
| `PatternInfo`     | A newly created pattern with its children            |
| `VertexInfo`      | Detailed vertex information (children, parents, etc.) |
| `WorkspaceInfo`   | Workspace summary (name, counts, timestamps)         |
| `GraphStatistics` | Aggregate graph metrics                              |
| `TokenRef`        | Reference a token by index or label string           |
| `GraphSnapshot`   | Full graph topology snapshot (re-exported from context-trace) |

## Error Model

Errors are organized by domain and composed into a top-level `ApiError`:

```text
ApiError
├── Workspace(WorkspaceError)   — lifecycle errors (not found, already exists, lock conflict, I/O)
├── Atom(AtomError)             — atom operation errors
├── Pattern(PatternError)       — pattern validation and creation errors
├── Search(SearchError)         — search errors (Phase 2)
├── Insert(InsertError)         — insert errors (Phase 2)
└── Read(ReadError)             — read errors (Phase 2)
```

## CLI

The companion `context-cli` binary (in `tools/context-cli/`) provides both subcommand and REPL interfaces:

```bash
# Subcommand interface
context-cli create demo
context-cli add-atom demo a
context-cli add-atom demo b
context-cli add-pattern demo ab
context-cli stats demo
context-cli save demo

# Interactive REPL
context-cli repl
# or just:
context-cli
```

## Phase Plan

This crate is being built incrementally:

| Phase | Scope                        | Status |
|-------|------------------------------|--------|
| 1     | Foundation + CLI             | 🚧     |
| 2     | Algorithm Commands           | 📋     |
| 3     | MCP Adapter                  | 📋     |
| 4     | HTTP + GraphQL Adapter       | 📋     |
| 5     | TypeScript Types + Advanced  | 📋     |

See `agents/plans/20260310_PLAN_CONTEXT_API_OVERVIEW.md` for the full plan.