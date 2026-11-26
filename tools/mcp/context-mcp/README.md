# context-mcp

MCP server for context-engine graphs. Agents index text into named graphs
and answer exact-substring questions about it (occurrences, contexts,
parents), through typed tools over the `context-api` `Session`.

- Library: `context_mcp::{McpConfig, ContextServer, run_stdio}`; the
  `context-mcp` binary is a thin stdio wrapper.
- Every tool returns the `context_api::Response` JSON as compact text:
  `{"ok": {...}}` or `{"error": {"kind", "message", "hint"}}`. API errors are
  tool results with `isError: true` (not protocol errors).
- stdout is reserved for MCP JSON-RPC; logs go to stderr (`RUST_LOG`).

Plan: `context-api/agents/plans/20261126_PLAN_CONTEXT_TOOLS/05_MCP.md`.

## Run

```bash
cargo run -p context-mcp -- --store ./.context-engine --allow-fs .
```

| Flag | Default | Meaning |
|---|---|---|
| `--store DIR` | `./.context-engine` (env `CONTEXT_STORE`) | directory of `<name>.ctx` graphs |
| `--allow-fs DIR` | off | allow `files` inputs below `DIR`; relative paths resolve against it |
| `--max-output SIZE` | `32KiB` | byte budget of one tool result (`32768`, `64KiB`, `1MiB`) |
| `--allow-delete` | off | register `context_delete_graph`; allow `overwrite: true` and log deletion |
| `--render-cap N` | `200` | hard vertex cap of `context_render` |

Graphs stay open (and file-locked) while the server runs; every read is
saved immediately. Graphs are addressed by store name only (no paths).

## Client configuration

VS Code `.vscode/mcp.json`:

```json
{
  "servers": {
    "context": {
      "type": "stdio",
      "command": "context-mcp",
      "args": ["--store", "${workspaceFolder}/.context-engine", "--allow-fs", "${workspaceFolder}"]
    }
  }
}
```

Claude Desktop style: the same `command`/`args` under `"mcpServers"`.

## Tools

| Tool | Parameters | Result |
|---|---|---|
| `context_list_graphs` | — | `ok.graphs.graphs[]` |
| `context_graph_info` | `graph` | `ok.stats` (summary + width/parent histograms) |
| `context_create_graph` | `graph`, `texts?`, `split?`, `overwrite?` | `ok.graph` (empty graph) or `ok.read` (with texts) |
| `context_read` | `graph`, `texts?`, `files?`, `split?`, `create?` | `ok.read` (per input: id, label, vertex, chars) |
| `context_search` | `graph`, `query`, `view?` (`summary`), `limit?` (20), `width?` (40), `label_width?` | `ok.search` |
| `context_vertex` | `graph`, `vertex` (`"#123"` = index, else exact text), `limit?`, `label_width?` | `ok.vertex` |
| `context_render` | `graph`, `format?` (`mermaid`), `root?`, `depth?` | `ok.render`; refused with `too_large` over the render cap |
| `context_execute` | a raw `context_api::Request` (`{"command": ..., ...}`) | the raw `Response` |
| `context_help` | — | short usage guide |
| `context_delete_graph` | `graph` (only with `--allow-delete`) | `ok.deleted` |

Search views:

- `summary` — is the query in the graph, and where (a vertex, inside a host
  vertex, a partial match, or not found);
- `occurrences` — every position in the read inputs with line, column and
  snippet (concordance / grep);
- `contexts` — distinct larger vertices containing the match, narrowest
  first (exploration);
- `parents` — direct parent vertices; `path` — the raw search path.

Render formats: `mermaid`, `dot`, `ascii`, `ascii_dag`, `json`. The tool never
forces the gate: render a subgraph with `root` and a small `depth` instead.

### Output budget

Results larger than `--max-output` are shortened item-wise (contexts,
occurrences, parents, inputs, graph lists, render text/graph), halving the
largest list until the JSON fits, and marked:

```json
{"ok": {...}, "truncated_output": true,
 "truncated_items": {"/ok/search/occurrences/items": {"kept": 12, "total": 400}},
 "truncated_hint": "... ask for fewer items (limit), a smaller width/label_width, or render a subgraph with root and depth"}
```

The JSON is re-serialized, never cut mid-way (shortened results have sorted
keys). If nothing can be shortened, the result is a `too_large` error.

### Example calls

```json
{"tool": "context_create_graph", "arguments": {"graph": "repo", "texts": []}}
{"tool": "context_read", "arguments": {"graph": "repo", "files": ["src/lib.rs", "src/main.rs"]}}
{"tool": "context_search", "arguments": {"graph": "repo", "query": "fn parse_", "view": "occurrences", "limit": 10}}
{"tool": "context_search", "arguments": {"graph": "repo", "query": "Result<", "view": "contexts", "limit": 5}}
{"tool": "context_render", "arguments": {"graph": "repo", "root": "impl Display for", "depth": 1}}
{"tool": "context_execute", "arguments": {"command": "inputs", "graph": "repo", "limits": {"limit": 5}}}
```

Result of `context_search {"graph": "demo", "query": "bca", "view": "occurrences"}`
on a graph created from `["abcabca", "bcab"]`:

```json
{"ok":{"search":{"graph":"demo","query":"bca","outcome":{"kind":"vertex","token":{"index":6,"width":3,"label":"bca"}},
 "occurrences":{"total":3,"truncated":false,"inputs_with_matches":2,"items":[
  {"input_id":0,"label":"<text>","offset":1,"line":1,"column":2,"snippet":{"left":"a","matched":"bca","right":"bca","left_truncated":false,"right_truncated":false}},
  {"input_id":0,"label":"<text>","offset":4,"line":1,"column":5,"snippet":{"left":"abca","matched":"bca","right":"","left_truncated":false,"right_truncated":false}},
  {"input_id":1,"label":"<text>","offset":0,"line":1,"column":1,"snippet":{"left":"","matched":"bca","right":"b","left_truncated":false,"right_truncated":false}}]}}}}
```

## Embedding and transports

`ContextServer::new(config)` builds the handler for any rmcp transport
(`server.serve(transport)`); the tests connect a client over
`tokio::io::duplex`. `ContextServer::with_backend(Box<dyn Backend + Send>,
config)` is the extension point for proxy mode (stage 4 `RemoteBackend`
forwarding to a running context server); tool code does not change.

Not yet: proxy mode (`--server URL`), server auto-discovery, structured
content / output schemas (the `Response` schema would add ~10 KB per tool to
`tools/list`; text content carries the same JSON).

## Tests

```bash
cargo test -p context-mcp
npx @modelcontextprotocol/inspector cargo run -p context-mcp -- --store ./ctx-store   # manual
```
