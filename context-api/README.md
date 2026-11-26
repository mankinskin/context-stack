# context-api

The command API of the context stack. Every tool — the
[CLI and shell](../tools/cli/context-cli/README.md), the
[HTTP/WebSocket server](../tools/http/context-http/README.md) and the
[MCP server](../tools/mcp/context-mcp/README.md) — sends a `Request` to a
`Session` (in process or through a server) and gets a `Response`, so every
transport returns the same JSON.

```text
context-cli / shell ─┐
context-http ────────┼──► Session::execute(Request) -> Response
context-mcp ─────────┘          │
                    context-read · context-search · context-trace
```

`read` (with `create` and `import`) is the only command that adds vertices:
text is read with `context_read::read`. There is no insert command. Search
uses positioned `context-search`; nothing uses the legacy row model.

Plan and design notes:
[agents/plans/20261126_PLAN_CONTEXT_TOOLS](agents/plans/20261126_PLAN_CONTEXT_TOOLS/README.md).

## Usage

```rust
use context_api::{Command, CommandResult, Request, Session, SessionConfig};
use context_api::types::{Input, SearchView, Split};

let session = Session::new(SessionConfig::new(".context-engine"));
let create = Command::Create {
    graph: "notes".into(),
    inputs: vec![Input::Text { text: "the quick brown fox".into(), label: None }],
    split: Split::None,
    overwrite: false,
};
session.execute(Request::new(create)).into_result()?;

let search = Command::Search {
    graph: "notes".into(),
    query: "brown".into(),
    view: SearchView::Occurrences,
    limits: Default::default(),
};
if let CommandResult::Search(result) = session.execute(Request::new(search)).into_result()? {
    println!("{:?}", result.occurrences);
}
```

JSON (all transports):

```jsonc
// request
{"id": 1, "command": "search", "graph": "notes", "query": "brown", "view": "contexts"}
// success
{"id": 1, "ok": {"search": {"graph": "notes", "query": "brown", "outcome": {...}, "contexts": {...}}}}
// failure
{"id": 1, "error": {"kind": "not_found", "message": "graph 'x' not found", "hint": "..."}}
```

## Commands

| Command | Purpose |
|---|---|
| `new`, `create`, `read` | create an empty graph; create from inputs; read inputs (`text`, or `file` below `allow_fs`) with optional `split` (`lines`, `paragraphs`), `dry_run`, `steps`, `save` policy |
| `search` | find a query; views `summary`, `path` (the search response), `contexts` (ancestors containing the match, narrowest first), `occurrences` (positions in the read inputs with line, column and snippet), `parents` |
| `vertex`, `inputs`, `info`, `stats`, `validate` | inspect a vertex (by index or text), the input log, counts, histograms, structural and retention checks |
| `render` | `ascii`, `ascii_dag`, `dot`, `mermaid`, `json` of the graph or of the descendants of `root` (with `depth`), behind a size gate |
| `export`, `import` | v3 interchange JSON (the oracle format, gated) or the native `.ctx` bytes |
| `list`, `delete`, `save`, `close` | store and session lifecycle |
| `list_logs` … `delete_logs` | trace logs (feature `logs`) |

Search outcomes: `not_found` (unknown chars or empty query), `vertex` (the
query is a vertex), `in_host` (the query occurs inside its narrowest host),
`partial` (only a query prefix is in the graph). Result lists honour
`Limits` (`limit`, `context_width`, `label_width`, `max_depth`,
`max_visited`) and report `truncated` / `total`.

The render gate defaults to 64 vertices and 4 096 label characters;
`Gate::force` lifts it, except for the session's `render_cap`.

## Storage

```text
<store>/notes.ctx              graph file: magic, version, JSON metadata
                               (input log, counts), bincode graph, crc32
<store>/notes.ctx.lock         advisory lock (exclusive while a writer has it open)
<store>/notes.ctx.server.json  present while a server holds the graph
```

- Graphs are addressed by name (`<store>/<name>.ctx`) or, where allowed
  (`SessionConfig::allow_graph_paths`, or below `allow_fs`), by a path
  ending in `.ctx`.
- Saves replace the file atomically (temporary file, fsync, rename); a
  crash leaves the previous file. `create` writes the file once, at the end.
- Loading validates the checksum, the graph (context-trace `serde`
  feature: structure, store checks) and the input log.
- Old row-model workspaces (`<store>/<name>/metadata.json`) are rejected
  with a hint to re-create the graph from its source text.

## Sessions

`SessionConfig` selects the behaviour of a transport:

| Field | One-shot CLI | Server / shell / MCP |
|---|---|---|
| `keep_open` | false: open, run, close per command | true: graphs stay open (exclusive lock) |
| `save` | `OnWrite` | `OnWrite` or `Manual` (save on `save`, `close`, `shutdown`) |
| `allow_fs` | `None` (the CLI reads files itself) | directory for `file` inputs and path graph refs |
| `render_cap`, `max_visited_cap` | — | hard limits for remote clients |

`Session::execute` takes `&self`: mutations of a graph are serialized by a
per-graph writer slot, queries run concurrently. Every command runs on a
thread with a large stack (the algorithms recurse over the nesting depth);
an algorithm panic drops the graph's unsaved state and returns an
`internal` error. Events (`read_progress`, `graph_changed`, `saved`) go to
an `EventSink`.

## Features

| Feature | Default | Purpose |
|---|---|---|
| `logs` | on | trace capture and log queries (`<graph>.ctx.logs/`) |
| `client` | off | `client::RemoteBackend` (HTTP), server discovery, WebSocket event streams |
| `ts-gen` | off | TypeScript bindings in `packages/context-types/src/generated/` (`cargo test -p context-api --features ts-gen --lib export_bindings`) |

## Tests

```bash
cargo test -p context-api   # unit tests and tests/acceptance (oracle, brute-force search, round trips, gate)
```
