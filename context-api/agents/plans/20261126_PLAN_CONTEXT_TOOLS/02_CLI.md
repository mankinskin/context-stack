# Stage 2 — Standalone CLI

Part of [20261126_PLAN_CONTEXT_TOOLS](README.md). Depends on
[stage 1](01_CORE_API.md).

## 2.1 What changes in `tools/cli/context-cli`

| Today | Fate |
|---|---|
| `src/main.rs`: ≈45 clap subcommands mirroring legacy `Command` variants, explicit open/save per call | **rewrite**: the command set below; each one-shot command opens, executes and (for mutations) saves itself |
| `src/repl.rs` (1 371 lines, rustyline) | **replaced** by `shell` in [stage 4](04_CLIENT_SHELL.md); reuse its rustyline setup, history file and completion |
| `src/output.rs` | **rewrite**: text renderers per `CommandResult`, `--format json` = the `Response` JSON unchanged |
| `tests/integration/*` (atoms, dedup, legacy oracle, skill3 exploration) | **delete** legacy suites; new suites §2.6 |
| `tests/common/graph_compare.rs` (legacy) | **delete**; use `positioned::diff` |

The CLI never calls algorithm crates directly; it builds `Request`s and
hands them to a `Backend` (`LocalBackend` = in-process `Session`). Stage 4
adds `RemoteBackend` without touching command code.

## 2.2 Command line

```text
context-cli [GLOBAL] <COMMAND>

GLOBAL
  --store <DIR>          graph store (default ./.context-engine, env CONTEXT_STORE)
  --server <URL>         run against a server (stage 4, env CONTEXT_SERVER)
  --format <text|json>   output format (default text; json = Response envelope)
  -q, --quiet            no progress / warnings on stderr
  --trace                per-command tracing capture (feature logs)

GRAPH COMMANDS                                    <GRAPH> = name or path/to/file.ctx
  new     <GRAPH> [--force]                       create an empty graph
  create  <GRAPH> [INPUT...] [--force]            new + read + save
  read    <GRAPH> [INPUT...] [--create] [--dry-run] [--steps]
  search  <GRAPH> <QUERY> [--view summary|path|contexts|occurrences|parents]
                  [--limit N] [--width N] [--max-depth N]
  show    <GRAPH> [--root <VERTEX>] [--depth N] [--format ascii|ascii-dag|dot|mermaid|json]
                  [--max-vertices N] [--force] [-o FILE]
  vertex  <GRAPH> <VERTEX>                        VERTEX = #index or text
  inputs  <GRAPH> [--limit N]
  info    <GRAPH>        stats <GRAPH>       validate <GRAPH>
  export  <GRAPH> --format v3|native [-o FILE]
  import  <GRAPH> <FILE> [--force]
  list                   delete <GRAPH>

INPUT (repeatable, kept in order, each becomes one reader input unless split)
  TEXT                   positional text argument
  -t, --text <TEXT>      explicit text (for text starting with '-')
  -f, --file <PATH>      a file; a directory with --recursive [--glob '*.md']
  -                      stdin (also implied when no INPUT is given and stdin is not a TTY)
  --split none|lines|paragraphs   split every source into several inputs (default none)

SERVICES
  serve   [GRAPH...] [--http ADDR] ...            stage 3
  shell   [GRAPH]                                 stage 4
  mcp     [--server URL]                          stage 5
```

Conventions:

- **Exit codes:** 0 success; 1 command error (not found, locked, gate
  refused); 2 usage error (clap); 3 I/O or corrupt file. A search with no
  match exits 0 by default and 1 with `--fail-if-missing` (grep-like use in
  scripts).
- **stdout** carries only the result; progress, warnings and hints go to
  stderr (so `--format json | jq` works).
- `read` without `--create` on a missing graph fails with a hint;
  `create` on an existing graph fails without `--force`.
- `--dry-run` reads into an in-memory copy and reports what would change
  (vertex delta, input tokens) without saving.
- Progress: for inputs > 1 MiB a stderr progress bar (chars consumed) fed
  by `read_observed`; suppressed by `-q` or non-TTY stderr.

## 2.3 Text output formats

```text
$ context-cli search corpus "brown fox"
found: vertex #812 "brown fox" (width 9)

$ context-cli search corpus "own fo"
found in host #812 "brown fox" at [2, 8)

$ context-cli search corpus "fox jumps over the lazy cat"
partial: matched "fox jumps over the lazy " (24/27 chars) in host #1290; full query not in graph

$ context-cli search corpus "brown fox" --view occurrences --width 15
3 occurrences in 2 inputs
  [1] notes.txt:4:11   …the quick [brown fox] jumps over…
  [1] notes.txt:9:2    …a [brown fox] and a gr…
  [2] <stdin>:1:5      …the [brown fox] is fast…

$ context-cli search corpus "brown fox" --view contexts --limit 3
3 contexts (narrowest first)
  #901  width 15  @4   "the [brown fox] i"…
  #1004 width 20  @10  "…the quick [brown fox] jumps"
  #1290 width 64  @10  "…the quick [brown fox] jumps over…"

$ context-cli search corpus "brown fox" --view path
host #812 "brown fox"  coverage EntireRoot  span [0, 9)  query [0, 9)
start: #812 ─@0→ #95 "brown" ─@0→ #7 "b"
end:   #812 ─@6→ #301 "fox" ─@2→ #20 "x"
```

`show --format dot`:

```dot
digraph corpus {
  rankdir=TB; node [fontname="monospace"];
  v5 [label="abc"]; v3 [label="ab"]; v0 [label="a", shape=box];
  v5 -> v3 [label="@0"]; v3 -> v0 [label="@0"]; ...
}
```

`show --format mermaid`:

```text
graph TD
  v5["abc"] -->|0| v3["ab"]
  v3 -->|0| v0["a"]
```

Gate refusal (exit 1):

```text
$ context-cli show corpus --format dot
error: graph too large to render: 18 304 vertices, 2 917 551 label chars (limit 64 vertices / 4 096 chars)
hint: render a subgraph with --root "<text>" [--depth 2], or pass --force
```

## 2.4 Code layout

```text
tools/cli/context-cli/src/
  main.rs          clap root, global flags, dispatch to backend
  args/            one module per command group: graph.rs, input.rs, search.rs, show.rs
  input.rs         INPUT resolution: args/files/dirs/stdin → Vec<Input::Text{label}>
  backend.rs       trait Backend + LocalBackend (stage 4 adds RemoteBackend)
  output/          text renderers per result type; json passthrough
  progress.rs      stderr progress from read events
```

Input resolution reads files on the CLI side and always sends
`Input::Text` with the path as label, so local and remote execution behave
the same and servers never need file access (README risk table).

## 2.5 Implementation steps

1. Replace `Cli`/`CliCommand` with the grammar above; keep `--trace` behind
   feature `logs`.
2. `input.rs`: ordering, `-` and implicit stdin, directory walk
   (`--recursive`, `--glob`), UTF-8 validation with path in the error,
   `--split`.
3. `LocalBackend` wraps `context_api::Session`; one-shot commands open the
   graph in the mode the command needs (shared for queries, exclusive for
   `read`/`create`/`import`/`delete`).
4. Text renderers; `--format json` prints `serde_json::to_string(&Response)`
   (pretty when stdout is a TTY).
5. `show -o FILE` writes atomically; format inferred from extension
   (`.dot`, `.mmd`, `.md` wraps mermaid in a fenced block, `.json`).
6. Remove legacy tests and `context-trace/legacy`, `ngrams` dev-deps
   (oracle tests live in context-api).

## 2.6 Tests

Integration tests run the real binary (`assert_cmd` + `predicates`,
`tempfile` store) — exercising argument parsing, stdin, exit codes:

| Test | Asserts |
|---|---|
| `new_then_info` | empty graph, `vertex_count 0`, file exists, lock released after exit |
| `create_from_args_files_stdin` | inputs from `TEXT`, `-f`, `-` in order; input log labels; graph `diff`-equal to `Create` via API |
| `read_appends_and_saves_atomically` | second `read` adds inputs; a failing input (invalid UTF-8 file) leaves the file byte-identical |
| `read_missing_graph_hint` | exit 1, hint mentions `--create` |
| `split_lines` | `--split lines` creates one input per line |
| `search_views` | each view in text and JSON; JSON parses as `Response`; `--fail-if-missing` exit code |
| `search_matches_grep` | `occurrences` count equals `grep -o`-style brute force on the fixture files |
| `show_formats` | dot parses (`dot -Tsvg` if available, else structural check), mermaid starts with `graph TD`, ascii non-empty |
| `show_gate` | refusal + hint; `--root` subgraph succeeds; `--force` succeeds |
| `export_import_v3` | export → import under another name → `diff` empty; fixture snapshot import works |
| `locked_graph` | while a test holds the exclusive lock, `read` fails with `Locked` (exit 1) |
| `legacy_workspace_rejected` | a `.context-engine/<name>/metadata.json` dir yields the legacy hint |

```bash
cargo test -p context-cli
# manual smoke
cargo run -p context-cli -- create demo "abcabca" "bcab"
cargo run -p context-cli -- search demo bca --view occurrences
cargo run -p context-cli -- show demo --format mermaid
```
