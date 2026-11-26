# Stage 4 — Client mode and interactive shell

Part of [20261126_PLAN_CONTEXT_TOOLS](README.md). Depends on
[stage 2](02_CLI.md) and [stage 3](03_SERVER.md).

## 4.1 Purpose

- Run every CLI command against a running server (`--server URL`), so a
  background process serves a large graph and each command costs one
  request instead of a full load.
- An interactive shell (`context-cli shell`) that works in process or
  remotely, with history, completion and live read progress.
- The same `RemoteBackend` is the client side for MCP proxy mode
  (stage 5) and the protocol reference for the viewer frontend.

Today: no client mode exists; `context-cli repl` (`src/repl.rs`) is
in-process and legacy-only.

## 4.2 `Backend`

```rust
pub trait Backend {
    fn execute(&mut self, request: Request) -> Response;
    /// Events for a graph; local backend emits them in-process, remote over WebSocket.
    fn subscribe(&mut self, graph: &GraphRef, topics: &[Topic]) -> Result<EventStream>;
}
pub struct LocalBackend  { session: context_api::Session }             // stage 2
pub enum   RemoteBackend { Http(HttpClient), Ws(WsClient) }            // this stage
```

- `HttpClient`: blocking `ureq` for one-shot commands (no tokio runtime in
  the common CLI path).
- `WsClient`: tokio + `tokio-tungstenite`, used by the shell and by
  one-shot `read` when progress is shown (subscribe, then execute).
- `Backend` lives in a small shared module (in `context-api` behind
  feature `client`, so context-mcp reuses it without depending on the CLI
  crate).

## 4.3 Selecting the backend

Order of precedence:

1. `--server URL` or `CONTEXT_SERVER` → remote (`--token` /
   `CONTEXT_TOKEN`).
2. `--local` → in process, never auto-connect.
3. Auto-discovery: the command targets a graph whose
   `<graph>.ctx.server.json` names a live server (pid alive, `/v1/health`
   answers within 200 ms) → remote, with a stderr note
   `using server http://127.0.0.1:7878 (graph held by pid 1234)`.
4. Otherwise in process.

So `context-cli search corpus "fox"` works the same whether or not a
server holds `corpus`, and never fails with `Locked` when a live server
owns the graph.

Input files and stdin are read client-side and sent as text (stage 2
§2.4); large inputs (> 1 MiB) use the streamed `POST .../read` route.

## 4.4 Shell

```text
$ context-cli shell corpus            # local, or remote when discovered / --server
context> search "brown fox" --view occurrences
3 occurrences in 2 inputs ...
context> read -f notes/today.md
reading notes/today.md  ████████████░░░░  71%  ...
read 1 input: #1305 (5 812 chars), +214 vertices
context> use other                    # switch default graph
other> show --root "abc" --format mermaid
other> :format json                   # shell settings start with ':'
other> :server http://10.0.0.5:7878   # reconnect
other> :events on                     # print graph_changed events from other clients
other> exit
```

- Line grammar = the CLI grammar without the program name and with an
  optional default graph (`use`), parsed by the same clap definitions
  (`try_parse_from(shell_words::split(line))`), so the shell and the CLI
  cannot diverge.
- Shell-only commands start with `:` (`:format`, `:server`, `:local`,
  `:events`, `:help`, `:quit`).
- Local shell keeps the graph open (exclusive lock) and saves according to
  `:save on-read|manual` (default on-read), and on exit.
- Remote shell: one WebSocket; events printed above the prompt
  (rustyline `ExternalPrinter`).
- Completion: command names, flags, graph names (`List`), and — for
  `search`/`vertex` — no content completion (would need a content index).
- History in `$XDG_STATE_HOME/context-cli/history` (reuse the existing
  repl setup).

## 4.5 Tests

| Test | Asserts |
|---|---|
| `client_equivalence` | the stage-2 CLI integration suite runs twice: local and with `--server` against an in-test server; outputs (text and JSON) identical |
| `auto_discovery` | with a server holding the graph, a plain CLI command uses it (stderr note) and succeeds; with a stale `.server.json` (dead pid) it falls back to local |
| `client_files_are_local` | `read -f` with `--server` sends text; server started without `--allow-fs` accepts it |
| `shell_script` | `context-cli shell < script.txt` (non-TTY mode reads commands line by line) produces the expected transcript, local and remote |
| `shell_events` | two remote shells; a read in one produces a `graph_changed` line in the other |
| `token` | wrong token → clear error, exit 1 |

```bash
cargo test -p context-cli --test client
# manual
context-cli serve corpus --preload &
context-cli read corpus -f new.txt                # auto-discovered server
context-cli --server http://127.0.0.1:7878 search corpus "fox"
context-cli shell corpus
```
