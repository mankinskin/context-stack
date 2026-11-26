# context-cli

Command-line tool for context-engine graphs on the positioned stack. Every
command builds a `context_api::Request` and runs it on an in-process
`Session` (`LocalBackend`); `--format json` prints the `Response` envelope
unchanged, so scripts see the same JSON as the (future) server and MCP
transports.

Plan: `context-api/agents/plans/20261126_PLAN_CONTEXT_TOOLS/02_CLI.md`.

## Usage

```text
context-cli [GLOBAL] <COMMAND>

GLOBAL
  --store <DIR>          graph store (default ./.context-engine, env CONTEXT_STORE)
  --server <URL>         client mode (stage 4; currently an error), env CONTEXT_SERVER
  --local                run in process even if CONTEXT_SERVER is set
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

Not yet available: `serve` (stage 3), `--server` client mode and `shell`
(stage 4), `mcp` (stage 5).

## Tests

```bash
cargo test -p context-cli
```

Integration tests (`tests/`) run the binary against a temporary store with
`assert_cmd`.
