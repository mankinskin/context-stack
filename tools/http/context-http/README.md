# context-http

HTTP + GraphQL adapter for the context-engine API.

This binary exposes the entire `context-api` command surface over HTTP. The primary interface is an RPC-style `POST /api/execute` endpoint that accepts a `Command` JSON body and returns a `CommandResult` JSON response — exactly mirroring the MCP adapter's semantics but over HTTP.

## Quick Start

```bash
# Build
cargo build -p context-http

# Run (defaults to http://127.0.0.1:3100)
cargo run -p context-http

# With custom port and host
PORT=8080 HOST=0.0.0.0 cargo run -p context-http

# With GraphQL support
cargo run -p context-http --features graphql
```

## Endpoints

### RPC (Primary)

| Method | Path             | Description                              |
|--------|------------------|------------------------------------------|
| POST   | `/api/execute`   | Execute any `Command`, returns `CommandResult` |

This single endpoint covers **100%** of the API surface. It accepts the same `Command` JSON format used by the MCP adapter.

### REST (Convenience)

| Method | Path                                  | Description              |
|--------|---------------------------------------|--------------------------|
| GET    | `/api/health`                         | Health check             |
| GET    | `/api/workspaces`                     | List all workspaces      |
| GET    | `/api/workspaces/:name/snapshot`      | Graph snapshot           |
| GET    | `/api/workspaces/:name/atoms`         | List atoms               |
| GET    | `/api/workspaces/:name/vertices`      | List vertices (tokens)   |
| GET    | `/api/workspaces/:name/statistics`    | Graph statistics         |

### GraphQL (Optional, feature-gated)

| Method | Path             | Description                     |
|--------|------------------|---------------------------------|
| POST   | `/api/graphql`   | Execute a GraphQL query         |
| GET    | `/api/graphql`   | GraphQL Playground (development)|

Only available when built with `--features graphql`.

## Usage Examples

### Create a workspace

```bash
curl -X POST http://localhost:3100/api/execute \
  -H "Content-Type: application/json" \
  -d '{"command": "create_workspace", "name": "my-graph"}'
```

Response:

```json
{
  "result": {
    "type": "workspace_info",
    "name": "my-graph",
    "atom_count": 0,
    "vertex_count": 0
  }
}
```

### Add atoms and patterns

```bash
# Add atoms
curl -X POST http://localhost:3100/api/execute \
  -H "Content-Type: application/json" \
  -d '{"command": "add_atoms", "workspace": "my-graph", "chars": ["h", "e", "l", "o"]}'

# Add a simple pattern
curl -X POST http://localhost:3100/api/execute \
  -H "Content-Type: application/json" \
  -d '{"command": "add_simple_pattern", "workspace": "my-graph", "atoms": ["h", "e"]}'
```

### Insert and search sequences

```bash
# Insert a text sequence
curl -X POST http://localhost:3100/api/execute \
  -H "Content-Type: application/json" \
  -d '{"command": "insert_sequence", "workspace": "my-graph", "text": "hello"}'

# Search for a sequence
curl -X POST http://localhost:3100/api/execute \
  -H "Content-Type: application/json" \
  -d '{"command": "search_sequence", "workspace": "my-graph", "text": "hello"}'
```

### Enable per-command tracing

Add `"trace": true` to any command to capture a detailed execution trace:

```bash
curl -X POST http://localhost:3100/api/execute \
  -H "Content-Type: application/json" \
  -d '{"command": "insert_sequence", "workspace": "my-graph", "text": "hello", "trace": true}'
```

Response includes a `trace` summary alongside the result:

```json
{
  "result": { "type": "insert_result", "..." : "..." },
  "trace": {
    "event_count": 42,
    "duration_ms": 5,
    "log_file": "insert_sequence_20260310_143022.log"
  }
}
```

### Convenience REST endpoints

```bash
# Health check
curl http://localhost:3100/api/health

# List workspaces
curl http://localhost:3100/api/workspaces

# Get snapshot for a workspace
curl http://localhost:3100/api/workspaces/my-graph/snapshot

# List atoms
curl http://localhost:3100/api/workspaces/my-graph/atoms

# List vertices
curl http://localhost:3100/api/workspaces/my-graph/vertices

# Get statistics
curl http://localhost:3100/api/workspaces/my-graph/statistics
```

### List workspaces via RPC

```bash
curl -X POST http://localhost:3100/api/execute \
  -H "Content-Type: application/json" \
  -d '{"command": "list_workspaces"}'
```

## Command JSON Format

The `Command` enum uses internally-tagged serialization with a `"command"` discriminant field and `snake_case` variant names:

```json
{ "command": "<variant_name>", ...fields }
```

All available commands:

| Command | Required Fields | Description |
|---------|----------------|-------------|
| `create_workspace` | `name` | Create a new workspace |
| `open_workspace` | `name` | Open an existing workspace |
| `close_workspace` | `name` | Close an open workspace |
| `save_workspace` | `name` | Persist workspace to disk |
| `list_workspaces` | — | List all workspaces |
| `delete_workspace` | `name` | Delete a workspace |
| `add_atom` | `workspace`, `ch` | Add a single atom |
| `add_atoms` | `workspace`, `chars` | Add multiple atoms |
| `get_atom` | `workspace`, `ch` | Get atom info |
| `list_atoms` | `workspace` | List all atoms |
| `add_simple_pattern` | `workspace`, `atoms` | Add a pattern from atoms |
| `get_vertex` | `workspace`, `index` | Get vertex info |
| `list_vertices` | `workspace` | List all vertices |
| `search_pattern` | `workspace`, `query` | Search by token refs |
| `search_sequence` | `workspace`, `text` | Search by text |
| `insert_first_match` | `workspace`, `query` | Insert first match |
| `insert_sequence` | `workspace`, `text` | Insert text sequence |
| `insert_sequences` | `workspace`, `texts` | Insert multiple texts |
| `read_pattern` | `workspace`, `index` | Read pattern decomposition |
| `read_as_text` | `workspace`, `index` | Read pattern as text |
| `get_snapshot` | `workspace` | Full graph snapshot |
| `get_statistics` | `workspace` | Graph statistics |
| `validate_graph` | `workspace` | Integrity check |
| `show_graph` | `workspace` | Debug graph display |
| `show_vertex` | `workspace`, `index` | Debug vertex display |
| `list_logs` | `workspace` | List trace log files |
| `get_log` | `workspace`, `filename` | Read a log file |
| `query_log` | `workspace`, `filename`, `query` | JQ query on log |
| `analyze_log` | `workspace`, `filename` | Log analysis |
| `search_logs` | `workspace`, `query` | Search across logs |
| `delete_log` | `workspace`, `filename` | Delete a log file |
| `delete_logs` | `workspace` | Delete log files |

## Error Responses

Errors are returned as JSON with an appropriate HTTP status code:

```json
{
  "error": "workspace 'foo' not found",
  "kind": "workspace"
}
```

### HTTP Status Code Mapping

| Error Category | Condition | HTTP Status |
|---------------|-----------|-------------|
| Workspace | Not found | 404 |
| Workspace | Already exists | 409 |
| Workspace | Not open | 400 |
| Workspace | Already open | 409 |
| Workspace | Lock conflict | 423 |
| Workspace | I/O or serialization | 500 |
| Atom | Workspace not open | 400 |
| Pattern | Atom not found | 404 |
| Pattern | Too short / duplicate | 422 |
| Pattern | Already in pattern | 409 |
| Search | Token not found | 404 |
| Search | Query too short | 422 |
| Search | Internal error | 500 |
| Insert | Token not found | 404 |
| Insert | Query too short | 422 |
| Read | Vertex not found | 404 |
| Log | File not found | 404 |
| Log | Query error | 400 |
| Internal | JSON parse / mutex | 400 / 500 |

## Configuration

All configuration is via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3100` | HTTP server port |
| `HOST` | `127.0.0.1` | Bind address |
| `CONTEXT_ENGINE_DIR` | Current directory | Workspace storage root |
| `STATIC_DIR` | — | Optional static files directory |
| `LOG_LEVEL` | `info` | Tracing log level (`trace`, `debug`, `info`, `warn`, `error`) |
| `LOG_FILE` | — | Set to enable file logging |

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                    context-http                           │
│                                                          │
│  ┌────────┐  ┌──────────┐  ┌──────────┐  ┌───────────┐  │
│  │ main.rs│  │ router.rs│  │  rpc.rs  │  │  rest.rs  │  │
│  └───┬────┘  └────┬─────┘  └────┬─────┘  └─────┬─────┘  │
│      │            │             │               │        │
│      └────────────┴──────┬──────┴───────────────┘        │
│                          │                               │
│              ┌───────────┴───────────┐                   │
│              │  state.rs (AppState)  │                   │
│              │  Arc<Mutex<Manager>>  │                   │
│              └───────────┬───────────┘                   │
│                          │                               │
│              ┌───────────┴───────────┐                   │
│              │      error.rs         │                   │
│              │  ApiError → HTTP      │                   │
│              └───────────────────────┘                   │
└──────────────────────┬───────────────────────────────────┘
                       │
         ┌─────────────┴─────────────┐
         │       context-api         │
         │  WorkspaceManager         │
         │  Command / CommandResult  │
         └───────────────────────────┘
```

### Key Design Decisions

- **RPC-first**: `POST /api/execute` is the single endpoint that handles all commands. REST endpoints are convenience wrappers.
- **Blocking ops in spawn_blocking**: `WorkspaceManager` is synchronous. All calls are wrapped in `tokio::task::spawn_blocking` to avoid blocking the async runtime.
- **Mutex for shared state**: `WorkspaceManager` requires `&mut self`, so it's wrapped in `Arc<Mutex<_>>`.
- **GraphQL is feature-gated**: The `graphql` feature adds `async-graphql` dependencies. It's read-only — all mutations go through the RPC endpoint.
- **Reuses viewer-api infra**: CORS, tracing initialization, and static file serving come from the shared `viewer-api` crate.

## Testing

```bash
# Unit + integration tests
cargo test -p context-http

# With GraphQL tests
cargo test -p context-http --features graphql
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `context-api` | Domain logic, `Command`/`CommandResult`, `WorkspaceManager` |
| `viewer-api` | Shared HTTP infra (axum, CORS, tracing, static files) |
| `axum` 0.7 | HTTP framework |
| `tokio` 1 | Async runtime |
| `serde` / `serde_json` | JSON serialization |
| `tracing` 0.1 | Structured logging |
| `tower-http` 0.5 | CORS middleware |
| `async-graphql` 7 | GraphQL schema (optional) |
| `async-graphql-axum` 7 | GraphQL axum integration (optional) |