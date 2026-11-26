# context-http

HTTP and WebSocket server of the context API. It keeps graphs open in one
`context_api::Session`, so shells, scripts, agents and viewers do not reload
a large graph per command, and executes the same `Request`s as the CLI:
every transport returns identical `Response` JSON.

Plan: [`context-api/agents/plans/20261126_PLAN_CONTEXT_TOOLS/03_SERVER.md`](../../../context-api/agents/plans/20261126_PLAN_CONTEXT_TOOLS/03_SERVER.md).

## Running

```bash
cargo run -p context-http -- --store ./.context-engine corpus
# {"listening":"http://127.0.0.1:7878","pid":12345}
```

Once bound, the server prints exactly one JSON line to stdout (logs go to
stderr, filter with `RUST_LOG`). Use `--http 127.0.0.1:0` for an ephemeral
port and read the URL from that line.

| Flag | Default | Meaning |
|---|---|---|
| `GRAPH...` | — | graphs opened at start (others open on first use) |
| `--http <ADDR>` | `127.0.0.1:7878` | listen address (`HOST:PORT` or a bare port) |
| `--store <DIR>` | `.context-engine` (env `CONTEXT_STORE`) | graph store directory |
| `--save <POLICY>` | `on-read` | `on-read` (save after every mutation), `interval=30s` (save dirty graphs periodically; `ms`/`s`/`m`/`h`), `manual` (only on `save`). Every policy saves dirty graphs on shutdown |
| `--token <TOKEN>` | env `CONTEXT_TOKEN` | bearer token; **required** when ADDR is not loopback |
| `--allow-fs <DIR>` | refused | allow `{"kind":"file"}` inputs below DIR (canonicalized, no `..`/symlink escape) |
| `--max-body <SIZE>` | `64MiB` | request body / WebSocket message limit (`B`, `KB`, `KiB`, `MB`, `MiB`, ...) |
| `--render-cap <N>` | `2000` | hard vertex cap of render/export; `force` cannot exceed it |
| `--cors-origin <ORIGIN>` | none | allowed CORS origin (repeatable); no CORS headers by default |
| `--static <DIR>` | — | serve a frontend for non-`/v1` paths (SPA fallback to `index.html`) |

Ctrl-C / SIGTERM shut down gracefully: in-flight requests finish, dirty
graphs are saved, locks released and discovery files removed.

### As a library

```rust
let mut config = context_http::ServerConfig::new("./.context-engine");
config.addr = "127.0.0.1:0".parse()?;
context_http::serve(config, async { let _ = tokio::signal::ctrl_c().await; }).await?;
```

`ServeArgs` (clap `Args`, `into_config()`) lets other binaries embed the same
flags (`context-cli serve`). `Server::bind` + `Server::run` separate binding
from serving (tests use them to learn the port); `AppState::new` + `router`
give the bare axum router.

## Routes

All routes are under `/v1`. Unless noted, the body is the `Response` JSON
(`{"id":..,"ok":{...}}` or `{"id":..,"error":{"kind","message","hint"}}`).

| Route | Command |
|---|---|
| `POST /v1/execute` | body `Request` (`{"id":1,"command":"info","graph":"g"}`) → `Response`; every command, including the log commands |
| `GET /v1/ws` | WebSocket, see below |
| `GET /v1/health` | `{"status":"ok","version":..,"graphs_open":[..]}` (no auth) |
| `GET /v1/graphs` | `list` |
| `GET /v1/graphs/{g}` | `info` |
| `GET /v1/graphs/{g}/search?q=&view=&limit=&width=` | `search` (`view`: `summary`, `path`, `contexts`, `occurrences`, `parents`; `width` = context width) |
| `GET /v1/graphs/{g}/vertex/{ref}` | `vertex`; digits select an index, anything else the vertex text |
| `GET /v1/graphs/{g}/render?format=&root=&depth=&force=` | `render`; raw body: `dot` → `text/vnd.graphviz`, `ascii`/`ascii_dag`/`mermaid` → `text/plain; charset=utf-8`, `json` → the `Response` JSON |
| `POST /v1/graphs/{g}/read?label=&split=&create=` | `read` of the `text/plain` body (UTF-8) as one input source (`split`: `none`, `lines`, `paragraphs`) |
| `POST /v1/graphs/{g}/save` | `save` |

Status codes by error kind: `invalid_input` 400, `forbidden` 403,
`not_found` 404, `already_exists`/`locked` 409, `too_large` 413 (render gate
or body limit), `unsupported` 501, `corrupt`/`io`/`internal` 500; a missing or
wrong token gives 401.

```bash
URL=http://127.0.0.1:7878
curl -s $URL/v1/health
curl -s $URL/v1/execute -d '{"command":"new","graph":"corpus"}'
curl -s --data-binary @book.txt "$URL/v1/graphs/corpus/read?label=book.txt&split=paragraphs"
curl -s $URL/v1/graphs/corpus | jq .ok.graph
curl -s "$URL/v1/graphs/corpus/search?q=brown%20fox&view=occurrences&limit=10" | jq
curl -s "$URL/v1/graphs/corpus/render?format=dot&root=brown%20fox&depth=2" | dot -Tsvg > fox.svg
curl -s -X POST $URL/v1/graphs/corpus/save
# with a token
curl -s -H "Authorization: Bearer $CONTEXT_TOKEN" $URL/v1/graphs
```

## WebSocket protocol (`/v1/ws`)

Text frames, one JSON message per frame. Authentication: the
`Authorization: Bearer` header or `?token=` (browsers).

```jsonc
// client → server
{"id": 1, "command": "search", "graph": "corpus", "query": "brown fox", "view": "contexts"}
{"id": 2, "subscribe": {"graph": "corpus", "topics": ["graph_changed", "read_progress"]}}
{"id": 3, "unsubscribe": {"graph": "corpus"}}

// server → client
{"id": 1, "ok": {"search": {...}}}
{"id": 2, "ok": {"subscribed": ["graph_changed", "read_progress"]}}
{"id": 3, "ok": {"unsubscribed": "corpus"}}
{"event": "read_progress", "graph": "corpus", "request": 7, "input": 0, "chars": 120000, "total": 500000}
{"event": "graph_changed", "graph": "corpus", "generation": 42, "vertex_count": 18304, "new_inputs": [3]}
{"event": "saved", "graph": "corpus", "generation": 42}
{"event": "lagged", "skipped": 17}
```

- Requests run concurrently; responses carry the request `id` and may arrive
  out of order (a query finishes before an earlier long read).
- `subscribe` without `topics` subscribes to all (`graph_changed`,
  `read_progress`, `saved`); `unsubscribe` without `graph` drops all.
- Events come from a bounded broadcast channel (1024); a subscriber that
  falls behind receives `{"event":"lagged","skipped":N}` instead of slowing
  the server.
- Malformed messages are answered with
  `{"id": <id or null>, "error": {"kind": "invalid_input", ...}}`.

```bash
websocat ws://127.0.0.1:7878/v1/ws
{"id":1,"subscribe":{"graph":"corpus"}}
{"id":2,"command":"read","graph":"corpus","inputs":[{"kind":"text","text":"hello world"}]}
```

## Execution model

- Every command runs in `tokio::task::spawn_blocking` on one shared
  `Arc<Session>`: mutations of a graph are serialized by its writer slot,
  queries run concurrently and see committed reader steps. Algorithm panics
  are caught by the session (error response, unsaved changes dropped).
- The server holds the exclusive lock of every open graph and writes
  `<graph>.ctx.server.json` (`{url, pid, token_required}`, never the token)
  when a graph opens; it is removed when the graph is closed or deleted and on
  shutdown. A one-shot CLI command on a served graph fails with `locked`,
  naming the server URL.

## Tests

```bash
cargo test -p context-http
cargo test -p context-http --release -- --ignored   # 5 MB read
```

Each test starts a server on an ephemeral port with a temp store
(`tests/common`): `execute_equivalence`, `rest_routes`, `read_text_body`,
`ws_pipelining`, `concurrent_read`, `save_policies`, `auth_and_fs`,
`lock_and_discovery`.
