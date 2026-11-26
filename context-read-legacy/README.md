# Context-Read-Legacy

Row-model reader of context-read: reads input sequences into the pattern-row
`Hypergraph` of [`context-trace-legacy`](../context-trace-legacy/README.md),
using [`context-search-legacy`](../context-search-legacy/README.md) and
[`context-insert-legacy`](../context-insert-legacy/README.md). Extracted from
the former `src/legacy` module of context-read. Public API:
`context_read_legacy::{read, ReadCtx, ReadSequenceIter, SegmentResult,
IntoReadInput}`. The positioned reader is `context_read::read` in
[`context-read`](../context-read/README.md).

**Frozen:** no new code here; only fixes needed to keep the legacy tests
green. New (positioned-model) code goes into `context-read` and must not
depend on this crate.

**Legacy tests:** every legacy test is
`#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]`, so a
plain `cargo test` skips them. Run the legacy suite with
`cargo test -p context-read-legacy --features legacy-tests,test-api` (the
feature propagates to the lower legacy crates) whenever a change touches a
legacy crate. Do not use `-- --ignored` for this: it also runs slow sweeps.
The 6 known failures in `legacy_reader_behavior` keep their unconditional
`#[ignore = "legacy known failure: …"]`, so the legacy suite runs green.

## Structure
- **`pipeline/`**: Reading and accumulation
  - `mod.rs`: Read context and segment orchestration
  - `root.rs`: Ownership-safe accumulation sink
- **`expansion/`**: Graph expansion operations
  - `mod.rs`: Root-first postfix search and forward steps
  - `closure.rs`: Closure bound/policy selection and sink integration
    (the closure executor is `context_insert_legacy::Overlap`)
  - `block.rs`: Immediate commit orchestration
- **`input.rs`**: Input conversion
- **`segment.rs`**: Known/unknown atom block iteration
- **`request.rs`**: `ReadRequest` (explicit read requests)
- **`tests/`**: `cursor.rs` mechanism contracts (below)

## Usage
```rust
use context_trace_legacy::HypergraphRef;

let graph = HypergraphRef::default();
let root = context_read_legacy::read(&graph, "aabbaabb".chars()).unwrap();
```

## Block expansion and accumulation

See [Block expansion documentation](../context-read/docs/README.md) for notation, examples,
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
expansion by `context_insert_legacy::Overlap::close`:
`Ci = [Si,T] = [Li,C(i-1)]`, `C0 = E`; the final binary `Ck` is committed.
Complements `Li` and `T` are read from the frames' siblings (a single sibling
is used directly) or resolved by scope-restricted insertion, which rewrites
the scope's decompositions; remaining frames are translated through each
resolution's publication.
Whole-child overlaps can replace the tail directly. Self-overlap freezes the
root and uses whole-root closure, preventing cycles.

## Tests and oracle fixtures

Legacy reader behavior tests live in [`tests/legacy_reader_behavior.rs`](tests/legacy_reader_behavior.rs).
They call only the public `context_read_legacy::read` API and compare the **entire**
graph with checked-in n-gram oracle snapshots: tokens and decompositions must
match exactly. Extra structure is a failure, as are duplicate-content vertices,
invalid widths, and decompositions with inconsistent content. Vertex IDs,
pattern IDs, and map iteration order are not semantic.

| Lane | Manifest | Fixtures | Format | Used by |
|---|---|---|---|---|
| Legacy v2 rows | [`tests/fixtures/v2/scenarios.json`](tests/fixtures/v2/scenarios.json) (frozen) | `tests/fixtures/v2/<name>.json` | `format_version: 2`, row decompositions (`ngrams::output::legacy`) | `legacy_reader_behavior`, cursor tests, graph seeding |

`tests/fixture_validation.rs` checks the v2 lane against its own manifest;
`tests/support/` holds the v2 loader (`legacy.rs`) and manifest loading
(`mod.rs`). The positioned v3 lane is documented in
[`context-read`](../context-read/README.md#tests-and-oracle-fixtures).

### Legacy v2 fixtures (frozen)

The v2 files were generated by the legacy row-based oracle (now
`ngrams::graph::parse_corpus_legacy`). The `ngrams` CLI emits **only v3** now,
so v2 fixtures are frozen and **cannot be regenerated with the current CLI**.
Do not point the CLI at `tests/fixtures/v2/`: it would overwrite them with v3
documents. The v2 manifest is a frozen copy of the manifest at the time of the
split; new scenarios go into the v3 manifest only. They remain until the
reader migrates to positioned covers.

Legacy reader tests read all inputs in order and compare only the final graph
against the v2 fixture. The three `closure_*_seed` scenarios provide
independent oracle-backed corpora needed by specialized internal mechanism
tests.

The legacy oracle's partition discovery is known to be order-dependent;
sorting the serialized output does not resolve that issue. The v2 snapshots
record one observed final output per corpus, not a uniquely established
reference graph, so they are not authoritative reader expectations. The
ignored legacy regression
`legacy_partitions_are_independent_of_root_hash_iteration_order` in
`ngrams` exercises both root iteration orders for
`["subdivision", "visualization"]` and exposes this defect.

Specialized cursor, closure-path, scheduling, and sink precondition contracts
live in [`src/tests/cursor.rs`](src/tests/cursor.rs), using oracle-backed graph
seeds. They are **not** alternative specifications of reader graph semantics.
Malformed graphs occur only in explicitly identified fixture-validator tests.
The old compound invariant checker is not a substitute for oracle comparison:
structural policies need independent justification.

```bash
cargo test -p context-read-legacy --features legacy-tests,test-api --test legacy_reader_behavior
cargo test -p context-read-legacy --features legacy-tests,test-api --test fixture_validation
cargo test -p context-read-legacy --features legacy-tests,test-api --lib
```

Oracle mismatches are ordinary failing tests. The only exception is the fixed
list of 6 legacy known failures in `tests/legacy_reader_behavior.rs`, which are
`#[ignore = "legacy known failure: …"]` so the frozen legacy lane gates on
"all non-ignored tests pass".

Criterion bench `grammar`: `cargo bench -p context-read-legacy --bench grammar`.
