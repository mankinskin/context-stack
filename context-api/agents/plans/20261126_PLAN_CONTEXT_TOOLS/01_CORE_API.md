# Stage 1 — Persistence and `context-api` v2

Part of [20261126_PLAN_CONTEXT_TOOLS](README.md). Prerequisite for all
other stages.

## 1.1 What changes

`context-api` is rewritten in place on the positioned stack. Its structure
(command enum → dispatcher → typed results, serde/schemars/ts-rs types,
error enums per domain) is kept; the graph type, the commands and the
storage change.

| Module today | Fate | Notes |
|---|---|---|
| `workspace/{mod,manager,metadata,persistence}.rs` | **rewrite** → `store/` (`GraphStore`, `GraphFile`, `Session`, `OpenGraph`) | single `.ctx` file per graph (D1), lock + atomic save kept |
| `commands/command.rs`, `dispatch.rs`, `result.rs`, `api.rs`, `api_impl.rs` | **rewrite** | new `Command` set (§1.4), `Request`/`Response` envelope |
| `commands/read.rs` | **rewrite** | positioned `read`, input sources, input log |
| `commands/search.rs` | **rewrite** | positioned `find_ancestor`, views (§1.5) |
| `commands/debug.rs`, `ascii_graph.rs` | **port** | vertex view, ASCII renderer input from compositions; add dot/mermaid (§1.6) |
| `commands/export_import.rs` | **rewrite** | v3 interchange JSON + native `.ctx` copy |
| `commands/{atoms,patterns,insert}.rs`, `resolve.rs` | **delete** | read is the only mutation (README §3) |
| `commands/compare.rs` | **rewrite** later (optional) | positioned `diff` instead of legacy label maps |
| `validation.rs` | **rewrite** | `PositionedGraph::validate` + G6 against the input log |
| `logs.rs`, `jq.rs`, `log_parser.rs`, `tracing_capture.rs` | **keep** behind feature `logs` | backend-independent |
| `types.rs` | **shrink/rewrite** | DTOs below |

`Cargo.toml`: algorithm crates with `default-features = false`;
`context-trace` features `serde` (new) and, only for import, `interchange`
(new, see 1.2). `ngrams` moves to dev-dependencies (oracle tests).

## 1.2 `context-trace`: features `serde` and `interchange`

### `serde` (D2)

`PositionedGraph<BaseGraphKind>` stores per slot `index`, `key`, `width`,
`node` (atom or `Composition` of `Occurrence { offset, child: Token }`),
reciprocal `parents`, debug `label`; plus the `atoms` map and `generation`
(`committed/graph.rs:61`, `model/vertex.rs:39`).

Serialized form (private to context-trace, `committed/serde.rs`):

```rust
#[derive(Serialize, Deserialize)]
struct StoredGraph {
    vertices: Vec<StoredNode>,          // slot i = VertexIndex(i)
    generation: u64,
}
#[derive(Serialize, Deserialize)]
enum StoredNode {
    Atom(char),
    Composite(Vec<(usize /*offset*/, usize /*child index*/)>),
}
```

- Only nodes are stored. Width, `parents`, `atoms`, keys and debug labels
  are derived on load.
- `Deserialize for PositionedGraph` goes through
  `TryFrom<StoredGraph>`: allocate all slots, derive widths in
  dependency order (children may have higher indices than parents when
  atoms are committed late, so no index order is assumed; a cycle or a
  missing child is an error), build `Composition::new` (rejects < 2
  occurrences and unsorted/contained occurrences), fill parents, then run
  `validate()` and the store check (`edit::check::validate_store`,
  available internally regardless of `test-api`).
- No public constructor is added; the only entry is serde. This keeps the
  "only insert and read mutate" rule: loading restores an already valid
  graph and rejects anything else.
- Test `positioned/tests/serde.rs`: round trip of every `fixture_graphs()`
  graph is `diff`-equal and index-identical; corrupted tables
  (missing child, wrong offset, overlapping/contained occurrence,
  single-occurrence composite, duplicate atom, cycle) are rejected.

### `interchange`

Move `interchange/convert.rs` (`from_interchange`, `to_interchange`,
`store_interchange`) from `test-api` to a new feature `interchange`
(`test-api` implies it). `from_interchange` builds through `GraphEdit`
inside context-trace; it is import of a validated oracle-format document,
used by `import` only.

## 1.3 Storage: `GraphStore`, `.ctx` file, lock, atomic save

```text
<store>/                       default ./.context-engine/, --store, CONTEXT_STORE
  corpus.ctx                   graph "corpus"
  corpus.ctx.lock              fs2 advisory lock (exclusive while open for writing)
  corpus.ctx.server.json       present while a server holds the graph (stage 3)
```

File layout (`store/file.rs`):

| Part | Content |
|---|---|
| magic | `b"CTXG"` |
| `format_version: u32` | 1 |
| `meta_len: u64` + JSON | `GraphMeta { name, created_at, modified_at, generation, inputs: Vec<InputRecord>, description }` |
| `graph_len: u64` + bincode | `PositionedGraph` (1.2) |
| `crc32: u32` | over all previous bytes |

`InputRecord { id: u32, source: InputSource /*text|file path|stdin|remote*/,
token: usize, width: usize, read_at }` (D3). The text is not stored; it is
reconstructed from `token` by `string_repr`.

Atomic save: write `<file>.tmp.<pid>` → `fsync` → `rename` over `<file>` →
`fsync` directory. A test hook (`cfg(test)` closure between write and
rename) simulates a crash.

Lock policy: opening for write takes the exclusive lock (`try_lock_exclusive`;
on conflict `GraphError::Locked { holder }` where `holder` comes from the
`.server.json` file if present, so the CLI can suggest `--server`, stage 4).
Read-only opens (search, show) take a shared lock.

Legacy detection: `<store>/<name>/metadata.json` exists →
`GraphError::LegacyWorkspace { name }` with the hint "re-create the graph
from its source text with `context-cli create`" (D4).

`GraphStore` API:

```rust
impl GraphStore {
    pub fn new(dir: PathBuf) -> Self;
    pub fn resolve(&self, graph: &GraphRef) -> PathBuf;        // name or *.ctx path
    pub fn create(&self, graph: &GraphRef, overwrite: bool) -> Result<OpenGraph>;
    pub fn open(&self, graph: &GraphRef, mode: OpenMode) -> Result<OpenGraph>;
    pub fn list(&self) -> Result<Vec<GraphSummary>>;            // header + meta only
    pub fn delete(&self, graph: &GraphRef) -> Result<()>;
}
pub struct OpenGraph { path, meta: GraphMeta, graph: PositionedGraphRef,
                       lock: FileLock, dirty: bool, saved_generation: u64 }
impl OpenGraph { pub fn save(&mut self) -> Result<SaveReport>; }
```

`Session` (in-process state shared by every transport) holds a
`GraphStore` and the open graphs:
`HashMap<GraphName, Arc<GraphSlot>>`, `GraphSlot { writer: Mutex<()>,
graph: OpenGraph behind RwLock }` (D8). `Session::execute(Request) ->
Response` is the single entry point.

## 1.4 Command model

`#[serde(tag = "command", rename_all = "snake_case")]`, ts-rs and schemars
derives kept.

```rust
pub struct Request  { pub id: Option<u64>, #[serde(flatten)] pub command: Command }
pub enum   Response { Ok { id: Option<u64>, result: CommandResult },
                      Error { id: Option<u64>, error: ErrorResponse } }

pub enum Command {
    // lifecycle
    New      { graph: GraphRef, #[serde(default)] overwrite: bool },
    List,
    Info     { graph: GraphRef },
    Delete   { graph: GraphRef },
    Save     { graph: GraphRef },                 // server/shell; CLI saves implicitly
    Close    { graph: GraphRef, #[serde(default)] save: bool },
    // the only mutation
    Read     { graph: GraphRef, inputs: Vec<Input>,
               #[serde(default)] split: Split,     // none | lines | paragraphs
               #[serde(default)] create: bool,     // create if missing
               #[serde(default)] save: SavePolicy, // now | defer (server default: per server config)
               #[serde(default)] steps: bool },    // include reader steps in the report
    Create   { graph: GraphRef, inputs: Vec<Input>, #[serde(default)] split: Split,
               #[serde(default)] overwrite: bool }, // New + Read + save
    // queries
    Search   { graph: GraphRef, query: String, #[serde(default)] view: SearchView,
               #[serde(default)] limits: Limits },
    Vertex   { graph: GraphRef, vertex: VertexRef },         // index or text
    Inputs   { graph: GraphRef, #[serde(default)] limits: Limits },
    Stats    { graph: GraphRef },
    Validate { graph: GraphRef },
    Render   { graph: GraphRef, format: RenderFormat, #[serde(default)] root: Option<VertexRef>,
               #[serde(default)] depth: Option<u32>, #[serde(default)] gate: Gate },
    Export   { graph: GraphRef, format: ExportFormat },        // interchange_v3 | native
    Import   { graph: GraphRef, data: ImportData, #[serde(default)] overwrite: bool },
    // logs (feature "logs"): ListLogs, GetLog, QueryLog, AnalyzeLog, SearchLogs, DeleteLog(s) unchanged
}

pub enum Input { Text { text: String, #[serde(default)] label: Option<String> },
                 File { path: String } }          // File refused by servers without --allow-fs
```

`Stdin` is not a wire input: the CLI reads stdin and sends `Text` with
label `"<stdin>"`.

Read semantics (`commands/read.rs`):

1. Resolve inputs into `(label, String)` items; invalid UTF-8 → error
   before any mutation (all-or-nothing input validation).
2. Apply `split` (`lines`: one input per non-empty line; `paragraphs`:
   blank-line separated) — every item is one reader input, i.e. one
   corpus text.
3. For each item: `context_read::read(&graph, text.chars())` (or
   `read_traced` when `steps` is requested); append an `InputRecord`;
   empty item → skipped with a warning. Live progress (CLI progress bar,
   server events) needs a per-step callback: `read_steps` is
   `pub(crate)` (`context-read/src/read.rs:95`), so add a public
   `context_read::read_observed(graph, input, impl FnMut(&Step))` that
   forwards the committed `Step` (no `StepView` internals exposed).
4. Save per `save` policy. In the one-shot CLI the whole command is one
   transaction: an error in any item means nothing is saved (the in-memory
   graph is dropped).

`ReadReport { graph, inputs: [{id, label, token: TokenInfo, chars,
new_vertices}], vertex_count_before, vertex_count_after, saved:
Option<SaveReport>, steps: Option<Vec<StepInfo>> }`.

## 1.5 Search and its views

`commands/search.rs`, using `graph.find_ancestor(query.chars())`.

Outcomes:

| Case | Condition | Result |
|---|---|---|
| `UnknownAtom` | a query char never read | `found: none`, `unknown_chars` listed |
| complete vertex | `query_exhausted && is_entire_root` | the query is a vertex: `token` |
| complete in host | `query_exhausted && !is_entire_root` | narrowest host `root_token`, `span` |
| partial | `!query_exhausted` | longest matched query prefix `[query_start, checkpoint)`, host and span; the full query occurs nowhere (to be confirmed by the completeness test, 1.8) |

`SearchView` selects the payload (all views include the outcome above):

- `summary` (default): outcome, host `TokenInfo { index, width, label }`.
- `path`: serialized `Response`: coverage kind (`EntireRoot`, `Prefix`,
  `Postfix`, `Range`), `span`, `query_start`, `checkpoint_position`,
  start/end paths as `[{ parent, offset, child }]` top-down, generation.
  DTOs built in context-api from the public `PathCoverage`/`RolePath`
  accessors; if an accessor is missing, add a read-only accessor in
  context-search (no state change).
- `contexts`: the distinct **ancestors** containing the match, each with the
  absolute offset(s) of the match inside it and a snippet
  `left … [match] … right` (`Limits.context_width` chars per side), ordered
  by ancestor width (narrowest contexts first). This is context
  exploration: "in which larger units does this occur".
- `occurrences`: positions of the query in the **inputs** (input log
  tokens): `[{ input id, label, char offset, line, column, snippet }]`
  (concordance / KWIC). This is text search.
- `parents`: only direct parents with offsets (cheap, for navigation UIs).

Enumeration (shared by `contexts` and `occurrences`):

```text
start = (host, span.start)            // or (token, 0) for a complete vertex
seen  = {start}; queue = [start]
while (v, off) = pop(queue):
    for (parent, o) in v.parent_occurrences():
        item = (parent, o + off)
        if seen.insert(item): push(queue, item)   // dedup: the same absolute
                                                  // position is reachable through
                                                  // overlapping children
```

Limits: `limit` (default 50 items), `max_depth`, `max_visited` (hard work
budget, returns `truncated: true`). Counts reported (`total` when not
truncated). Never sum path counts — overlapping children reach the same
position through several paths.

Assumption (to be confirmed by test 3 in 1.8, before the views are
documented): when the query is not a vertex, every occurrence of it lies
inside an occurrence of the narrowest host at the same relative offset. The
oracle retains a substring that occurs in two different contexts, so a
non-vertex query has a single context. If the test finds a counterexample,
`occurrences` falls back to a per-input scan from the input tokens
(`start_at` descents), and the plan is revised.

Snippets: compute `left`/`right` by reading the ancestor's interval
`[off − w, off + len + w)` through `start_at` descent, not by
`string_repr` of the whole ancestor (inputs can be megabytes). Implement as
an api-side helper over `Navigate::start_at` / compositions (the
`read_interval` helper is debug/test-only by design).

## 1.6 Render and the gate

`RenderFormat = ascii | ascii_dag | dot | mermaid | json` (json = the
rendered subgraph as `{vertices:[{index,label,width}], edges:[{parent,
child, offset}]}` for viewers).

Subgraph: all vertices, or the descendants of `root` (optionally limited by
`depth`). Gate (D7) is checked **before** any label is built:

```rust
pub struct Gate { pub max_vertices: Option<usize>, pub max_label_chars: Option<usize>, pub force: bool }
```

- defaults 64 vertices, 4 096 label chars (label chars = Σ widths of the
  subgraph, known without expansion);
- `force` lifts the defaults; transports may set a hard cap that `force`
  cannot exceed (`Session::with_render_cap`);
- refusal: `RenderError::TooLarge { vertices, label_chars, limit, hint }`
  with a hint to use `--root`, `--depth` or `--force`.

Output:

- dot: `digraph { rankdir=TB; v12 [label="abc"]; v12 -> v3 [label="@0"]; }`,
  atoms `shape=box`, labels escaped and truncated to `label_width`;
- mermaid: `graph TD` + `v12["abc"] -->|0| v3["ab"]`, escaped (`"` → `#quot;`);
- ascii: port the input builder of `render_layered_ascii_owned_with_mode`
  (layers by width, top = widest) to compositions; renderer unchanged.

## 1.7 Other commands

- `Info`: `GraphSummary { name, path, vertex_count, atom_count,
  composite_count, inputs, total_input_chars, generation, created_at,
  modified_at, file_size, dirty }`.
- `Stats`: plus width histogram, max width, parent-count histogram, average
  composition size.
- `Vertex`: index or text (`VertexRef::Text` resolved by search with
  `complete vertex` outcome, else error "not a vertex; host is …"): label,
  width, composition `[(offset, child)]`, parents `[(parent, offsets)]`.
- `Validate`: `PositionedGraph::validate` + G6 (every composite not an
  input token has ≥ 2 parent occurrences) using the input log; report, never
  panic.
- `Export`: `interchange_v3` → `PositionedGraphSnapshot { corpus: input
  texts, graph: store_interchange }` (gated like render, since it is
  content-keyed: Θ(Σ widths)); `native` → the `.ctx` bytes (base64 in JSON
  transports).
- `Import`: `interchange_v3` → validate snapshot → `from_interchange` → input
  log from `corpus` (token found by search) → save; `native` → load
  (validates) → save under the new name.

## 1.8 Tests (stage gate)

All in `context-api` (`src/tests/`), using `tempfile` stores.

1. **Round trip:** for every `fixture_snapshots()` corpus: `Create` with
   the corpus texts → `diff` against `fixture_graphs()` graph is empty →
   reopen from disk → still empty `diff`, same indices, same input log.
2. **Oracle consistency:** random small corpora (alphabet 2–4, length ≤ 12,
   as in the exhaustive reader tests): graph from `Read` equals
   `ngrams::graph::parse_corpus` output (`diff`) — only for corpora the
   reader gate already covers; deviations are reader bugs, not API bugs.
3. **Search completeness:** for each corpus and every substring query plus
   random non-substrings: `occurrences` equals brute force
   (`text.match_indices` with overlaps) per input; partial/unknown outcomes
   exactly when no input contains the query.
4. **Contexts:** every reported `(ancestor, offset)` satisfies
   `label(ancestor)[offset..offset+len] == query`; no duplicates;
   truncation flag set when `max_visited` is hit.
5. **Atomic save:** crash hook between write and rename → old file loads;
   corrupted CRC / truncated file / wrong magic → typed errors.
6. **Lock:** second exclusive open fails with `Locked`; shared opens
   coexist.
7. **Gate:** render of a graph above the limit fails with `TooLarge`;
   `--root` subgraph below it succeeds; `force` respects the hard cap.
8. **Serde/ts:** `Command`/`Response` JSON round trip for every variant
   (port `tests/command_serde.rs`, `result_serde.rs`).
9. **No legacy:** `cargo tree -p context-api -e features -i context-trace`
   shows no `legacy` feature.

```bash
cargo test -p context-trace --features serde,interchange positioned::tests::serde
cargo test -p context-api
cargo check -p context-api --no-default-features
```
