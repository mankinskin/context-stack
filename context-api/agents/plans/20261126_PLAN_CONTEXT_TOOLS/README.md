# Plan: context tools on the positioned stack (CLI, server, client, MCP)

**Date:** 2026-11-26 · **Status:** 📋 ready, decisions D1–D8 accepted (2026-11-26) ·
**Scope:** `context-api`, `tools/cli/context-cli`, `tools/http/context-http`,
`tools/mcp/context-mcp`, one serde feature in `context-trace`.

The positioned reader (`context_read::read`) is complete. This plan builds
the user-facing tool surface on it and retires the legacy row-model backend
from all tools (roadmap step 6, "Migrate `context-api` … HTTP/MCP
serialization", in
[20261120_PLAN_POSITIONED_READER_OPEN_WORK](../../../../context-read/agents/plans/20261120_PLAN_POSITIONED_READER_OPEN_WORK.md)).

## Files

| File | Content |
|---|---|
| this README | goals, current state, target architecture, decisions, stage overview, verification |
| [01_CORE_API.md](01_CORE_API.md) | Stage 1: graph persistence and the `context-api` rewrite (command model, read, search, contexts, render) |
| [02_CLI.md](02_CLI.md) | Stage 2: standalone CLI (graph commands, input sources, output formats, render gate) |
| [03_SERVER.md](03_SERVER.md) | Stage 3: HTTP + WebSocket server |
| [04_CLIENT_SHELL.md](04_CLIENT_SHELL.md) | Stage 4: client mode and interactive shell |
| [05_MCP.md](05_MCP.md) | Stage 5: MCP server (local and proxy mode) |
| [06_EXAMPLES.md](06_EXAMPLES.md) | User applications and tool usage, including out-of-scope features |

## 1. Goals

The user must be able to:

1. create a new empty graph;
2. load a graph, read text from CLI arguments, files or stdin into it, and
   save it atomically;
3. draw a graph (ASCII) or export it as Graphviz dot or Mermaid, with a size
   gate for large graphs;
4. query a graph with a string (search) and output the search response, the
   contexts or the occurrences of the query, in text or JSON;
5. create a new graph from input (new + read in one command);
6. host a server (HTTP and WebSocket) that executes the same commands;
7. act as a client of that server (one-shot commands and an interactive
   shell), so a background process keeps the graph in memory; the same
   protocol later backs a visual viewer;
8. serve the same interface to agents over MCP.

Main use-case directions: **text indexing**, **context exploration**,
**text search**. Reconstructing likely contexts (completion, prediction) is
out of scope (examples in [06_EXAMPLES.md](06_EXAMPLES.md) §4).

## 2. What exists today

All four tool crates compile (`cargo check -p context-api -p context-cli
-p context-mcp -p context-http --all-targets`: 0 errors, 2026-11-26), but
**every graph operation runs on the legacy row model**.

| Crate | What it provides | Backend dependency |
|---|---|---|
| `context-api` (≈13 k lines) | `Command` enum (≈45 variants, `#[serde(tag = "command")]`, `command.rs`), `execute` dispatcher (`commands/dispatch.rs`), `WorkspaceManager` with named workspaces under `.context-engine/<name>/` (`graph.bin` bincode, `metadata.json`, fs2 `.lock`, atomic tmp+rename, `workspace/persistence.rs`), atoms/patterns/insert/search/read commands, ASCII layered renderer (`ascii_graph.rs`), JSON/bincode export/import, graph compare, per-command tracing capture and JQ log queries (`tracing_capture.rs`, `logs.rs`, `jq.rs`), ts-rs bindings into `packages/context-types` | `context-trace/legacy` `Hypergraph<BaseGraphKind>`, `context-read::legacy`, `ngrams::parse_corpus_legacy` (`workspace/manager.rs:41`) |
| `context-cli` | clap subcommands mirroring `Command` 1:1 (`src/main.rs`), rustyline REPL (`src/repl.rs`), text/JSON output (`src/output.rs`), integration tests incl. legacy oracle comparisons | `context-api` only (tests: legacy trace + ngrams) |
| `context-mcp` | rmcp 0.14 stdio server with three tools: `execute` (any `Command` JSON), `help`, `workflow` (`src/server.rs`) | `context-api` |
| `context-http` | axum 0.8: `POST /api/execute`, REST convenience routes, log REST routes, optional GraphQL, permissive CORS, static file fallback (`src/router.rs`) | `context-api` |

Missing entirely:

- persistence of a `PositionedGraph` (it has no serde impl; the only
  serialized form is the content-keyed v3 interchange
  `PositionedGraphSnapshot`, whose conversion `from_interchange` /
  `to_interchange` is gated behind `test-api`,
  `context-trace/src/positioned/interchange/mod.rs:13`);
- positioned search output (path DTOs), context and occurrence enumeration;
- dot and Mermaid export, size gate;
- WebSocket transport, event stream, client mode, server discovery;
- typed MCP tools.

Not usable as-is:

- `context-editor` kernel/sandbox-app have no graph protocol (unrelated
  Bevy/Dioxus scaffold); the future viewer will consume the WebSocket
  protocol from stage 3.
- Visualization events (`GraphOpEvent`) exist only in legacy search/insert;
  the positioned plans dropped them. The only positioned progress hook is
  the reader's step callback (`read_steps`, `context-read/src/read.rs`).

## 3. Positioned API the tools build on

| Need | API (all non-legacy) |
|---|---|
| empty graph | `PositionedGraph::default()`, shared as `PositionedGraphRef::new(g)` (`context-trace/src/positioned/committed/graph.rs`) |
| read | `context_read::read(&PositionedGraphRef, impl IntoIterator<Item = char>) -> Option<Token>`; `read_traced` also returns `Vec<Step>` (`context-read/src/read.rs:41`) |
| search | `graph.find_ancestor(text.chars()) -> Result<Response, SearchError>` (`context-search/src/search/mod.rs:63`, `searchable.rs:65`); unknown atom → `SearchError::UnknownAtom` |
| response | `Response { cache, end }`: `root_token`, `path() -> &PathCoverage`, `query_exhausted`, `is_entire_root`, `span`, `query_start`, `checkpoint_position`, `anchor`, `generation` (`context-search/src/state/response.rs:42`) |
| contexts | `Navigate::parent_occurrences() -> (parent Token, AtomPosition)` sorted by width; `prefixes`, `postfixes`, `start_at` (`context-trace/src/positioned/store/navigate.rs:116`) |
| labels | `string_repr(vertex)` (O(width)) |
| interchange | `PositionedGraphSnapshot` / `PositionedGraphData` (serde, v3), `store_interchange` |
| oracle | `ngrams::graph::parse_corpus` → positioned `PositionedGraphData` (`ngrams/src/graph/mod.rs:252`) |
| test fixtures | `context_trace::positioned::fixtures::{fixture_snapshots, fixture_graphs}` (`test-api`) |

Binding rules from the stack that constrain the tools:

- **`read` is the only public mutation.** Insert is an internal reader
  building block; the tools expose no insert, add-atom or add-pattern
  command. Only `read` inputs are inputs.
- **Only insert and read mutate the graph**; the `edit` feature is
  reserved for context-insert and context-read (guard test
  `context-trace/src/positioned/tests/guard.rs`). Loading a saved graph
  must therefore not use a general graph-building API (decision D2).
- **New code must not use `legacy`** and must pass with the algorithm
  crates' default features off.

## 4. Target architecture

```text
             ┌──────────────── context-cli (bin) ─────────────────┐
 user ──────►│ one-shot cmds │ shell (REPL) │ serve │ mcp          │
             └──────┬────────┴──────┬───────┴───┬───┴──┬──────────┘
                    │ Backend trait │           │      │
          ┌─────────┴───┐   ┌───────┴──────┐    │      │
          │ LocalBackend│   │RemoteBackend │    │      │
          │ (in-process)│   │ (HTTP / WS)  │────┼──────┼──► running server
          └──────┬──────┘   └──────────────┘    │      │
                 │                    context-http (lib)  context-mcp (lib)
                 │                    axum: /v1/execute,   rmcp tools, local
                 │                    /v1/ws, REST          or proxy backend
                 ▼                         │                  │
        ┌───────────────────── context-api ─────────────────────────┐
        │ Request/Response, Command, GraphStore (open/lock/save),   │
        │ Session (open graphs), read, search, contexts, render     │
        └───────┬──────────────┬───────────────┬────────────────────┘
          context-read    context-search   context-trace (+ serde feature)
```

- `context-api` is the only crate that touches the algorithm crates. It
  exposes one `Session::execute(Request) -> Response` used by every
  transport, so CLI, server, client, shell and MCP return identical JSON.
- `context-http` and `context-mcp` become libraries (their current bins stay
  as thin wrappers). `context-cli` links them behind default-on features
  `server` and `mcp`, so one binary provides every mode.
- A `Backend` trait (`execute(Request) -> Response`, `subscribe(...)`) has a
  local and a remote implementation; the shell and MCP choose one by
  `--server`.

## 5. Decisions (accepted 2026-11-26)

| ID | Decision | Proposal | Alternatives |
|---|---|---|---|
| D1 | Graph storage unit | **One file per graph** `<name>.ctx` (header + metadata + vertex table) with sibling `<name>.ctx.lock`; graphs addressed by name in a store dir (default `./.context-engine/`, `--store`, env `CONTEXT_STORE`) or by a path ending in `.ctx`. One file makes the save a single atomic rename. | Keep the workspace directory (`graph.bin` + `metadata.json`): two files cannot be replaced atomically together. |
| D2 | Persistence format | **Index-preserving binary**: `Serialize`/`Deserialize` for `PositionedGraph` behind a new `context-trace` feature `serde`, implemented inside context-trace; deserialization rebuilds the store and runs full validation (`validate`, `validate_store`), never exposing a builder. Encoded with bincode 1 (already a dependency). | (a) v3 interchange JSON as the storage format: content-keyed, size Θ(Σ widths), loses indices; keep it for export/import only. (b) Store the corpus and replay `read` on load: slow, defeats the server's purpose. |
| D3 | Input log | Graph metadata keeps an **input log** `[{id, source, token, width, chars, read_at}]` (application metadata, not a graph mark; the dissolution plan has no stored input mark). Needed for occurrences per input and for the interchange `corpus`. | No log: occurrences can only be reported per root vertex. |
| D4 | Legacy commands | **Drop** atoms/patterns/insert/`search_pattern`/`create_workspace_from_ngrams`/legacy compare; keep log commands (file-based, backend-independent) behind a `logs` feature. Old `.context-engine/<name>/graph.bin` workspaces are detected and rejected with a hint (no row→positioned migration; re-read the source text). | Keep a `legacy-api` feature: doubles every command. |
| D5 | Binary | Keep the name `context-cli`; `serve`, `shell`, `mcp` are subcommands. `context-http` / `context-mcp` bins remain as wrappers. | Rename to `context` / `ctx`. |
| D6 | Wire protocol | One JSON envelope `{id, command}` → `{id, ok}` / `{id, error}` for HTTP `POST /v1/execute`, WebSocket text frames and MCP proxying; server events `{event}` on WebSocket only. Versioned path prefix `/v1`. | GraphQL (existing optional feature): dropped for now. |
| D7 | Render gate | Default limit 64 vertices and 4 096 label characters, applied to the rendered subgraph (`--root`); `--force` lifts the default limit locally; server/MCP enforce a configurable hard cap that `--force` cannot exceed. | Unlimited with warning. |
| D8 | Concurrency | Server: per-graph single writer (reads are serialized per graph, run in `spawn_blocking`), searches concurrent through `PositionedGraphRef` read locks; exclusive file lock while a process has the graph open for writing. CLI one-shot read: lock → load → read → save → unlock. | Multi-writer: not supported by the reader. |

## 6. Stages

| Stage | File | Deliverable | Depends on |
|---|---|---|---|
| 1 | [01_CORE_API](01_CORE_API.md) | `context-trace` `serde` feature; `context-api` v2: `GraphStore`, `Session`, `Command`/`Request`/`Response`, read, search (summary, path, contexts, occurrences), render (ascii, dot, mermaid) with gate, export/import (v3 JSON), info/stats/validate/vertex; legacy dependency removed | — |
| 2 | [02_CLI](02_CLI.md) | `context-cli` v2: `new`, `read`, `create`, `search`, `show`, `export`, `import`, `info`, `list`, `delete`, `validate`, `vertex`, `inputs`; input sources; `--format text|json` | 1 |
| 3 | [03_SERVER](03_SERVER.md) | `context-http` v2 library + `context-cli serve`: `/v1/execute`, `/v1/ws`, REST reads, autosave, discovery file, auth, panic isolation, event stream (graph changed, read progress) | 1 |
| 4 | [04_CLIENT_SHELL](04_CLIENT_SHELL.md) | `Backend` trait, `--server` for every command, server auto-discovery, `context-cli shell` (local or remote) | 2, 3 |
| 5 | [05_MCP](05_MCP.md) | `context-mcp` v2: typed tools, local or proxy backend, output budgets | 1 (proxy: 4) |
| 6 | this README §8 | READMEs, ts-rs bindings regenerated, roadmap step 6 items ticked | 1–5 |

Stages 2, 3 and 5 can proceed in parallel after stage 1.

## 7. Verification overview

Per stage the stage file lists exact tests. Gates across all stages:

```bash
# no legacy backend in the tools
cargo tree -p context-cli -e normal,features | grep -c legacy   # expect 0 (dev-only ngrams may use legacy)
cargo test -p context-api -p context-cli -p context-http -p context-mcp
# downstream consumers (winit does not build in the Linux codespace)
cargo build --workspace --all-targets --exclude context-editor-kernel --exclude sandbox-app
```

Correctness anchors (independent of the implementation under test):

- **Oracle consistency:** a graph built with `context-cli create` from a
  fixture corpus equals the oracle graph (`fixture_graphs`, or
  `ngrams::graph::parse_corpus`) under `positioned::diff`, before and after
  a save/load round trip.
- **Search completeness:** for random corpora and queries, the
  `occurrences` view equals brute-force `str::match_indices` over the input
  texts (overlapping matches included), and a query is reported as
  not found exactly when no input contains it.
- **Transport equivalence:** the same request script produces
  byte-identical `Response` JSON through local CLI, HTTP, WebSocket, client
  CLI and MCP proxy.
- **Atomicity:** killing the process during a save (fault-injection test
  hook between write and rename) leaves the previous file loadable.

## 8. Stage 6: documentation and cleanup

- Rewrite `context-api/README.md` (it still describes atoms/patterns and
  references a nonexistent `agents/plans/20260310_PLAN_CONTEXT_API_OVERVIEW.md`),
  `tools/http/context-http/README.md`, `tools/mcp/context-mcp/README.md`;
  add `tools/cli/context-cli/README.md` with the examples of
  [06_EXAMPLES.md](06_EXAMPLES.md).
- Regenerate `packages/context-types` with `cargo test -p context-api
  --features ts-gen`; delete stale generated legacy types.
- Tick roadmap step 6 consumer-migration items in
  `context-read/agents/plans/20261120_PLAN_POSITIONED_READER_OPEN_WORK.md`.

## 9. Risks

| Risk | Mitigation |
|---|---|
| Deserialization becomes a back door for arbitrary graph construction | Implemented inside context-trace, full validation on load, no public builder; fuzz/corruption tests reject invalid tables |
| Context enumeration explodes on large graphs (an occurrence is reachable through several overlapping children) | Deduplicate by `(ancestor, absolute offset)`; limits (`--limit`, `--max-depth`) and counts computed by enumeration, not by summing paths (see 01 §5) |
| Labels are O(width) (`string_repr`) | Truncated labels (`--label-width`), snippets built by `Navigate::start_at` descent over the needed range only |
| Remote file inputs leak server files | Server never reads paths unless started with `--allow-fs <dir>`; the client reads files locally and sends text |
| A reader panic corrupts a server-held graph | `catch_unwind` per mutating command; poisoned graph reloaded from its last save; error returned |
| Large inputs block the server | `spawn_blocking`, streamed `text/plain` read endpoint, progress events, body size limit |
