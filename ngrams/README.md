# ngrams

Generate a complete n-gram graph for a corpus of input texts. The library API
remains available; the binary uses the same oracle algorithm.

## CLI

Use a release build: even short corpora can be slow without optimization.

```bash
cargo build --release -p ngrams --bin ngrams
./target/release/ngrams "first text" "second text"
./target/release/ngrams --file scenarios.json --out ../context-read/tests/fixtures/
./target/release/ngrams --help
```

Or install the binary with `cargo install --path ngrams`, then run `ngrams`
directly. Paths are relative to the current working directory.

Inline arguments form **one corpus**, and its graph JSON is written to stdout.
Diagnostics go to stderr, so stdout can be redirected to a file.
Use `--` before positional texts that start with a hyphen.

File mode accepts either a single scenario object or an array:

```json
[
  {"name": "example", "inputs": ["ab", "bc", "abc"]},
  {"name": "other", "inputs": ["xy", "xyz"]}
]
```

`--out` is mandatory with `--file`. The directory is created if needed, and
each scenario writes `<name>.json` there, replacing an existing file with the
same name. Unrelated files are not removed. Names must be unique, nonempty
ASCII filenames containing only letters, digits, `_`, `-`, or `.`, excluding
`.` and `..`. Empty corpora and empty texts are rejected. File and inline modes
cannot be combined.

## JSON format

Each output describes the **entire final graph** for one corpus, not incremental
steps:

```json
{
  "format_version": 2,
  "corpus": ["ab"],
  "vertices": {
    "a": [],
    "ab": [["a", "b"]],
    "b": []
  }
}
```

Vertex content is the key. Its value is an array of decompositions, each an
ordered array of child strings (`string[][]`); this preserves alternative
decompositions. Atoms have no decompositions. Tokens and decompositions are
sorted for readable diffs; IDs and hash iteration order are not serialized.
Version 2 replaces the reader-specific version 1 `steps` format.

Outputs are checked for content consistency, widths, duplicate tokens and
decompositions, reciprocal links, and corpus roots. Generation/validation errors
exit nonzero rather than publishing an empty substitute graph. A failed batch
can leave files from earlier successful scenarios.

**Known correctness limitation:** the unchanged oracle is order-dependent.
Different runs may generate different decompositions for the same corpus.
Sorting JSON does not fix that defect, and these structural checks do not prove
semantic completeness. The active regression
`partitions_are_independent_of_root_hash_iteration_order` exposes it.
Do not treat generated graphs as uniquely correct until it is resolved.
The planned fix is described in
[the positioned composition plan](agents/plans/20261118_PLAN_POSITIONED_COMPOSITION_ORACLE.md).

```bash
cargo test --release -p ngrams --test cli
# Currently fails on the known order-dependence regression:
cargo test --release -p ngrams --lib
```

### Trace the root-order regression

```bash
NGRAMS_TRACE_DIR="$PWD/target/ngrams-traces" \
  cargo test --release -p ngrams --lib \
  partitions_are_independent_of_root_hash_iteration_order -- --nocapture
```

This writes `subdivision-first.jsonl` and `visualization-first.jsonl` in the
reader's trace format, even though the equality assertion still fails. Files
with those names are overwritten on each run. Set an absolute directory path
to avoid resolving it relative to the crate's test working directory.

Each file records the root queue and the partition events, including positioned
child-cover choices, constructed patterns, and new labels. Compare the
`built partition patterns` events for `vis` and `visu`: in the
`visualization`-first run, `visu` labels the gap `vi` before `vis` chooses its
children. Event definitions live in [the graph tracing module](src/graph/trace.rs).
