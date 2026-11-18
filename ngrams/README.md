# ngrams

Generate a positioned n-gram graph (format v3) for a corpus of input texts.
The library entry point `ngrams::graph::parse_corpus` and the binary use the
same oracle algorithm.

## CLI

Use a release build: even short corpora can be slow without optimization.

```bash
cargo build --release -p ngrams --bin ngrams
./target/release/ngrams "first text" "second text"
./target/release/ngrams --file scenarios.json --out ../context-read/tests/fixtures/v3
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

Write v3 output to its own directory (such as `context-read/tests/fixtures/v3`);
the checked-in v2 fixtures in `context-read/tests/fixtures/` come from the
legacy pipeline and must not be overwritten.

## JSON format (version 3)

Each output describes the **entire final graph** for one corpus, not incremental
steps:

```json
{
  "format_version": 3,
  "corpus": ["abcabca"],
  "vertices": {
    "a": [],
    "abca": [[0, "a"], [1, "b"], [2, "c"], [3, "a"]],
    "abcabca": [[0, "abca"], [3, "abca"]],
    "b": [],
    "c": []
  }
}
```

- `corpus` keeps the supplied texts in order, including repeats; each text is
  a vertex, and repeated texts share one vertex.
- `vertices` contains exactly the retained set: corpus roots, atoms (one
  Unicode scalar `char` each), and frequency-selected tokens. There are no
  wrapper, gap, or compatibility vertices.
- Each value lists every **maximal proper** retained occurrence in the owner
  as `[atom_offset, child]`, ordered by offset and then child width. Offsets
  count `char`s, not bytes. Wholly contained and duplicate occurrences are
  dropped; partial overlaps are kept, so one child can appear at several
  offsets. Above, `abca` occurs at offsets 0 and 3 and overlaps at position 3.
  Read overlapping children by projecting them into owner coordinates, not by
  concatenating them. Atoms have empty covers.
- Keys are sorted; IDs, reverse links, and containment data are not
  serialized. Identical ordered inputs produce identical bytes.

Outputs are structurally validated before they are written: version, corpus
roots, nonempty content, atoms vs. composites, known and strictly narrower
children, in-range offsets whose span equals the child, canonical order, no
duplicate or contained occurrences, and full positional coverage. Duplicate
vertex keys are rejected when decoding. Errors name the owner and the offending
entry or position. Structural validation cannot prove that every maximal
occurrence is present (e.g. `ababa` as `[[0,"aba"],[3,"b"],[4,"a"]]` is
well-formed but omits `aba` at offset 2); the oracle's tests check covers
against independent substring enumeration. Generation/validation errors exit
nonzero rather than publishing a partial graph. A failed batch can leave files
from earlier successful scenarios.

The result is independent of traversal order and of corpus input order (apart
from the `corpus` field itself).

```bash
cargo test --release -p ngrams
cargo test --release -p ngrams --lib positioned_corpus_sweep -- --ignored
```

## Legacy row-based pipeline (version 2)

`ngrams::graph::parse_corpus_legacy` keeps the previous pipeline, which
additionally selects overlap wrappers and gap tokens and returns a row
`Hypergraph` (`LegacyParseResult`). Its interchange types live in
`ngrams::output::legacy` (`GraphSnapshot`, `GraphData`, `FORMAT_VERSION = 2`),
where each vertex maps to alternative decompositions as ordered arrays of child
strings. The CLI does not emit v2. Row consumers (workspace creation, v2
fixtures, and provisional reader comparisons) use it until they migrate.

**Known limitation (legacy only):** the legacy pipeline is order-dependent;
different runs may generate different decompositions for the same corpus. The
ignored regression `legacy_partitions_are_independent_of_root_hash_iteration_order`
reproduces it. See
[the positioned composition plan](agents/plans/20261118_PLAN_POSITIONED_COMPOSITION_ORACLE.md).

### Trace the legacy root-order regression

```bash
NGRAMS_TRACE_DIR="$PWD/target/ngrams-traces" \
  cargo test --release -p ngrams --lib \
  legacy_partitions_are_independent_of_root_hash_iteration_order \
  -- --ignored --nocapture
```

This writes `subdivision-first.jsonl` and `visualization-first.jsonl` in the
reader's trace format, even though the equality assertion still fails. Files
with those names are overwritten on each run. Set an absolute directory path
to avoid resolving it relative to the crate's test working directory.

Each file records the root queue and the partition events, including positioned
child-cover choices, constructed patterns, and new labels. Compare the
`built partition patterns` events for `vis` and `visu`: in the
`visualization`-first run, `visu` labels the gap `vi` before `vis` chooses its
children. Event definitions live in [the graph tracing module](src/graph/trace.rs);
they are emitted only by the legacy partition pass.
