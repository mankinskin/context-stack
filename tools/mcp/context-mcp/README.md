# context-mcp

MCP server for the context-engine hypergraph workspace system.

Exposes the `context-api` command surface through three MCP tools:

- **`execute`** — Run any context-engine command (the primary workhorse).
- **`help`** — Discover available commands, grouped by category, with examples and parameter descriptions. Agents should call this first.
- **`workflow`** — Get ready-to-use command sequences for common tasks (basic setup, bulk insert, search & read, etc.).

## Architecture

```text
┌─────────────┐
│  MCP Client │  (AI agent, Copilot, Claude Desktop, etc.)
└──────┬──────┘
       │ JSON-RPC over stdio
┌──────┴──────┐
│ context-mcp │  ← this binary (3 tools: execute, help, workflow)
└──────┬──────┘
       │
┌──────┴──────┐
│ context-api │  (WorkspaceManager + execute())
└─────────────┘
```

## Usage

### With Copilot CLI

```bash
copilot --additional-mcp-config @.github/context-mcp-config.json
```

### Standalone

```bash
cargo run -p context-mcp
```

The server communicates over stdio (stdin/stdout) using the MCP JSON-RPC protocol. All diagnostic output goes to stderr.

## Tools

### Tool: `help`

Discover available commands before you start. Call with no arguments for a full overview, or drill into a specific command or category.

| Parameter  | Type   | Required | Description |
|------------|--------|----------|-------------|
| `command`  | string | No       | Get detailed help for one command (e.g., `"insert_sequence"`) |
| `category` | string | No       | List commands in a category: `workspace`, `atoms`, `patterns`, `search`, `insert`, `read`, `debug` |

**Examples:**

Overview of everything:
```json
{}
```

Detailed help for a specific command:
```json
{"command": "insert_sequence"}
```

List all search commands:
```json
{"category": "search"}
```

Fuzzy lookup (suggests matches):
```json
{"command": "insert"}
```

### Tool: `workflow`

Get ready-to-use command sequences for common tasks. Each workflow is a step-by-step template with example commands you can pass directly to the `execute` tool.

| Parameter   | Type   | Required | Description |
|-------------|--------|----------|-------------|
| `name`      | string | No       | Workflow name (default: `"list"` — shows all available workflows) |
| `workspace` | string | No       | Workspace name to substitute into template commands (default: `"demo"`) |

**Available workflows:**

| Name             | Description |
|------------------|-------------|
| `basic`          | Create workspace, add atoms, insert, search, read, save — the recommended starting workflow |
| `bulk_insert`    | Insert multiple sequences efficiently using `insert_sequences` |
| `search_and_read`| Search for patterns and read results from an existing workspace |
| `inspect`        | Debug and inspect graph state: statistics, validation, vertex details |
| `persistence`    | Save, close, and reopen workflow |

**Examples:**

List all workflows:
```json
{}
```

Get the basic workflow for a custom workspace:
```json
{"name": "basic", "workspace": "my-graph"}
```

### Tool: `execute`

The primary workhorse — run any context-engine command. Send a `Command` JSON object and receive the corresponding `CommandResult`.

**Tip:** Call `help` first to discover commands, or `workflow` for step-by-step templates.

#### Recommended First-Time Flow

1. Call `help` (no arguments) — get an overview of all commands
2. Call `workflow` with `name: "basic"` — get a step-by-step template
3. Follow the steps using `execute`

#### Quick Start (Direct)

1. **Create:** `{"command": "create_workspace", "name": "demo"}`
2. **Add atoms:** `{"command": "add_atoms", "workspace": "demo", "chars": ["a","b","c","d","e"]}`
3. **Insert:** `{"command": "insert_sequence", "workspace": "demo", "text": "abcde"}`
4. **Search:** `{"command": "search_sequence", "workspace": "demo", "text": "abc"}`
5. **Read:** `{"command": "read_as_text", "workspace": "demo", "index": 5}`
6. **Save:** `{"command": "save_workspace", "name": "demo"}`

#### Available Commands

| Category    | Commands |
|-------------|----------|
| **Workspace** | `create_workspace`, `open_workspace`, `close_workspace`, `save_workspace`, `list_workspaces`, `delete_workspace` |
| **Atoms**     | `add_atom`, `add_atoms`, `get_atom`, `list_atoms` |
| **Patterns**  | `add_simple_pattern`, `get_vertex`, `list_vertices` |
| **Search**    | `search_pattern`, `search_sequence` |
| **Insert**    | `insert_first_match`, `insert_sequence`, `insert_sequences` |
| **Read**      | `read_pattern`, `read_as_text` |
| **Debug**     | `get_snapshot`, `get_statistics`, `validate_graph`, `show_graph`, `show_vertex` |

#### Response Format

Successful commands return:
```json
{
  "success": true,
  "result": { ... }
}
```

Failed commands return:
```json
{
  "success": false,
  "error": "error description"
}
```

API-level errors are returned as successful MCP responses (not MCP-level errors) so the agent can read and react to error messages.

## Environment Variables

| Variable   | Default | Description |
|------------|---------|-------------|
| `RUST_LOG`  | —       | Tracing filter (e.g., `context_mcp=debug`) |

## Design Rationale

### Why three tools instead of one?

The original design used a single `execute` tool, pushing the entire `Command` enum (25+ variants) through one tool's JSON schema. This works, but creates friction for agents:

- **Schema bloat** — Agents receive a massive flat schema with every variant's fields at once.
- **No discoverability** — New agents don't know where to start or what's available.
- **No workflow guidance** — The correct ordering (create → atoms → insert → search → read → save) isn't obvious.

The `help` and `workflow` tools solve these problems:
- `help` provides structured, queryable documentation the agent can explore incrementally.
- `workflow` provides copy-paste-ready command sequences so agents can get productive immediately.
- Both are read-only and zero-risk — they don't touch the workspace manager.

This follows the same pattern as `tools/doc-viewer`, which exposes 6 CRUD tools (`list`, `search`, `validate`, `create`, `update`, `delete`) instead of a single monolithic tool.

## Testing

```bash
cargo test -p context-mcp
```

Runs 22 tests covering:
- Execute tool: workspace lifecycle, atoms, insert/search workflows, persistence, serialization
- Help tool: overview, specific command lookup, fuzzy matching, category filtering, coverage of all commands
- Workflow tool: listing, template generation, workspace name substitution, unknown workflow handling, step validation