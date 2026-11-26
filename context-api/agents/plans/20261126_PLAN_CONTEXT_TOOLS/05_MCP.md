# Stage 5 — MCP server

Part of [20261126_PLAN_CONTEXT_TOOLS](README.md). Depends on
[stage 1](01_CORE_API.md); proxy mode on [stage 4](04_CLIENT_SHELL.md)
`RemoteBackend`.

## 5.1 What changes in `tools/mcp/context-mcp`

| Today | Fate |
|---|---|
| rmcp 0.14 stdio server (`src/server.rs`), `ContextServer { Arc<Mutex<WorkspaceManager>> }` | keep rmcp and stdio; state becomes `Box<dyn Backend + Send>` (local `Session` or remote) |
| tool `execute` (any `Command` JSON) | keep as an escape hatch (`context_execute`), schema = `Request` |
| tools `help`, `workflow` with hand-written help text (`server/help/*`, `workflow.rs`, ≈1 100 lines) | **replace** by typed tools whose JSON schemas (schemars) document themselves; keep a short `context_help` returning usage guidance |
| `main.rs` bin | thin wrapper over `context_mcp::run(config)`; `context-cli mcp` calls the same function |

## 5.2 Tools

Typed tools are easier for agents than one generic `execute` (the model sees
parameter schemas directly):

| Tool | Parameters | Result |
|---|---|---|
| `context_list_graphs` | — | `List` |
| `context_graph_info` | `graph` | `Info` + `Stats` summary |
| `context_create_graph` | `graph`, `texts?: [string]`, `overwrite?` | `New` or `Create` |
| `context_read` | `graph`, `texts: [string]` or `files: [path]` (local mode only, below `--allow-fs`), `split?`, `create?` | `ReadReport` (no steps) |
| `context_search` | `graph`, `query`, `view?` (`summary` default), `limit?` (default 20), `width?` (default 40) | `Search` |
| `context_vertex` | `graph`, `vertex` (index or text) | `Vertex` |
| `context_render` | `graph`, `format?` (`mermaid` default), `root?`, `depth?` | `Render`; hard cap 200 vertices, no `force` |
| `context_execute` | `Request` | `Response` |

All tools return the `Response` JSON as text content; additionally as
structured content with an output schema if the pinned rmcp version
supports it (verify when implementing; upgrade rmcp only if needed).

Output budget: results larger than `--max-output` (default 32 KiB) are
truncated at item granularity with `truncated: true` and a hint
(narrower `limit`, `--root`), never cut mid-JSON.

No delete tool by default (destructive); enabled with `--allow-delete`.

## 5.3 Modes

```text
context-cli mcp [--store DIR] [--allow-fs DIR] [--max-output 32KiB]   # local: own Session over stdio
context-cli mcp --server http://127.0.0.1:7878 [--token ...]          # proxy: forward to a running server
```

Proxy mode lets an agent and a human shell share one served graph (and the
agent's reads appear as `graph_changed` events in the shell and the viewer).
Local mode auto-discovers a server like the CLI (stage 4 §4.3) when the
graph is held by one.

Optional later: MCP streamable HTTP transport mounted in the stage-3 server
(`/v1/mcp`, rmcp `transport-streamable-http-server`), so remote agents need
no local process. Not part of this stage's gate.

stdout stays reserved for MCP JSON-RPC; logs to stderr (unchanged).

Example client configuration (VS Code `mcp.json` / Claude Desktop style):

```json
{
  "servers": {
    "context": {
      "command": "context-cli",
      "args": ["mcp", "--store", "${workspaceFolder}/.context-engine", "--allow-fs", "${workspaceFolder}"]
    }
  }
}
```

## 5.4 Tests

Port the existing in-process tests (`server/tests/{execute,workflow,help}.rs`)
to an rmcp client connected through an in-memory duplex transport:

| Test | Asserts |
|---|---|
| `list_tools_schemas` | every tool has an input schema; schemas validate the example arguments |
| `create_read_search` | create → read texts → search occurrences equals the CLI result for the same corpus |
| `render_cap` | `context_render` on a large graph returns the gate error with hint |
| `files_require_allow_fs` | `files` refused without `--allow-fs`, accepted below DIR |
| `output_budget` | a search with many occurrences is truncated with `truncated: true` and valid JSON |
| `proxy_mode` | MCP in proxy mode against an in-test server: same results as local; a read emits `graph_changed` on a WebSocket subscriber |
| `execute_escape_hatch` | `context_execute` with a raw `Request` returns the same JSON as `/v1/execute` |

```bash
cargo test -p context-mcp
# manual, with the MCP inspector
npx @modelcontextprotocol/inspector context-cli mcp --store /tmp/ctx
```
