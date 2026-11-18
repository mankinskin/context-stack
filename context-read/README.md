# Context-Read

Graph reading and expansion operations for the context framework.
Provides ordered, recursive hypergraph operations for sequenced tokenized
data with structural overlap closure and ownership-safe accumulation.

## Features
- Ordered recursive hypergraph operations
- Sequenced tokenized data handling
- Graph complement operations
- Root-postfix expansion with immediate commits
- Block iteration for sequence processing
- Context-aware reading operations

## Structure
- **`pipeline/`**: Reading and accumulation
  - `mod.rs`: Read context and segment orchestration
  - `root.rs`: Ownership-safe accumulation sink
- **`expansion/`**: Graph expansion operations
  - `mod.rs`: Root-first postfix search and forward steps
  - `closure.rs`: Closure bound/policy selection and sink integration
    (the closure executor is `context_insert::Overlap`)
  - `block.rs`: Immediate commit orchestration
- **`input.rs`**: Input conversion
- **`segment.rs`**: Known/unknown atom block iteration

## Usage
```rust
use context_trace::HypergraphRef;

let graph = HypergraphRef::default();
let root = context_read::read(&graph, "aabbaabb".chars()).unwrap();
```

## Key Concepts
- **Ordered Hypergraph**: Recursive graph structure for sequenced data
- **Root Anchor**: Structural postfixes of the latest committed root supply overlaps
- **Block Iteration**: Efficient sequence processing in blocks
- **Complement Operations**: Graph complement calculations

### Block expansion and accumulation

See [Block expansion documentation](docs/README.md) for notation, examples,
graph changes, and algorithm responsibilities.

`ExpansionCtx` searches eligible postfix tokens of the latest committed root,
largest-first, for an expansion into the remaining known block. Candidate
searches are read-only; the accepted probe response is materialized directly,
without searching its query again, and the traversal's frames to the shared
token are translated through the materialization's publication. Without an
extending postfix, expansion consumes the largest forward match.

`BlockExpansionCtx` immediately commits each span through `RootManager::commit`
and feeds the returned `CommitReceipt` back to the expansion. The root manager
accumulates committed input, extending an owned container when safe or
constructing a new root without mutating existing semantic tokens. It applies
`Append`, `ReplaceTail` or `ReplaceRoot`; it never searches or resolves overlaps.
Expansion reuses canonical combined roots where available. Ownership is
independent of known/unknown block
boundaries. A newly allocated accumulation container remains growable until it
is shared or structurally ineligible for extension; adopted tokens are never
modified. Appending the root to itself creates a new container instead of a
self-reference.

The commit seam is the entire committed section of the current block.
It bounds eligible root postfix widths; there is no seam-position scan,
separate local anchor or pending overlap chain.

After materialization, closure recomputes editability. For an editable root it
closes around the entire last child; otherwise around the root. With
`X=L+P` and `E=P+T`, every retained anchor frame `Si` (the widest-postfix
chain at traversal) with `|P| < |Si| <= |X|` is closed bottom-up with the
expansion by `context_insert::Overlap::close`:
`Ci = [Si,T] = [Li,C(i-1)]`, `C0 = E`; the final binary `Ck` is committed.
Complements `Li` and `T` are read from the frames' siblings (a single sibling
is used directly) or resolved by scope-restricted insertion, which rewrites
the scope's decompositions; remaining frames are translated through each
resolution's publication.
Whole-child overlaps can replace the tail directly. Self-overlap freezes the
root and uses whole-root closure, preventing cycles.

## Dependencies
- **context-trace**: Core graph structures and operations
- Standard Rust collections and iterators
- Test framework integration

## Development

### Tests and oracle fixtures

Reader behavior tests live in [`tests/reader_behavior.rs`](tests/reader_behavior.rs).
They call only the public `context_read::read` API and compare the **entire**
graph with checked-in n-gram oracle snapshots: tokens and decompositions must
match exactly. Extra structure is a failure, as are duplicate-content vertices,
invalid widths, and decompositions with inconsistent content. Vertex IDs,
pattern IDs, and map iteration order are not semantic.

[`tests/fixtures/scenarios.json`](tests/fixtures/scenarios.json) defines ordered
input corpora. Each scenario has fixtures in two lanes:

| Lane | Path | Format | Used by |
|---|---|---|---|
| Legacy v2 rows | `tests/fixtures/<name>.json` | `format_version: 2`, row decompositions (`ngrams::output::legacy`) | `reader_behavior`, cursor tests, graph seeding |
| Positioned v3 | `tests/fixtures/v3/<name>.json` | `format_version: 3`, positioned covers (`ngrams::output::PositionedGraphSnapshot`) | `fixture_validation` only, until the reader produces positioned covers |

Each fixture records the corpus and one complete final oracle graph in a
top-level `vertices` object. Reader tests read all inputs in order and compare
only the final graph against the **v2** fixture. Cumulative-prefix snapshots
and oracle-seeded behavioral transition tests have been removed. The three
`closure_*_seed` scenarios provide independent oracle-backed corpora needed by
specialized internal mechanism tests.

#### Legacy v2 fixtures (frozen)

The v2 files were generated by the legacy row-based oracle (now
`ngrams::graph::parse_corpus_legacy`). The `ngrams` CLI emits **only v3** now,
so v2 fixtures are frozen and **cannot be regenerated with the current CLI**.
Do not point the CLI at `tests/fixtures/`: it would overwrite them with v3
documents. They remain until the reader migrates to positioned covers.

The legacy oracle's partition discovery is known to be order-dependent;
sorting the serialized output does not resolve that issue. The v2 snapshots
record one observed final output per corpus, not a uniquely established
reference graph, so they are not authoritative reader expectations. The
ignored legacy regression
`legacy_partitions_are_independent_of_root_hash_iteration_order` in
`ngrams` exercises both root iteration orders for
`["subdivision", "visualization"]` and exposes this defect.

#### Positioned v3 fixtures

The v3 files are generated by the [ngrams CLI](../ngrams/README.md), which
calls `ngrams::graph::parse_corpus` (not the reader). Regenerate them in a
release build (from the workspace root):

```bash
cargo run --release -p ngrams --bin ngrams -- \
  --file context-read/tests/fixtures/scenarios.json \
  --out context-read/tests/fixtures/v3
# Generate one corpus to stdout:
cargo run --release -p ngrams --bin ngrams -- "ab" "bc" "abc"
```

Outputs are sorted, content-keyed JSON and deterministic; regeneration must
produce no diff. Oracle errors abort generation rather than yielding empty or
reader-derived expectations. `fixture_validation` requires exactly one v3
fixture per manifest entry, a corpus equal to the scenario inputs (order and
multiplicity), structural validity, and equality with a fresh `parse_corpus`
run. To add a scenario, add its corpus to the manifest, register its test in
the scenario macro, regenerate v3, and review the JSON diff against hand
expectations. Reader tests also require a v2 fixture for every manifest entry,
which the current CLI cannot produce, so new scenarios effectively wait for the
reader migration. Do not edit expected graph data by hand. Empty and
single-character API contracts are tested directly because the oracle does not
support all such corpora.

Specialized cursor, closure-path, scheduling, and sink precondition contracts
live in [`src/tests/cursor.rs`](src/tests/cursor.rs), using oracle-backed graph
seeds. They are **not** alternative specifications of reader graph semantics.
Malformed graphs occur only in explicitly identified fixture-validator tests.
The old compound invariant checker is not a substitute for oracle comparison:
structural policies need independent justification.

Run the suites separately:

```bash
cargo test -p context-read --test reader_behavior
cargo test -p context-read --test fixture_validation
cargo test -p context-read --lib
```

Oracle mismatches are ordinary failing tests, not ignored tests or permissive
subset comparisons. A passing internal mechanism test does not override a
failing behavioral test.

```bash
cargo test          # Run tests
cargo doc --open    # Generate documentation
```

## Architecture
Ordered recursive hypergraph system with root-postfix expansion,
complement operations, and efficient sequence processing
for tokenized data structures.

![Graph Visualization](https://user-images.githubusercontent.com/20745737/133164477-8d7237d0-2f24-4b6e-9ddb-ceb7a70da43e.png)
*(rendered with [egui](https://github.com/emilk/egui) in [graph_app](https://github.com/mankinskin/graph_app))*