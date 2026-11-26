# context-cli

Command-line tool for context-engine graphs on the positioned stack. Every
command builds a `context_api::Request` and runs it on an in-process
`Session` (`LocalBackend`) or on a server (`context_api::client::RemoteBackend`,
client mode); `--format json` prints the `Response` envelope unchanged, so
scripts see the same JSON as the server and MCP transports.

The same binary runs the server (`serve`), an interactive shell (`shell`)
and the MCP server (`mcp`).

Plans: `context-api/agents/plans/20261126_PLAN_CONTEXT_TOOLS/02_CLI.md`,
`04_CLIENT_SHELL.md`.

## Usage

```text
context-cli [GLOBAL] <COMMAND>

GLOBAL
  --store <DIR>          graph store (default ./.context-engine, env CONTEXT_STORE)
  --server <URL>         run on a server (client mode), env CONTEXT_SERVER
  --token <TOKEN>        bearer token of the server, env CONTEXT_TOKEN
  --local                run in process: ignore CONTEXT_SERVER, no auto-discovery
  --format <text|json>   output format (also accepted after the command)
  -q, --quiet            no progress on stderr

GRAPH COMMANDS                                    <GRAPH> = name or path/to/file.ctx
  new     <GRAPH> [--force]                       create an empty graph
  create  <GRAPH> [INPUT...] [--force]            new + read + save
  read    <GRAPH> [INPUT...] [--create] [--dry-run] [--steps]
  search  <GRAPH> <QUERY> [--view summary|path|contexts|occurrences|parents]
                  [--limit N] [--width N] [--max-depth N] [--label-width N]
                  [--fail-if-missing]
  show    <GRAPH> [--root <VERTEX>] [--depth N] [--format ascii|ascii-dag|dot|mermaid|json]
                  [--max-vertices N] [--force] [-o FILE]
  vertex  <GRAPH> <VERTEX> [--limit N]            VERTEX = #index or text
  inputs  <GRAPH> [--limit N]
  info    <GRAPH>        stats <GRAPH>       validate <GRAPH>
  export  <GRAPH> --format v3|native [-o FILE] [--force]
  import  <GRAPH> <FILE> [--force]
  list                   delete <GRAPH>

MODES
  shell   [GRAPH]                                 interactive shell (see below)
  serve   [GRAPH...] [--http ADDR] [--save on-read|interval=30s|manual]
          [--token T] [--allow-fs DIR] [--max-body SIZE] [--render-cap N]
          [--cors-origin O] [--static DIR]        HTTP/WebSocket server (feature `server`)
  mcp     [--allow-fs DIR] [--max-output SIZE] [--allow-delete] [--render-cap N]
                                                  MCP server on stdio (feature `mcp`)

INPUT (repeatable, kept in order, each becomes one reader input unless split)
  TEXT                   positional text argument
  -t, --text <TEXT>      explicit text (for text starting with '-')
  -f, --file <PATH>      a file; a directory with --recursive [--glob '*.md']
  -                      stdin (also implied when no INPUT is given and stdin is not a TTY)
  --split none|lines|paragraphs   split every source into several inputs
```

Files are read by the CLI (UTF-8 required) and sent as text labelled with
their path; stdin is labelled `<stdin>`. Directories are walked in sorted
order. Inputs larger than 1 MiB show a progress line on stderr when it is a
terminal (suppressed by `-q`).

`show -o FILE` writes atomically and infers the format from the extension
when `--format` is not given: `.dot`, `.mmd`, `.md` (mermaid in a fenced
block), `.json`. Graphs larger than the render gate (64 vertices / 4 096
label chars) are refused; render a subgraph with `--root` or pass `--force`.

`export --format native` writes the `.ctx` bytes to `-o FILE` (to stdout
only as the base64 envelope with `--format json` before the command).
`import` detects native files by their `CTXG` magic, anything else is read
as a v3 interchange JSON document.

stdout carries only the result; errors (`error: …`), hints (`hint: …`) and
progress go to stderr.

| Exit code | Meaning |
|---|---|
| 0 | success (a search without match too, unless `--fail-if-missing`) |
| 1 | command error: not found, already exists, locked, gate refused, invalid input, unsupported; `validate` found errors; `--fail-if-missing` and the query is not (fully) found |
| 2 | usage error |
| 3 | I/O error, invalid UTF-8 input, corrupt graph file |

## Examples

Index a folder of notes and search it:

```bash
# one input per file, in directory order
context-cli create notes -f ~/notes --recursive --glob '*.md'
context-cli info notes

# where does a phrase occur? (concordance)
context-cli search notes "context engine" --view occurrences --width 30

# add today's note later; saved atomically, nothing written on error
context-cli read notes -f ~/notes/2026-11-26.md

# script use: exit 1 if missing
context-cli search notes "TODO(linus)" --fail-if-missing -q && echo "open todos"
```

Explore the contexts of a word:

```bash
context-cli create poems -f poems.txt --split paragraphs
context-cli search poems "the sea" --view contexts --limit 10
context-cli search poems "the sea" --view parents         # direct parents only
context-cli vertex poems "the sea"                         # composition and parents
context-cli vertex poems '#4711'                           # by index
```

Small graphs: draw and export:

```bash
context-cli create demo "abcabca" "bcab"
context-cli show demo                                   # ASCII layers
context-cli show demo --format mermaid -o demo.md       # fenced mermaid block for docs
context-cli show demo --format dot | dot -Tsvg > demo.svg
context-cli show notes --root "context engine" --depth 2 --format dot -o ce.dot  # subgraph of a big graph
context-cli export demo --format v3 -o demo.v3.json     # oracle interchange format
context-cli import demo2 demo.v3.json
```

Pipelines and stdin:

```bash
git log --format=%s | context-cli create commits --split lines
journalctl -u myservice --since today -o cat | context-cli read logs --create --split lines
context-cli search commits "fix(" --view occurrences --format json \
  | jq -r '.ok.search.occurrences.items[].snippet.matched'
```

## Server and client mode

`context-cli serve` keeps graphs in memory (the `context-http` server; flags
as `context-http`, see `tools/http/context-http/README.md`). It prints one
line `{"listening":"http://127.0.0.1:7878","pid":1234}` on stdout (port 0
binds an ephemeral port), writes `<graph>.ctx.server.json` next to every
graph it holds and saves dirty graphs on Ctrl-C / SIGTERM.

Every command then selects where it runs:

1. `--server URL` or `CONTEXT_SERVER` → the server (`--token` /
   `CONTEXT_TOKEN` for servers started with `--token`);
2. `--local` → in process, never auto-connect (wins over `CONTEXT_SERVER`);
3. auto-discovery: the graph's `.server.json` names a running process and
   its `/v1/health` answers within 200 ms → the server, with a note on stderr
   `using server http://127.0.0.1:7878 (graph held by pid 1234)` (not with
   `-q`); stale discovery files are ignored;
4. otherwise in process.

So a command works the same whether or not a server holds the graph, and
never fails with `locked` because of a live server. Outputs are identical in
both modes. Files and stdin are always read by the CLI and sent as text (the
server needs no `--allow-fs`); a `read` input larger than 1 MiB is sent
through the streamed `POST /v1/graphs/{g}/read` route, one input per request
in order, and the reports are merged into one. Progress of remote reads
comes from the server's WebSocket events. Graph paths (`*.ctx`) are never
auto-discovered.

```bash
# keep a large graph in memory
context-cli serve wiki &                 # opens wiki at start, prints {"listening":"http://127.0.0.1:7878",...}
context-cli search wiki "black hole" --view occurrences   # note: using server …
context-cli read wiki -f new_article.txt          # no reload, no lock conflict

# shared team index on a host
context-cli serve docs --http 0.0.0.0:7878 --token "$CONTEXT_TOKEN" --save interval=60s
CONTEXT_SERVER=http://indexhost:7878 CONTEXT_TOKEN=... context-cli search docs "deploy key"
```

An unreachable server is an I/O error (exit 3) naming the URL; a missing or
wrong token is refused (exit 1) with a hint. Only `http://` URLs are
supported by the client.

## Shell

```text
$ context-cli shell corpus            # local, or remote when discovered / --server
corpus> search "brown fox" --view occurrences
corpus> read -f notes/today.md
corpus> use other                     # switch the default graph
other> show --root "abc" --format mermaid
other> :format json                   # shell settings start with ':'
other> :server http://10.0.0.5:7878   # run on a server from now on
other> :events on                     # print graph_changed / saved events of other clients
other> exit
```

- A line is a graph command without the program name, parsed by the same
  clap definitions as the CLI (`help`, `help search`). Quoting follows the
  shell (`shell-words`).
- Default graph: `shell GRAPH` or `use GRAPH`. A command that misses a
  required argument is retried with the default graph inserted as its first
  argument: `search bca` = `search GRAPH bca`, `info` = `info GRAPH`. A
  command with all its positionals still names its own graph
  (`search other bca`). Give `read`/`create` texts with `-t` (`read -t xyz`),
  since a lone positional is taken as the graph; stdin is never read as
  input in the shell.
- Settings: `:format text|json`, `:server URL`, `:local`, `:events on|off`,
  `:save on-read|manual` (local), `:status`, `:help`, `:quit` (also `exit`,
  `quit`, Ctrl-D).
- Local shell: one session keeps the graphs open (exclusively locked) and
  saves after every read (`:save on-read`, default) or only on `save` and on
  exit (`:save manual`); dirty graphs are saved on exit.
- Remote shell: one WebSocket subscription on the default graph; with
  `:events on`, `graph_changed` and `saved` events of other clients (e.g. an
  agent through `context-cli mcp --server`) are printed above the prompt.
- On a terminal: line editing, completion of commands, settings and
  `--flags`, history in `$XDG_STATE_HOME/context-cli/history`
  (`~/.local/state/context-cli/history`).
- When stdin is not a terminal, commands are read line by line without
  prompt or history (`#` comments and empty lines are skipped); the exit code
  is that of the last failed command (0 if all succeeded):

```bash
printf 'search bca\nread -t xyzxyz\ninfo\n' | context-cli shell demo
```

## MCP

`context-cli mcp` serves the `context-mcp` tools on stdio
(`tools/mcp/context-mcp/README.md`). In process by default (the global
`--store`); with `--server URL` it is a proxy forwarding every tool call to a
running server, so an agent and a human share one graph (the agent's reads
appear as events in the shell). The MCP-side policies (`--max-output`,
`--allow-delete`, `--render-cap`, graph names only) apply in both modes; in
proxy mode `files` inputs are read by the server and need its `--allow-fs`.

```bash
context-cli mcp --store .context-engine --allow-fs .        # local
context-cli serve research &
context-cli mcp --server http://127.0.0.1:7878             # proxy
context-cli shell research     # sees the agent's reads as graph_changed events
```

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

## Features

| Feature | Default | Adds |
|---|---|---|
| `server` | yes | `serve` (context-http, tokio) |
| `mcp` | yes | `mcp` (context-mcp, tokio) |

Client mode and the shell are always built (blocking `ureq` / `tungstenite`,
no async runtime): `cargo build -p context-cli --no-default-features`.

## Tests

```bash
cargo test -p context-cli
```

Integration tests (`tests/`) run the binary against a temporary store with
`assert_cmd`; `tests/client.rs` (client equivalence, auto-discovery, shell,
events, tokens) and `tests/mcp_proxy.rs` start `context-cli serve` child
processes on ephemeral ports.
