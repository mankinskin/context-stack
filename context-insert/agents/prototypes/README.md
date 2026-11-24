# Positioned insert prototypes

Python prototypes behind the positioned insertion plan
([20261122_PLAN_POSITIONED_INSERT](../../../context-read/agents/plans/20261122_PLAN_POSITIONED_INSERT.md),
revision 3, roadmap step 3; revision 2:
[20261120_PLAN_POSITIONED_INSERT](../../../context-read/agents/plans/20261120_PLAN_POSITIONED_INSERT.md))
and the positioned reader plan
([20261122_PLAN_POSITIONED_READER](../../../context-read/agents/plans/20261122_PLAN_POSITIONED_READER.md),
prototype R0) with the dissolution plan
([20261124_PLAN_DISSOLUTION](../../../context-read/agents/plans/20261124_PLAN_DISSOLUTION.md),
task D0). They validate algorithms on the v3 oracle fixtures before
the Rust implementation and produce the plan's evidence numbers. They are
reference material, not tests: the Rust gate (plan task 3.4) replaces them.

## Model and limits

- A graph is a dict `vertex string -> [(offset, child string)]`, the v3
  fixture form (every maximal retained proper occurrence, overlaps kept).
- Strings name vertices and allow comparison with the oracle. The insert
  algorithms navigate only offsets and widths (`covering`, `ending_at`) and
  use strings only to name new vertices (`common.label`).
- Search is replaced by brute-force stand-ins in [common.py](common.py):
  `minimal_hosts` gives the response root and span, `start_path` and
  `end_path` the response paths (largest token at the start, largest token
  ending at the end), as validated for positioned search in context-search
  plan 2.6.
- An **insert case** is a fixture plus a substring `N` (2+ atoms) of a
  vertex string that is not a vertex (1,247 cases). The expected graph is the
  oracle's graph of `corpus + [N]`.

## Setup

Python 3.8 or later, standard library only. Run from this directory; the
scripts find the repository from their own location.

```sh
cargo build --release -p ngrams --bin ngrams    # oracle CLI used by most scripts
cd context-insert/agents/prototypes
```

`NGRAMS_BIN=/path/to/ngrams` overrides the oracle binary
(default `target/release/ngrams`). `cover_merge.py` and `invariants.py` read
the checked-in fixtures and do not need the binary. Oracle results are
cached per corpus within one run; a gate takes a few seconds.

## Scripts

| Script | Status | Plan reference |
|---|---|---|
| [split_positions.py](split_positions.py) | **accepted** insert algorithm (rev 3): split positions with roles pushed into every crossing child, intervals derived per vertex, bottom-up join | rev 3 §4–§5, tasks 3.2–3.4 |
| [split_join.py](split_join.py) | superseded (rev 2): split graph of stored intervals with interval closure | rev 2 Part B |
| [two_chains.py](two_chains.py) | rejected: start chain, then end chain, edits in place | Evidence, D4 |
| [naive_chains.py](naive_chains.py) | rejected: both response paths as they were before the insertion | Evidence (counterexample) |
| [descend_closure.py](descend_closure.py) | superseded: recursive closure with host descent; streaming read mode | revision 1 → 2, D1 |
| [read_legacy_port.py](read_legacy_port.py) | **accepted** reader prototype R0: the legacy reader loop (segments, root-postfix expansion, forward match) with positioned root edits; uses `split_positions.py` as `insert_match` | [reader plan](../../../context-read/agents/plans/20261122_PLAN_POSITIONED_READER.md) §3–§6 |
| [dissolve.py](dissolve.py) | **accepted** dissolution prototype D0: `read_legacy_port.py` with eager `settle` (dissolve or discard staged tokens with ≤ 1 parent occurrence) after every root commit; per-step prefix-oracle, G6, order and committed-vertex checks; G6 on the oracle graphs | [dissolution plan](../../../context-read/agents/plans/20261124_PLAN_DISSOLUTION.md) §2, §5 |
| [cover_merge.py](cover_merge.py) | rejected for the reader: read-only cover merge with an offset seam scan (first reader draft); the reader keeps the legacy root-postfix expansion instead | reader plan §0 |
| [invariants.py](invariants.py) | F3 (unique minimal host) and I2 (overlap closure) on all fixtures | Part B "Facts and invariants" |
| [find_examples.py](find_examples.py) | search for deep insert examples with minimal corpora | named fixtures of task 3.1 |
| [oracle_diff.py](oracle_diff.py) | oracle graph changes when inputs are appended | deriving expectations |
| [common.py](common.py) | oracle wrapper, fixtures, search stand-ins, navigation, named examples | — |

### split_positions.py (accepted, revision 3)

```sh
python3 split_positions.py                          # gate over all insert cases
python3 split_positions.py --no-overlaps            # without overlap intervals (expected to fail)
python3 split_positions.py --paths-only             # push only along covering/ending_at (expected to fail)
python3 split_positions.py --example insert_shared_vertex
python3 split_positions.py --query abababc abcabababcaba
```

Expected gate output:

```
cases: 1247
cases with positions off the response paths: 232
changed vertices: 1969
changed vertices that are not split nodes: 0
created twice: 0
max E positions per node: 2
max S positions per node: 2
new: 2119
new tokens that are not N, a prefix or a postfix of N: 0
off-path positions bounding a composed interval: 0
ok: 1247
resolved inside a child: 1942
split nodes: 3397
split positions: 2632
split positions from response path edges: 1969
```

`--no-overlaps` gives `ok: 1187`, `mismatch: 60`; `--paths-only` gives
`ok: 1039`, `error: 208`. The trace marks split positions that are not on a
response path with `*`.

### split_join.py (superseded by split_positions.py)

```sh
python3 split_join.py                                   # gate over all insert cases
python3 split_join.py --example insert_shared_vertex    # trace a named example
python3 split_join.py --query abcabca acabcabcab bcabcb # trace QUERY on CORPUS...
```

Expected gate output:

```
cases: 1247
cases with split nodes off the paths: 2
max intervals per node: 3
new: 2119
ok: 1247
response paths = primary split chains: 1247
skipped crossing child: 344
split nodes: 2910
```

`ok` counts graphs equal to the oracle; `new` the vertices created over all
cases (no `dedup` or `error` line means none). The trace prints the oracle
graph, the response (root, span, paths), the split graph (intervals per
vertex), every join step and the comparison with the oracle.

Named examples (`common.EXAMPLES`; minimal corpora, the oracle derives the
repeated children): `insert_shared_vertex` (a vertex on both paths),
`insert_periodic` (5 levels on the end side), `insert_deep_borders` (4 and 5
levels, no shared vertex), `insert_small_periodic` (`baba` in `ababab`).

### two_chains.py (rejected)

```sh
python3 two_chains.py                     # gate + staleness statistics
python3 two_chains.py --example insert_shared_vertex
```

Expected: `ok: 1247`, `end-first ok: 1247`, `vertex on both response paths:
225`, `end path stale after the start chain: 79`, `cases with reuse: 0`.
Correct results, but it composes a shared vertex twice and needs the second
path re-navigated; see plan decision D4.

### naive_chains.py (rejected)

```sh
python3 naive_chains.py                   # gate, expected to fail
python3 naive_chains.py --example insert_small_periodic
```

Expected: `ok: 1187`, `mismatch: 56`, `error: 4`, followed by the first
failing cases (periodic strings where one vertex lies on both paths).

### descend_closure.py (superseded)

```sh
python3 descend_closure.py                # insert cases
python3 descend_closure.py --streaming    # fixture prefixes + next input
```

Expected: `ok: 1247` and `changed vertices are the root or on the response
paths: 1247`; in streaming mode `ok: 98`, `mismatch: 2` (repeats inside the
new input, `abc` + `abcabcabc` and `abc` + `abcabca`, which belong to the
reader's in-progress cover, not to insertion).

### read_legacy_port.py (accepted, reader prototype R0)

```sh
python3 read_legacy_port.py                       # read every fixture scenario input by input
python3 read_legacy_port.py abc abcabca           # trace reading these inputs in order
python3 read_legacy_port.py abcd bcde xabcdey
python3 read_legacy_port.py --dissolve [INPUTS...] # with eager settle, see dissolve.py
```

Each input is read into the graph of its predecessors (the reader's own
result; after a mismatch the scenario continues from the oracle graph) and
compared with the oracle graph. Every owned-root edit is checked against the
brute-force complete composition (G2) and overlap closure (G4). Expansion
probes every postfix of the root (the former seam bound, skipping postfixes
wider than the consumed run, never filtered one on the fixtures and was
removed). Expected gate output:

```
append in place: 180
closure in place: 38
closure new root: 24
closure new root (self-expansion): 61
expansion probes: 188
forward searches: 196
inserted: 79
inserted into the input root: 37
mismatch: only extra tokens (provisional retention): 10
new root [root, t]: 199
reads equal to the oracle: 189
scenarios: 94
```

followed by the ten mismatches (`repeat_abcabca` extra `abc`,
`repeat_ababa` extra `ab`, `insert_periodic`, `insert_deep_borders`, six
`period_*`): tokens retained by a prefix of the input and no longer
frequent at its end (dissolution plan §0). No `G2`, `G4` or `error` line
means none.

### dissolve.py (accepted, dissolution prototype D0)

```sh
python3 dissolve.py                  # gate (same as read_legacy_port.py --dissolve)
python3 dissolve.py abcabca          # trace with settle actions and per-step checks
```

Runs `read_legacy_port.read` with `settle` (dissolution plan §2.3) after
every root commit (unknown-atom append, forward step, expansion step).
Staged vertices are those created by the running read; committed vertices
and atoms are never dissolved, the current root is exempt. After every step
it checks: the graph equals `oracle(C + [I[:consumed]])` (atoms of the
current unknown segment, created before they are consumed, are ignored);
G6 on staged vertices; `settle` largest-first equals smallest-first;
committed vertices are kept; every composition spells its vertex string and
is complete (G2). After every input: the step decisions `(kind, width, P,
E, root)` and the probed postfixes equal those of the same read without
`settle` (redundancy lemma, plan §2.4; on a divergence it prints both
decisions and the dissolved tokens with their parent occurrences),
equality with `oracle(C + [I])` and G6 (inputs exempt). Finally G6 on the oracle graph of every prefix of every
scenario's inputs. Expected output:

```
dissolved: 16
dissolved into the root: 0
oracle graphs: 199
oracle graphs violating G6: 0
reads: 199
reads compared: 199
reads equal to the oracle: 199
reads violating G6: 0
reads with identical decisions: 199
scenarios: 94
steps: 701
steps equal to the prefix oracle: 701
```

Missing lines are zero: no discards, no differing probed postfixes, no
diverging decisions, no order-dependent settle, no lost
committed vertex, no G2 or staged-G6 violation, no error. Without `settle`
the same checks report 10 reads and 11 steps unequal (and violating G6).

### cover_merge.py (rejected for the reader)

```sh
python3 cover_merge.py                    # projection + merge
python3 cover_merge.py --reinsert         # plus the superseded revision-1 reinsert check
```

Expected: `projection cases: 2646`, `projection failures: 0`, `merge cases:
2850`, `merge failures: 0`, `seam searches: 5288`, `searches when scanning
every start: 9486`; with `--reinsert` also `reinsert ok: 69`,
`reinsert failures: 0`.

### invariants.py

```sh
python3 invariants.py
```

Expected: `F3 substrings: 1247`, `F3 exceptions: 0`, `I2 overlapping pairs:
123`, `I2 exceptions: 0`. Any exception is printed with its fixture.

### find_examples.py

```sh
python3 find_examples.py --fixtures 10                     # deepest fixture queries
python3 find_examples.py --random 6 200 --metric levels    # random minimal corpora
```

Random mode samples for the given seconds and prints the best 8 as
`(score, total corpus length, corpus, (query, root, offset))`; results depend
on seed and time. Run several seeds in parallel for broader coverage. Check a
candidate with `split_join.py --query QUERY CORPUS...`.

### oracle_diff.py

```sh
python3 oracle_diff.py ababab + baba
```

Prints the graph of the corpus before `+` and the vertices created, removed
and recomposed after appending the inputs after `+`.

## Updating

When the fixtures change, re-run the gates and update the evidence table of
the plan. Keep the expected outputs in this file in sync with the plan.
