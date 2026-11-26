# Stage 3 — HTTP and WebSocket server

Part of [20261126_PLAN_CONTEXT_TOOLS](README.md). Depends on
[stage 1](01_CORE_API.md). Consumed by [stage 4](04_CLIENT_SHELL.md),
[stage 5](05_MCP.md) (proxy mode) and the future viewer.

## 3.1 Purpose

A long-running process keeps one or more graphs in memory, so that
interactive shells, scripts, agents and the viewer do not reload a large
graph for every command. It executes the same `Request`s as the CLI and
pushes events (graph changed, read progress; later viewer telemetry).

## 3.2 What changes in `tools/http/context-http`

| Today | Fate |
|---|---|
| `POST /api/execute` (`rpc.rs`) with `trace` flag | **replace** by `POST /v1/execute` with the stage-1 envelope; trace via feature `logs` |
| REST `/api/workspaces/...` (`rest.rs`) | **replace** by the `/v1/graphs/...` read routes below |
| log REST (`log_rest.rs`) | keep behind feature `logs`, move under `/v1/graphs/{g}/logs` |
| GraphQL feature | **drop** (D6) |
| `AppState { Arc<Mutex<WorkspaceManager>> }` | **replace** by `Arc<Session>` with per-graph slots (D8) |
| `CorsLayer::permissive()` | **replace** by configured origins (default none; `--cors-origin`) |
| static dir fallback | keep (`--static DIR`) for the future viewer frontend |
| `main.rs` bin | thin wrapper over `context_http::serve(ServerConfig)`; `context-cli serve` calls the same function |

New dependencies: axum `ws` feature, `tokio::sync::broadcast` for events.

## 3.3 Command line

```text
context-cli serve [GRAPH...]
  --http <ADDR>               default 127.0.0.1:7878 (port 0 = ephemeral, printed on stdout)
  --store <DIR>
  --preload                   open listed graphs at start (default: lazily on first use)
  --save <on-read|interval=30s|manual>   default on-read
  --token <TOKEN>             bearer token; required when ADDR is not loopback (env CONTEXT_TOKEN)
  --allow-fs <DIR>            allow Input::File paths below DIR (default: refused)
  --max-body <SIZE>           default 64MiB
  --render-cap <N>            hard vertex cap for Render/Export, not liftable by force (default 2000)
  --cors-origin <ORIGIN>      repeatable
  --static <DIR>              serve a frontend
```

Startup prints one machine-readable line to stdout,
`{"listening":"http://127.0.0.1:7878","pid":1234}`, so scripts and tests
can start with port 0.

## 3.4 Routes

| Route | Purpose |
|---|---|
| `POST /v1/execute` | body `Request` → `Response` (every command) |
| `GET /v1/ws` | WebSocket, see 3.5 |
| `GET /v1/health` | `{status, version, graphs_open}` |
| `GET /v1/graphs` | `List` |
| `GET /v1/graphs/{g}` | `Info` |
| `GET /v1/graphs/{g}/search?q=&view=&limit=&width=` | `Search` (GET for browser/curl use) |
| `GET /v1/graphs/{g}/vertex/{ref}` | `Vertex` |
| `GET /v1/graphs/{g}/render?format=&root=&depth=&force=` | `Render`, raw body with content type (`text/vnd.graphviz`, `text/plain`, JSON) |
| `POST /v1/graphs/{g}/read?label=&split=&create=` | body `text/plain` (UTF-8) streamed into one `Read`; `curl --data-binary @file` |
| `POST /v1/graphs/{g}/save` | `Save` |

HTTP status mapping: `Response::Error` kinds → 400 (bad request), 404 (graph
or vertex not found), 409 (locked / already exists), 413 (gate or body
limit), 500 (internal, incl. caught panic). The body is always the
`Response` JSON, so clients need only one decoder.

## 3.5 WebSocket protocol

Text frames, JSON, one message per frame:

```jsonc
// client → server
{"id": 1, "command": "search", "graph": "corpus", "query": "brown fox", "view": "contexts"}
{"id": 2, "subscribe": {"graph": "corpus", "topics": ["graph_changed", "read_progress"]}}
{"id": 3, "unsubscribe": {"graph": "corpus"}}

// server → client
{"id": 1, "ok": {"search": {...}}}
{"id": 2, "ok": {"subscribed": ["graph_changed", "read_progress"]}}
{"event": "read_progress", "graph": "corpus", "request": 7, "input": 0, "chars": 120000, "total": 500000}
{"event": "graph_changed", "graph": "corpus", "generation": 42, "vertex_count": 18304, "new_inputs": [3]}
{"event": "saved", "graph": "corpus", "generation": 42}
```

- Requests on one socket may be pipelined; responses carry the request
  `id` and may arrive out of order (searches can finish before an earlier
  read).
- Events are versioned by an `Event` enum in context-api
  (`#[serde(tag = "event")]`, ts-rs exported) so the viewer gets typed
  bindings. Read progress is throttled (≥ 100 ms or ≥ 1 % between events).
- Slow subscribers: bounded broadcast channel; lagging clients receive
  `{"event":"lagged","skipped":N}` instead of blocking the server.
- Reserved topic `telemetry` for viewer telemetry (search/read traversal
  events) once the positioned crates emit them — out of scope here; the
  enum variant is not added until then.

## 3.6 Execution model

- Every request runs in `tokio::task::spawn_blocking` (the algorithms are
  synchronous and CPU-bound).
- Per graph (D8): mutating commands (`Read`, `Create`, `Import`, `Delete`,
  `Close`, `Save`) take the slot's writer mutex, so reads are serialized;
  queries run concurrently and take `PositionedGraphRef` read guards.
  A search running during a read observes a consistent committed graph
  (the reader commits per step under the write guard).
- Panic isolation: each execution is wrapped in `catch_unwind`. A panic
  during a mutation marks the slot poisoned, drops the in-memory graph and
  reloads the last saved file on next use; the response is
  `Error { kind: internal, message: "reader panicked …; graph reloaded from
  generation N" }`.
- Persistence policy `--save`: `on-read` saves atomically after every
  mutating request (default, simplest guarantee); `interval` saves dirty
  graphs periodically; `manual` only on `Save`. Every policy saves dirty
  graphs on graceful shutdown (SIGINT/SIGTERM, `axum::serve(...)
  .with_graceful_shutdown`).
- The server holds the exclusive file lock of every open graph and writes
  `<graph>.ctx.server.json` `{url, pid, token_required}` (never the token);
  removed on shutdown, stale files (dead pid) are ignored by clients.

## 3.7 Security

- Default bind loopback; non-loopback requires `--token`
  (`Authorization: Bearer`; WebSocket: same header or `?token=` for browser
  clients).
- `Input::File` refused unless `--allow-fs DIR`; paths canonicalized and
  required to stay below DIR (no `..`/symlink escape).
- Body limit (`--max-body`), render/export hard cap, search work budget
  (`max_visited` capped server-side).
- No CORS by default.

## 3.8 Tests

`axum-test` for HTTP, `tokio-tungstenite` for WebSocket; each test uses a
temp store and an ephemeral port.

| Test | Asserts |
|---|---|
| `execute_equivalence` | a script of requests gives the same `Response` JSON via `/v1/execute` as via `Session::execute` in process |
| `rest_routes` | search/info/render/vertex routes; status code mapping; render content types |
| `read_text_body` | `POST .../read` with a 5 MB body; progress events received on a subscribed socket; resulting graph `diff`-equal to a local `create` |
| `ws_pipelining` | interleaved ids; out-of-order responses matched by id |
| `concurrent_search_during_read` | searches succeed while a long read runs; every search result is valid for some generation |
| `save_policies` | `on-read` file updated after read; `manual` not until `Save`; graceful shutdown saves dirty graphs |
| `panic_recovery` | test-only command injecting a panic in a mutation → error response, graph reloaded, later requests succeed |
| `auth_and_fs` | missing/wrong token → 401 on non-loopback config; `Input::File` refused without `--allow-fs`, refused outside DIR |
| `lock_and_discovery` | `.server.json` written/removed; a CLI one-shot `read` on the served graph fails with `Locked` and names the server URL |

```bash
cargo test -p context-http
# manual
cargo run -p context-cli -- serve corpus --http 127.0.0.1:7878 &
curl -s localhost:7878/v1/health
curl -s 'localhost:7878/v1/graphs/corpus/search?q=brown%20fox&view=occurrences' | jq
curl -s --data-binary @book.txt 'localhost:7878/v1/graphs/corpus/read?label=book.txt'
websocat ws://127.0.0.1:7878/v1/ws   # then type requests
```
