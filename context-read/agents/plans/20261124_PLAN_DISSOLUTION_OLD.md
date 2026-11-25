---
tags: `#plan` `#context-read` `#context-trace` `#positioned-composition` `#reader` `#retention` `#dissolution` `#staging`
summary: Dissolution of provisional tokens for the positioned reader (reader plan phase R5, roadmap step 5). A token created by the running read can lose its token status later in the same input (all its occurrences end up inside one larger token; `abcabca` keeps `abc` today). Retention invariant G6 (every committed vertex that is neither an atom nor an input has ≥ 2 parent occurrences; holds on all 2339 oracle graphs). An eager, local `settle` at the end of every reader step dissolves or discards staged tokens with ≤ 1 parent occurrence (`GraphEdit::dissolve`/`discard`); its candidates are exactly the vertices that lost a link in the step's edit records, the step's new tokens and the previous root. Removal is immediate (swap-remove with renaming, no tombstones); new atoms are committed at once, so only composites are staged; committing an invalid vertex set panics. Overlay stage only: tagged staging is removed. Fixes the 10 provisional-retention deviations (offline simulation: 94/94 scenarios, 140/140 read-lane cases equal the oracle).
status: 🗄️ superseded by 20261125_PLAN_DISSOLUTION.md (clean implementation plan, 2026-11-25); this file keeps the design history (revisions 1–3.2)
date: 2026-11-25
parent: 20261122_PLAN_POSITIONED_READER.md (§5 provisional retention, phase R5)
depends_on: 20261124_PLAN_STAGING_GRAPH.md (implemented S0–S6; D-S1 final: overlay), 20261122_PLAN_POSITIONED_INSERT.md (revision 3, G1–G5, GraphEdit D8, no rollback D10)
---

# Plan: Dissolution of provisional tokens

> **Superseded (2026-11-25)** by
> [20261125_PLAN_DISSOLUTION](20261125_PLAN_DISSOLUTION.md), the clean
> implementation plan. This file is the design history only.

**Reader:** [20261122_PLAN_POSITIONED_READER](20261122_PLAN_POSITIONED_READER.md)
(§5 provisional retention, phase R5).
**Prerequisite:** [20261124_PLAN_STAGING_GRAPH](20261124_PLAN_STAGING_GRAPH.md)
(vertex states, store traits, overlay stage, `ReadStage`; implemented and
behavior-neutral). This plan is the first one that changes the reader's
output.
**Insertion:** [20261122_PLAN_POSITIONED_INSERT](20261122_PLAN_POSITIONED_INSERT.md)
(invariants G1–G5 §3.1, `GraphEdit` D8, no rollback D10).

**Revision 3.2 (2026-11-25).** Dissolution into the root is proven
redundant for every root, with or without parents: the root is
right-maximal (Lemma RM, L6, §2.8). The early-commit rule is stated,
sound and exact (E1, E2), with a graph-local form: off the root's postfix
chain, or ≥ 2 fixed right contexts by a parent walk (§5.1). A physical
commit into the base during the read would violate G6 (all early tokens
of the checked base views have their second parent occurrence in the open
root); proposed instead is a stability mark (stable slot prefix). Evidence
from the D1b prototype `early_commit.py`.

**Revision 3.1 (2026-11-25).** Q1–Q4 decided: `settle`'s candidates
come from the step's edit records (insertion and root-commit
`Publication`s) plus the previous root, exact by the invariant (S)
(§2.3.1); the only runtime G6 check is at commit (§2.1); dissolution into
the root is allowed (§2.8); new atoms are committed immediately, so the
stage holds only composites (§4).

**Revision 3 (2026-11-25).** Decisions A–H of §7 applied: the tagged stage
(staging plan §5, implementation (b)) and the `StagingStrategy` abstraction
are removed (task D2), so every store change below concerns the overlay
only; removal of staged vertices is immediate (swap-remove with
reconciliation, §2.2), there are no tombstones; `settle` is eager and local
(§2.3); vertices entering the committed graph must satisfy G6 or `commit`
panics (§2.1); there is no stored input mark (§2.1). Corrections against
the implementation (comparison of 2026-11-24): atoms are exempt from
`settle` (the reader holds them across steps), `settle` runs after every
step including adopting commits, `GraphEdit` bookkeeping must follow
removals, the read-lane base graphs are built without a loader, G6 is a
separate check (`validate_retention`), and the module paths follow the
post-S6 layout (`positioned/{edit,stage,…}/`).

## 0. Problem

The reader inserts every accepted match at once (legacy rule). A token that
a prefix of the input makes frequent can lose that status later in the same
input: all its occurrences end up inside occurrences of one larger token,
and the oracle does not retain it.

Example `abcabca`: after `abcabc`, `abc` is a border of the input read so
far and correctly a token (root `[abc@0, abc@3]`). The final `a` shifts
the border chain to `abca`, `abcabca`. Every occurrence of `abc` now lies
inside `abca`; the oracle has `abca = a@0, b@1, c@2, a@3` and no `abc`.
Today's reader output (`cargo test -p context-read --no-default-features
--features test-api --test reader_behavior overlay::repeat_abcabca --
--ignored`):

```text
repeat_abcabca, final corpus graph: graph differs from expected:
  extra vertices: "abc"
  "abca": missing ["a"@0, "b"@1, "c"@2] extra ["abc"@0]
```

In general, the oracle tokens of `p^n t` (`t` a proper prefix of the
pattern `p`) are the chain `p^k t`; whether `p` is a token is decided by
the first atom of `t`, `(n − 1)·|p|` positions after `p` was created. No
bounded window decides it: an "overhang" search that continues past the
root into its own span and stops after one full repeated period fixes
`n = 2` only. The decision therefore has
to be revised backwards: a token created earlier in the read is removed
again. The staging graph makes exactly these tokens (created by the running
read, not yet committed) identifiable and mutable.

**Evidence (2026-11-23):**

| Check | Result |
|---|---|
| Scenarios deviating (ignored, "provisional retention") | 9: `repeat_abcabca`, `repeat_ababa`, `insert_periodic`, `period_ab_x3_a`, `period_abc_x3_a`, `period_abc_x3_ab`, `period_abcd_x2_a`, `period_abcd_x2_abc`, `period_abcd_x3_ab` |
| Read-lane cases deviating | 1: `insert_deep_borders__1` (`READ_DEVIATIONS`) |
| Whole repetitions `p^n` (17 `period_*` scenarios) | all equal the oracle |
| Kind of deviation | only extra tokens, always created by the current read |
| G6 (§2.1) on all oracle graphs (v3, insert and read lanes) | holds on 1630 / 1630 |
| G6 on the reader output | violated in exactly the 10 deviating graphs |
| Offline dissolution (§2.2) of the reader output | 94 / 94 scenarios and 140 / 140 read-lane cases equal the oracle, in either order |

**Prototype D0 (2026-11-24,
[`dissolve.py`](../../../context-insert/agents/prototypes/dissolve.py),
`python3 read_legacy_port.py --dissolve`), eager `settle` in the reader
port:**

| Check | Result |
|---|---|
| Reads equal to the oracle (94 scenarios) | 199 / 199 (without `settle`: 189 / 199) |
| Steps equal to `oracle(C + [I[..consumed]])` after `settle` | 701 / 701 (atoms of an unknown segment, created before they are consumed, ignored) |
| G6 after every step / on the final graphs / on the oracle graph of every prefix | 0 / 0 / 0 violations |
| `settle` largest-first vs smallest-first | identical on every step (T2) |
| Committed vertices removed or changed in content | 0 |
| Step decisions (kind, width, `P`, `E`, root) and probed postfixes with vs without `settle` | identical in 199 / 199 reads (redundancy lemma) |
| Dissolutions / into the current root (Q3, §2.8) | 16 / 0 |
| Random inputs (one-off: all `{a,b}` strings of length 2–10, 1500 random `{a,b,c}` strings, 600 input pairs) | 4744 reads: decisions identical, all equal the oracle; 548 dissolutions, 0 into the root |

The G6 fixture check (D1, `every_oracle_graph_satisfies_retention` in
`context-read/tests/fixture_validation.rs`) covers 2339 oracle graphs of
the v3, insert and read lanes: 0 violations. It identifies inputs by the
snapshot's `corpus` strings, not by a stored mark.

## 1. Goals and non-goals

Goals:

- Every read yields `oracle(C + [I])`: no expected deviations, no tests
  ignored for provisional retention, `READ_DEVIATIONS` removed.
- **Streamability:** after every reader step the stage equals the oracle
  graph of the input consumed so far (`oracle(C + [I[..cursor]])`, up to
  atoms not consumed yet), so intermediate results are complete and usable
  at every step; tokens that no continuation can dissolve are marked
  stable at once (§5.1).
- G6 as an invariant of the committed graph: a vertex entering it violates
  G6 only through a bug (panic); tests check the whole committed graph
  (`validate_retention`).
- Dissolution is one strict `GraphEdit` operation (D8) with written
  preconditions, defined only on stages.
- One staging implementation (overlay): the tagged stage and the strategy
  abstraction are removed.

Non-goals:

- No change to the reader loop, search or insertion.
- No rollback (insert plan D10): dissolution is a forward edit, not an
  undo; committed vertices keep their content (staging plan §2.1).
- No performance work beyond "reasonable" asymptotics (local `settle`,
  §2.3). Overlay overhead, `Sync` handles and profiling are a later plan
  (§6).

## 2. Model

### 2.1 Retention invariant G6 and inputs

> **G6.** Every committed vertex that is neither an atom nor a corpus input
> has at least two parent occurrences (entries `(parent, offset)`).

A vertex with a single parent occurrence `(U, o)` occurs only inside `U`
at offset `o`, so its string has no context of its own; the oracle does not
retain it. G6 holds on every oracle graph (§0). G6 is a graph invariant
next to G1–G5 (insert plan §3.1).

**Inputs.** An input is the whole string of one `read` call (one corpus
entry); `read` is the only public interface of the context stack, so only
reads create inputs. Insertion never creates inputs: a token created by
`insert`, `insert_match` or `insert_next_match` is an ordinary token, and
the insert gate (`insert` outside a read) does not check G6.

**No stored input mark (decided 2026-11-25).** The input of a read is its
final root. Inside a stage the only staged vertex that can be an input is
the stage root, which `settle` and `commit` know by identity (the `root`
argument): a former root is never an input. Committed vertices are never
dissolved, so nothing needs to know whether a committed vertex is an
input, and `VertexRead` gains no `is_input`. Tests that check G6 on a
committed graph pass the inputs explicitly (`validate_retention(inputs)`):
the tokens returned by the reads, or the corpus strings of a fixture
mapped to tokens by `from_interchange_named` (§5).

**Where G6 is enforced (decided 2026-11-25):**

1. **Staged vertices:** `settle` dissolves or discards every staged
   composite other than the root with ≤ 1 parent occurrence after every
   reader step (§2.2, §2.3). Staged vertices violate G6 only between two
   `settle`s.
2. **Committing:** `StageHandle::commit(root)` panics if the vertex set it
   commits is invalid: a staged composite other than `root` with fewer
   than two parent occurrences (a bug: `settle` missed it). This is the
   only runtime G6 check.
3. **Already committed vertices are not checked at runtime.** Their parent
   counts can change in later reads (insertion and dissolution move
   occurrences into new tokens). A committed composite without parents is
   an input: non-inputs are committed with ≥ 2 parent occurrences and keep
   them (M5, §2.7). The converse does not hold, an input may also keep
   exactly one: corpus `bc`, `abcd`, `abce`; `bc` has the parent
   occurrences `abcd@1`, `abce@1`. Reading `xabcd` creates `abc` (now
   left-branching: `start`, `x`), and both occurrences of `bc` move into
   it: `bc` keeps only `abc@1`, which is valid for an input. Tests check
   the whole committed graph with `validate_retention(inputs)` after every
   read.

### 2.2 Dissolution and immediate removal

```text
dissolve(t):   precondition: t staged, composite, not the stage root,
               exactly one parent occurrence (U, o)
  comp(U) := maximal(comp(U) − t@o ∪ (comp(t) + o))
  links of t's children: (t, a) → (U, o + a), dropped where filtered by maximal
  remove t

discard(t):    precondition: t staged, composite, not the stage root,
               no parent occurrence
  unlink t's children; remove t
```

- `U` may be staged or committed. A committed `U` only changes its
  composition shape, not its content (allowed by staging plan §2.1; the
  overlay keeps it as the copy-on-write composition of its `BaseDelta`).
- **G1–G5 after dissolution:** §2.6, T1. Only occurrences from
  `comp(t) + o` are filtered by `maximal`.
- **Removal is immediate (decided 2026-11-25).** The removed vertex
  leaves no tombstone: the overlay stores staged vertices densely and
  removes by **swap-remove**. The last staged vertex `l` moves into the
  slot of `t`, and every reference to `l` is renamed in the same call:
  the parent links of its children (staged children: their link sets;
  committed children: `BaseDelta::added`) and the compositions of its
  parents (staged parents; committed parents: the delta composition).
  The stage holds no atoms (§4), so `l` is always a composite. This is
  local: `O(|comp(l)| + p(l))` entries. The operation returns `Moved { from: l, to: t }` (none if
  `t` was last).
- **Held indices.** Vertices are removed only inside `settle`, so every
  index recorded during a step (the step's edit records, §2.3.1) is still
  valid when `settle` starts. Inside `settle`, `Moved` renames the
  worklist, `GraphEdit`'s bookkeeping (`created`, `edits`, `touched`) and
  the root. Across steps the reader holds only the root and atoms; new
  atoms are committed when they are created (§4), so they never move.
- **Slot order.** Staged slots are no longer in creation order;
  `Stage::staged()` and `StagingGraph::vertices` document slot order.
  Nothing depends on the creation order of staged slots (checked
  2026-11-25: only doc comments state it; the order of committed indices
  after `commit` follows the slots).

### 2.3 `settle`: eager and local

**What `settle` does.** At the end of every reader step, `settle`
restores the invariant

> **(S)** every staged composite other than the root has ≥ 2 parent
> occurrences,

which held after the previous step. Its input is the step's candidate set
`K` (§2.3.1) and the root:

```text
settle(K, root):
  while K is not empty:
    t := pop(K)
    if t is committed or t = root: continue   # atoms are committed (§4)
    match p(t):                                # number of (parent, offset) entries
      ≥ 2: keep t
      1:   K ∪= dissolve(t).children           # §2.2: t@o in its host U is replaced by comp(t) + o
      0:   K ∪= discard(t).children
    rename K and root by the Moved of the removal
```

- **Eager (decided):** after every step and every kind of commit (also
  `Adopt`, which opens no edit of its own; §5). Afterwards (S) holds and
  the stage equals the oracle graph of the consumed prefix (D0: 701 / 701
  steps): the streaming property of §1.
- **Termination:** a removal removes a vertex and adds only its children
  to `K`; every other iteration shrinks `K`.
- **Order:** the result does not depend on the order (§2.6, T2).

#### 2.3.1 The candidates (decided: from the step's edit records)

Every edit of a step returns its record (`Publication`): insertion
returns it in `InsertOutcome::Created { publication }` (one `GraphEdit`
per insertion; today the reader drops it through `outcome.token()`), and
the root commit's `GraphEdit::finish()` returns one as well. A record
lists the created tokens and, for every vertex that existed before the
edit, the occurrences added to and removed from its composition (an
occurrence removed and re-added in the same edit cancels).

**Candidates of a step:**

```text
K := { o.child | P ∈ records of the step, e ∈ P.edits, o ∈ e.removed }  # lost a link
   ∪ { t       | P ∈ records of the step, t ∈ P.created }               # new
   ∪ { previous root, if the step changed the root }                     # no longer exempt
```

**Why this is exact.** A parent count changes only through links.
Relative to the state before an edit, a vertex's count decreases only by
losing a link to a vertex that existed before the edit: a link to a vertex
created in the same edit is gained in that edit before it can be lost
(net ≥ 0). The losses from pre-existing vertices are exactly the
`removed` occurrences of the record. So after a step, a vertex violating
(S) is

- a vertex whose count decreased: a child in some `removed` list;
- a vertex created in the step: it had no count before; or
- the previous root: exempt until the root changed.

Every other staged composite had ≥ 2 parent occurrences after the
previous `settle` and lost none. `K` may contain vertices that still have
≥ 2 (checked and kept), but it misses no violator. Inside `settle`, a
dissolution or discard decreases only the counts of the removed vertex's
children (T1, L2), which it adds to `K`. Debug and `test-api` builds
assert after every `settle` that a full scan of the stage finds no
violation of (S).

**Locality.** The candidates are bounded by the step's new compositions,
not by the stage size:

> **C1 (losses are local).** If `maximal` filters `x@k` from `comp(U)`
> because the added occurrence `y@j` contains it, and G2 holds before the
> edit, then `x` is a child of `y` (`x@(k − j) ∈ comp(y)`) or a descendant
> of `y` through vertices created in the same step.
>
> *Sketch.* `x@k ∈ comp(U)` is maximal among all vertex occurrences in
> `U` (G2). By G2 for `y`, `x@(k − j)` is listed in `comp(y)` or lies
> inside a listed child `c@m`. In the second case `c` occurs in `U` at
> `j + m` and contains `x@k`; if `c` existed before the edit, `x@k` was
> not maximal, a contradiction. So `c` is new, and the argument recurses
> into `c`.

So the vertices that lose a link are children of the occurrences added in
the step ("scan the connected children of the new vertex"). The added
occurrence need not be a new vertex: `extend(R, E@o)` adds an existing
`E` when the expansion probe matched a complete token. C1 explains the
cost; correctness rests only on the records.

### 2.4 Decisions never depend on dissolved tokens

**Lemma (redundancy).** A staged token `t` with a single parent occurrence
`(U, o)` occurs only inside `U` at offset `o`. Dissolving it does not change
any reader decision:

| Decision | Depends on | Effect of dissolving `t` |
|---|---|---|
| forward `insert_next_match(rest)` | the string of the longest occurring prefix | none; at most `Complete` becomes `Created` |
| probe `[P] ++ rest`, `P = t` | `t` is a postfix of the root | `U` ends where `t` ends and is probed first; `[t] ++ rest` matches only if `[U] ++ rest` does (`U ≠ R`; for `U = R` the probe of `t` is a rejected, redundant step, §2.8) |
| `find_complete([R, t'])` | existence of `Y` | if `Y` was dissolved, a new root with the same string is created |
| root, `P`, `E` within one step | identities | `settle` runs only between steps; the root is exempt; `E` has ≥ 2 parent occurrences after the commit |

So the reader holds only the root (exempt, renamed through `Moved`) and
committed atoms across `settle`, and decisions rely on strings, never on
staged identities. Proof: §2.6, L3–L5; dissolution into the root: §2.8.

### 2.5 Walkthrough `abcabca`

Empty graph, overlay stage. `segments` commits the new atoms `a`, `b`, `c`
at once (§4). Staged slots: S1 s0, S2 s1, S3 s2.

| Step | Graph (`S` = staged) | Records → candidates → `settle` |
|---|---|---|
| unknown `a`, `b`, `c` | `Append(a)` adopts `a`; `Append(b)` creates root S1 `ab`; `Append(c)` extends S1 in place to `abc` | created {S1}: the root |
| forward `abc` | `insert_next_match` finds S1 itself (`Complete`, no edit); `[S1, S1]` is no token: root S2 `[S1@0, S1@3]` | created {S2} (root), previous root S1: 2 parent occurrences, kept |
| probe `[S1, a]` | insertion creates S3 `abca = [S1@0, a@3]` in S2: S2 `[S3@0, S1@3]` | insert record: created {S3}; S2 removed {S1@0} |
| commit `Expand` | `extend(S2, S3@3)`: S2 `[S3@0, S3@3]` | commit record: S2 removed {S1@3}. `K` = {S3, S1}: S3 has `(S2, 0)`, `(S2, 3)`, kept; S1 has only `(S3, 0)`: dissolve into S3, S3 `[a@0, b@1, c@2, a@3]`; children `a`, `b`, `c` are committed; swap-remove moves S3 from s2 to s0 |
| `handle.commit(S2)` | S2 is the input; S3 committed with 2 parent occurrences | valid vertex set; equals the oracle |

Without this plan the `settle` column is empty and S1 `abc` is committed
with a single parent occurrence: today's deviation.

### 2.6 Proofs (task D1)

Notation as in insert plan §3.1. `occ(G)` is the set of strings that occur
in some vertex of `G` (substrings of vertex strings). `p(x)` is the number
of parent occurrences of `x`. All statements assume G1–G5 before the edit.

**L1 (locality).** Let `t` be a vertex with exactly one parent occurrence
`(U, o)`. Every occurrence of `t`'s string inside a vertex `w ≠ t` lies
inside an occurrence of `U` in `w` at offset `−o` relative to it (for
`w = U`: it is the occurrence at `o`). In particular `t` occurs in `U` only
at `o`, and `t@o` lies inside no other occurrence of `comp(U)`.

*Proof.* Induction on `|w|`. By G2 the occurrence lies inside a listed
occurrence `c@y` of `comp(w)` or is one. If it is one, `(w, x)` is a parent
occurrence of `t`, so `w = U`, `x = o`. Otherwise recurse into `c`
(`|c| < |w|`): it lies inside an occurrence of `U` in `c`, shifted by `y`.
For `w = U` the recursion cannot reach `U` again (`|c| < |U|`), so the
occurrence is `t@o`; `t@o` is maximal in `comp(U)` by G2. ∎

Consequence: `occ(G) = occ(G − t)`, since every occurrence of `t` and of
its substrings lies inside `U`.

**T1 (dissolution keeps G1–G5).** Let `G' = dissolve(G, t)`.

- **G1:** a vertex is removed; contents are unchanged.
- **G2 for `w ≠ U`:** `comp(w)` does not list `t` (its only parent
  occurrence is in `U`). Every occurrence of a vertex of `G'` in `w` lies
  inside a listed occurrence (G2 in `G`, `G' ⊆ G`), and listed occurrences
  are maximal among a superset, so `comp(w)` is the maximal set over `G'`.
- **G2 for `U`:** let `M` be the occurrences in `U` of vertices of `G'` and
  `C = comp(U) − t@o ∪ (comp(t) + o)`. `C ⊆ M`. Every `m ∈ M` lies inside
  an element of `comp(U)`; if that element is `t@o`, then `m` is an
  occurrence in `t` of a vertex `≠ t` (G1), so it lies inside an element of
  `comp(t) + o` (G2 for `t`). Hence `maximal(M) = maximal(C)`. Equal
  intervals have equal strings, so by G1 no two elements of `C` share an
  interval. `|comp'(U)| ≥ 2` because atoms are vertices and `U` is not a
  proper occurrence of itself.
- **No element of `comp(U) − t@o` is removed by `maximal`:** it would lie
  inside `t@o`, contradicting the maximality of `comp(U)`. So only
  occurrences from `comp(t) + o` are filtered.
- **G3:** maintained by `GraphEdit` (unlink `(t, a)` for every child, link
  `(U, o + a)` for the kept ones).
- **G4:** two overlapping occurrences of `comp'(U)` are two token
  occurrences in `U`; their overlap is a vertex of `G` by the intersection
  lemma (reader plan §3.4). It is not `t`: if both come from
  `comp(U) − t@o`, the overlap equal to `t` would be `t@o` (L1) inside two
  other listed occurrences, contradicting maximality; otherwise one comes
  from `comp(t) + o` and the overlap is shorter than `t`.
- **G5:** strings that are not vertices in `G'` are `t` and the non-vertices
  `s` of `G`.
  - `s = t`: `(U, o)` is a minimal host occurrence (no element of
    `comp'(U)` contains `[o, o + |t|)`, see above), and by L1 every other
    occurrence lies inside an occurrence of `U`, which lies inside a child
    of its host (G2), so it is not minimal.
  - `s ≠ t` with minimal host `(v, x)` in `G`, `v ≠ t`: compositions of
    `w ≠ U` are unchanged, and in `U` the new elements lie inside `t@o`, so
    `(v, x)` stays minimal; no other occurrence becomes minimal (an
    occurrence that lay inside `t@o` lies inside a child of `t`, now an
    element of `C`).
  - `s ≠ t` with minimal host `(t, x)` in `G`: the new minimal host is
    `(U, o + x)`; every other occurrence of `s` lies inside an occurrence
    of `t`, hence (L1) inside an occurrence of `U`. ∎

`discard(t)` with `p(t) = 0`: by the argument of L1, `t` occurs in no
other vertex, so no composition changes; only the links `(t, a)` of its
children are removed. G1–G5 are kept trivially.

**L2 (monotonicity).** If `p(t) ≤ 1` and `t` is dissolved or discarded,
then `p(x)` does not increase for any other vertex `x`.

*Proof.* Only `U` and the children of `t` change links. No element of
`comp(U) − t@o` is removed (T1), so the children of `U` keep their counts.
A child `c` of `t` loses each `(t, a)` and gains at most `(U, o + a)`. ∎

Removing a vertex with `p ≥ 2` could increase counts (its children would
move into two hosts); `settle` never does this.

**T2 (confluence of `settle`).** For a fixed root and staged set, every
order of `settle` removes the same set of vertices and yields the same
graph.

*Proof.* By G1 and G2 the compositions are a function of the vertex set
(insert plan §3.1), so a state is determined by its vertex set. `settle`
terminates (each step removes a vertex). Local confluence: if `a ≠ b` are
both removable (staged, not the root, `p ≤ 1`) in a state, then by L2 `b`
is still removable after removing `a` and vice versa, and both orders reach
the same vertex set. By Newman's lemma every maximal run reaches the same
final vertex set, hence the same graph. ∎

**L3 (forward match).** `insert_next_match(rest)` searches the longest
prefix of `rest` in `occ(G)`. By L1, `occ` is unchanged by `settle`, so the
matched string is the same; at most a dissolved `t` is matched again and
recreated by insertion (`Complete` becomes `Created`).

**L4 (probes, `U ≠ R`).** Let `t` be a postfix of the root `R` with
`p(t) = 1`, `(U, o)`, and `U ≠ R`. By L1, the occurrence of `t` at the end
of `R` lies inside an occurrence of `U` in `R`, which therefore ends at
`|R|`: `U` is a larger postfix of `R` and is probed before `t`. If
`t + rest[..k]` occurs, its `t` lies inside a `U` ending at the same
position (L1), so `U + rest[..k]` occurs. Hence the probe of `t` is
accepted only if the probe of `U` was, and never in either graph: removing
`t` from the postfix chain does not change the accepted probe.

**L5 (`find_complete`, root identities).** `R + t'` is looked up by
string; if the vertex was dissolved, a new root with the same string is
created (`Adopt` becomes `NewRoot`). `settle` runs between steps; the
reader holds only the root, which is exempt.

**Case `U = R`.** L4 needs `U ≠ R`; the case that the single parent
occurrence of a postfix `t` is the root itself is L6 (§2.8), proven by the
right-maximality of the root (RM).

### 2.7 Maximal-repeat characterization (task D1b)

**Observation (2026-11-24, research script; to become a prototype in
D1b).** For every corpus `C` tested, the oracle's vertex set is

> `oracle(C) = atoms(C) ∪ inputs(C) ∪ MR(C)`

where `MR(C)` is the set of **maximal repeats** of `C`: strings of at least
two atoms with at least two occurrences in the inputs of `C` that are both

- **left-branching:** the occurrences have ≥ 2 distinct left contexts, and
- **right-branching:** they have ≥ 2 distinct right contexts,

with the start and the end of an input counted as a context distinct from
every atom and from every other occurrence's boundary. Checked on all 199
oracle graphs of the scenario prefixes (every `inputs[..k]` of every
scenario) and 3544 random corpora (all `{a, b}` strings of length 2–10,
1500 random corpora of 1–3 `{a, b, c}` strings): 0 differences.

This turns the oracle's frequency rule into a statement about strings and
explains the dissolutions: in the oracle of the consumed prefix
`C + [I[..cursor]]`, the cursor is an input end, i.e. a right context of
its own. A token can be right-branching only through its occurrence ending
at the cursor; when the next atom equals its other right contexts, it
stops being a maximal repeat. These are exactly the tokens `settle`
dissolves. Contexts inside finished inputs never change, so maximal
repeats of the committed corpus stay maximal repeats (M5).

**Statements to prove (D1b):**

| # | Statement | Consequence |
|---|---|---|
| M1 | `oracle(C) = atoms ∪ inputs ∪ MR(C)` | correctness of every operation reduces to its vertex set (G1 + G2 determine the graph) |
| M2 | In a graph whose vertex set is `atoms ∪ inputs ∪ MR`, a vertex that is not an atom or input has `p ≥ 2` iff it is a maximal repeat (one direction is L1) | G6 holds on oracle graphs; G6 violations are exactly non-maximal staged tokens |
| M3 | G4 and G5 for such vertex sets: the overlap of two overlapping maximal repeats in a vertex is a vertex; every non-vertex string has a unique minimal extension to a vertex | G4/G5 without reference to the oracle |
| M4 | One reader step plus eager `settle` maps the vertex set of `C + [I[..c]]` to that of `C + [I[..c']]`: new maximal repeats are the inserted tokens and the root; lost ones are those right-branching only through the cursor, removed by `settle` | per-step oracle equality (D0: 701 / 701); the streaming property (§1); the early-commit rule (§5.1) |
| M5 | `MR(C) ⊆ MR(C + [I])` for a corpus `C` of finished inputs | committed non-input vertices keep `p ≥ 2` in every later read (with M2): G6 on committed vertices needs no runtime repair (§2.1) |
| C1 | Losses are local (§2.3.1) | the candidates are bounded by the step's new compositions (cost, not correctness) |
| RM | **Proven** (§2.8): at every probing step `R + rest[0]` occurs nowhere; with L1 the probe of a postfix whose only parent is the root is rejected (L6) | the redundancy lemma holds without exception |
| E1, E2 | Safe tokens are never dissolved; a settled token that is not safe can be dissolved by some continuation (§5.1) | the early-commit rule is sound and exact (rests on M2, M4, M5) |

Approach: M1 by comparing the oracle's retention rule (`ngrams`) with the
branching conditions; M2 from L1 and its converse (two occurrences with
different left or right contexts cannot both lie inside one `U` at a
common offset); M3 from the closure properties of maximal repeats; M4 by
case analysis of the commit table (reader plan §3.4) on the contexts that
change when the cursor moves; M5 since the contexts of occurrences in
finished inputs are fixed and new inputs only add occurrences.

### 2.8 Dissolution into the root (Q3, proven)

Question: may `settle` dissolve a staged postfix `t` of the root `R` whose
only parent occurrence is `t@o` in `R` (`o + |t| = |R|`)? The probes of the
next step are the postfix chain of `R` (the chain of last children),
largest first.

- **Without dissolution** the iterator probes `[t] ++ rest`; if that is
  rejected, it descends to the last child `P'` of `t`.
- **With dissolution** `t` is replaced by `comp(t) + o`. `t@o` was the only
  element of `comp(R)` ending at `|R|` (two elements with the same end
  would contain each other), so no other element contains the shifted
  `P'`: `maximal` keeps it as the new last child of `R`, and the iterator
  starts at `P'`.

So dissolving `t` into `R` changes no decision iff the probe of `t` is
always rejected. It is, by the following lemma, for every root, with or
without parents. This is the argument of 2026-11-25 made precise: `t` has
only `R` as a parent (L1), and `R` is already maximal (RM).

**Lemma RM (the root is right-maximal).** At the start of every step that
probes (cursor > 0 in a known run, `rest` not empty), `R + rest[0]` occurs
in no vertex: `R + rest[0] ∉ occ(G)`.

*Proof.* Case analysis on the last root commit (reader plan §3.4,
`ReadStage::commit`):

- **`InPlace`, `NewRoot`:** `R` has no parent occurrence. An editable root
  is parentless; `create` builds a vertex that did not exist
  (`find_complete` rejected the union), and nothing references it yet.
  Every occurrence of `R`'s string in a vertex `w ≠ R` would lie inside a
  chain of child occurrences ending in a parent occurrence of `R` (G2).
  There is none, so `R`'s string occurs only as `R`, and the longer
  `R + rest[0]` occurs nowhere.
- **`Adopt`:** in each of the three adopting spans, `R` ends with a string
  `m` that the step's search returned as a *longest* occurring prefix:
  - (a) the first `Append` of a run without a root: `R = m`, the forward
    match of `rest`;
  - (b) `ReplaceRoot` after a forward match: `R = R_prev + m`;
  - (c) `ReplaceRoot` after an accepted probe: `R = R_prev[..o] + E`, with
    `m = E = P + rest[..k]` the longest occurring prefix of `[P] ++ rest`.

  If `R + x` occurred (`x` the next atom after `m`), then so would its
  suffix `m + x`, a longer occurring prefix of the searched query:
  contradiction.
- **`occ(G)` is unchanged** between that search and the next probe:
  - inserting `m` creates a vertex for a string that occurs (L3);
  - `find_complete` is a lookup;
  - `Adopt` edits nothing;
  - `settle` keeps `occ` (L1) and gives `R` no parent (L2; `R` is a child
    of no removed vertex in the parentless cases);
  - rejected probes earlier in the next step do not edit
    (`expand.rs`, `rejected_probe_does_not_edit`).
- A run that starts after an unknown atom has no probe at cursor 0
  (`expand`), so RM is only needed after a commit within the run. ∎

**L6 (redundancy for `U = R`).** Let `t` be a postfix of `R` whose only
parent occurrence is in `R`. Then the probe `[t] ++ rest` is rejected.

*Proof.* By L1 every occurrence of `t`'s string in a vertex `w` lies inside
an occurrence of `R` in `w` that ends where `t` ends. If `t + rest[0]`
occurred in `w`, so would `R + rest[0]`, contradicting RM. ∎

With L4 (`U ≠ R`), dissolving a postfix never changes the accepted probe,
so the redundancy lemma (§2.4) has no exception. RM also covers roots with
parents: an adopted root has parents, but its suffix `m` was a longest
match. (The earlier partial argument, "a staged token occurs ≥ 2 times",
is not needed.)

**Evidence (D1b,
[`early_commit.py`](../../../context-insert/agents/prototypes/early_commit.py),
2026-11-25).** 94 scenarios and 3544 random corpora (5269 reads, 28630
settled steps):

- RM checked on the 21185 steps whose next atom is known: 0 violations;
- 1348 steps had a root with parents;
- 492 dissolutions, 0 into the root.

**Handling:** `settle` dissolves into the root like into any other host;
no assertion. `Settled::into_root` counts these dissolutions for the
benchmarks; the per-step oracle check (§5) stays the safety net.

## 3. Changes in context-trace

### 3.1 Removal of tagged staging (task D2)

Decided 2026-11-25 (staging plan D-S1 final: overlay only). Removed:

- `positioned/stage/tagged/` (`StageId`, `TaggedHandle`, `TaggedView`,
  `TaggedVertex`, `TaggedVertexMut`, `TaggedComposition`,
  `TaggedLabels`) and `positioned/tests/stage/tagged.rs`.
- `PositionedVertex::state` and `PositionedGraph::stage`: committed
  vertices are always `Committed`; `VertexRead::state` stays (overlay
  vertices report `Staged`). The guard allowlist
  (`positioned/tests/guard.rs`) loses both fields.
- The tagged-stage checks in `OverlayHandle::open` and in committed
  pushes; tag-only helpers of `StageIndex` (`tagged`) if unused
  afterwards; `Translation`'s `Untag` form (table only).

The overlay `Stage`, `StageHandle`, `Translation` and the store traits
stay: search, insertion and the reader remain written against them.

### 3.2 Stage-only removal

Only stages remove vertices; the committed store has no removal path at
the type level.

```rust
mod raw {
    pub trait Remove: Write<Index = StageIndex> {
        /// Precondition (checked by GraphEdit): staged, no links.
        /// Swap-remove; renames the moved vertex in every reference
        /// before returning (§2.2).
        fn remove(&mut self, seal: Seal, index: StageIndex) -> Option<Moved>;
    }
}

pub trait Stage: GraphWrite<Index = StageIndex> + raw::Remove {
    /// The staged vertices in slot order.
    fn staged(&self) -> impl Iterator<Item = StagedIndex> + '_;
}

/// A staged vertex moved from slot `from` to slot `to` by a removal.
pub struct Moved { pub from: StageIndex, pub to: StageIndex }
```

### 3.3 `GraphEdit` operations and candidates

```rust
impl<S: Stage + ?Sized> GraphEdit<'_, S> {
    /// §2.2. Staged composite ≠ root with exactly one parent occurrence.
    pub fn dissolve(&mut self, t: StageIndex, root: Token<StageIndex>)
        -> Result<Removal, PositionedError<StageIndex>>;
    /// Staged composite ≠ root without parent occurrences.
    pub fn discard(&mut self, t: StageIndex, root: Token<StageIndex>)
        -> Result<Removal, PositionedError<StageIndex>>;
    /// §2.3: worklist over `candidates`; renames `root` on a move.
    pub fn settle(&mut self, candidates: Candidates, root: &mut Token<StageIndex>)
        -> Settled;
}

/// §2.3.1: collected by the reader during one step.
#[derive(Default)]
pub struct Candidates(Vec<StageIndex>);
impl Candidates {
    /// Children of removed occurrences and created tokens of `publication`.
    pub fn record(&mut self, publication: &Publication<StageIndex>);
    /// A vertex that lost its exemption (the previous root).
    pub fn push(&mut self, vertex: StageIndex);
}

pub struct Removal {
    /// `dissolve`: the host; `discard`: none.
    pub host: Option<StageIndex>,
    /// Children of the removed vertex: their counts decreased.
    pub children: Vec<StageIndex>,
    pub moved: Option<Moved>,
}

/// Counts for tests and benchmarks.
pub struct Settled {
    pub candidates: usize,
    pub dissolved: usize,
    pub discarded: usize,
    pub into_root: usize,
}

pub struct Publication<I> {
    pub created: Vec<Token<I>>,
    pub edits: Vec<VertexEdit<I>>,
    pub removed: Vec<I>,      // new, in order
    pub moved: Vec<Moved>,    // new, in order (stage edits only)
}
```

- New `PositionedError` variants: `ParentCount { vertex, count }`,
  `IsRoot(vertex)`; `NotStaged` (committed `t`) and `Atom` exist.
- **Bookkeeping:** a removal deletes `t` from `touched`, `edits` and
  `created` (otherwise `finish()` → `validate_touched()` fails on the
  missing vertex) and records it in `removed`; `Moved` renames the entries
  of `created`, `edits`, `touched` and the worklist.
- `dissolve` updates `U` through the existing `recompose` (links of the
  host) and unlinks `(t, a)` for every child of `t`.

### 3.4 Commit and checks

- `StageHandle::commit(root)`: signature unchanged; `root` is the input.
  Panics if the committed vertex set is invalid: a staged composite other
  than `root` with fewer than two parent occurrences (§2.1; a bug, so no
  `CommitError` variant). No other runtime G6 check.
- `PositionedGraph::validate_retention(&self, inputs: impl
  IntoIterator<Item = CommittedIndex>) -> Result<(), PositionedError>`
  (`test-api`): G6 on every committed vertex, with the inputs given by the
  caller. Separate from `validate()` (G1–G5), which the insert gate also
  uses.
- No `VertexRead::is_input`, no input field (§2.1).

## 4. Overlay store changes

| Part | Change |
|---|---|
| New atoms | **committed immediately** (decided 2026-11-25): `OverlayHandle::atom(value) -> Token<StageIndex>` write-locks the base between views, creates the atom through the committed store's `GraphEdit::atom` and advances the stage's `base_generation` by its own bump, so `BaseChanged` still detects foreign edits. Atoms are stable from creation on; the stage holds no atoms (`StagingGraph::atoms` removed), only composites. A physical early commit would reuse this base-write mechanism (§5.1, later). |
| `StagingGraph::vertices` | dense, slot order; removal by `swap_remove` |
| Renaming the moved vertex `l` | parent links of its children (staged: `StagedVertex::parents`; committed, including atoms: `BaseDelta::added`) and compositions of its parents (staged: node; committed: `BaseDelta::composition`); `O(|comp(l)| + p(l))` |
| Dissolving into a committed host | copy-on-write composition and link deltas (exist, used by insertion) |
| `commit_into` | unchanged (vertices are dense, no atoms), plus the check of §3.4 |

Committed vertices are never removed; `remove` exists only on the stage
(`raw::Remove`). An overlay view holds a read guard of the base, so atoms
are created between views: `segments` creates them before any search; a
streaming reader creates them per character, also between steps.

## 5. Reader integration (context-read)

- `ReadStage<'g> { handle: OverlayHandle<'g>, root }`: `StagingStrategy`,
  `OverlayStaging`, `TaggedStaging`, `DefaultStaging` and `read_with`
  are removed (D2); `read` and `read_traced` stay the public interface.
- **Segments:** new atoms are committed at once through
  `OverlayHandle::atom` (§4); a `Segment` holds committed atoms.
- **Edit records:** `forward` and `expand` return the `Publication` of
  their insertion (`InsertOutcome::Created`) next to the span instead of
  dropping it; `ReadStage::commit` returns the record of its `GraphEdit`
  and reports the previous root when the root changes. `read_steps`
  collects them per step in `Candidates` (§2.3.1).
- **`settle` after every step:** `ReadStage::settle(candidates)` for every
  span, including `Adopt`; it opens its own `GraphEdit` on the stage,
  calls `edit.settle(candidates, &mut root)` and finishes the edit.
- `finish`: `handle.commit(root)` (panics on an invalid vertex set).
- `insert` outside a read (insert gate only): no stage, no `settle`, no
  G6 check (§2.1).
- Tests:
  - `context-read/tests/reader_behavior.rs`: remove the 9
    `#[ignore = "provisional retention (reader plan §5)"]`, the
    `READ_DEVIATIONS` mechanism, the `strategies!` macro (one instance per
    scenario) and `strategies_commit_identical_graphs`.
    `validate_retention` after every read: in the scenario lane with the
    tokens returned by the reads so far; in the read lane with the base
    corpus mapped to tokens by `from_interchange_named` plus the returned
    token.
  - Per-step checks: after every `settle`, G1–G5 (`validate()` through the
    stage view) and (S) by a full scan (debug/`test-api` assertion,
    §2.3.1). Oracle equality per step on a **step lane**: oracle fixtures
    of `C + [I[..cursor]]` for every cursor of the 9 entry scenarios,
    `insert_deep_borders__1` and the `period_*` family (corpus = the base
    inputs plus the consumed prefix; no extra inputs).
  - Unit tests (context-trace): `dissolve`/`discard` preconditions
    (committed `NotStaged`, atom `Atom`, root `IsRoot`, wrong count
    `ParentCount`), postconditions (`validate()`, links moved,
    `Publication::removed`/`moved`, committed host via its delta),
    swap-remove renaming (moved vertex with staged and committed children
    and parents), `Candidates::record` (removed children, created tokens,
    cancelled re-adds), `settle` cascades, `OverlayHandle::atom` (atom
    committed, generation, `BaseChanged` on a foreign edit), `commit`
    panics on an invalid vertex set.

### 5.1 Early commit: stable tokens (task D7, after D1b)

Decided earlier: commit at the input end first; then staged tokens become
final as early as possible, as soon as no continuation of the input can
dissolve them. Revision 3.2 states the rule, its graph-local form and the
interface. One decision is open: whether "final" means a stability mark in
the stage or a physical write to the base (E-I below).

**Definition (safe).** Take a staged composite `t ≠ R` at a settled step.
Its contexts are read over the committed inputs and the consumed prefix
`I[..cursor]`:

- a left context is the atom before an occurrence, or a distinct "input
  start" mark;
- a right context is the atom after an occurrence, or a distinct "input
  end" mark for an occurrence ending a finished input;
- the occurrence ending at the cursor has no right context yet.

`t` is **safe** if it has ≥ 2 distinct left contexts and ≥ 2 distinct
fixed right contexts. This is the maximal-repeat condition of §2.7 with
the cursor occurrence left out.

**E1 (safe tokens are never dissolved).** Contexts of existing occurrences
never change. A read only adds occurrences, and fixes the cursor
occurrence's right context when the next atom or the end comes. So a safe
`t` stays a maximal repeat of every later prefix `C + [I[..c']]`, of
`C + [I]` and, by M5, of every later corpus. By M4 it is a vertex of every
later settled stage, and by M2 it has p ≥ 2 there, so `settle` never
removes it.

**E2 (the rule is exact).** If a settled `t` is not safe, it is still a
maximal repeat (M2). Its right-branching then needs the cursor occurrence,
so it has exactly one fixed right context `a`. If the input continues with
`a` and ends, `t`'s only right context is `a`: `t` is no maximal repeat of
`C + [I]` and is dissolved. No rule can make `t` final earlier without
risking that.

E1 and E2 rest on M2, M4 and M5 (D1b proofs).

**Graph-local form.** The *chain* is the root's postfix chain: the tokens
with an occurrence ending at the cursor. Such an occurrence lies inside
`R`'s last child, recursively.

- **Off the chain:** every settled staged composite `t ≠ R` that is not on
  the chain is safe. All its contexts are fixed, and it is a maximal
  repeat now (M2).
- **On the chain:** `t` is safe iff the parent-occurrence walk finds ≥ 2
  fixed right contexts. The walk, stopping at 2:
  - a parent occurrence `(U, o)` with `o + |t| < |U|` contributes the atom
    at `o + |t|` in `U`;
  - one at the end of `U` inherits the contexts of `U`'s occurrences
    (recurse into `U`'s parents);
  - for `U = R`, the cursor contributes nothing;
  - a parentless committed `U` is an input and contributes its end.
- **Committed inputs with parents** cannot be recognised (no input mark,
  §2.1, decision F), so the walk misses their end. This is conservative:
  the token becomes safe one step later, when it leaves the chain.
- **Per-step work:** only staged tokens that are not stable yet. These are:
  - the chain;
  - tokens created in the step;
  - the previous step's chain (its cursor occurrence just got a context).

  The other non-stable tokens are safe as soon as they are settled and off
  the chain. A debug full scan checks this, like (S) in §2.3.1.

**Evidence (D1b,
[`early_commit.py`](../../../context-insert/agents/prototypes/early_commit.py),
2026-11-25; 5269 reads, 28630 steps):**

| Check | Result |
|---|---|
| Off-chain settled staged tokens not safe | 0 / 13845 |
| Chain tokens: walk vs definition | 0 differences (15060; 1750 safe) |
| Chain tokens missed without input marks (conservative) | 11 |
| Safe tokens dissolved later | 0 of 7168 |
| Safe tokens missing from the final oracle | 0 |
| Physical base view `committed + early` (scenarios): G4 / G5 | 0 / 67 violations |
| Physical base view: early tokens with p < 2 (G6) | in 67 / 67 |

**Physical commit vs. stability.** Writing safe tokens into the base while
the read runs meets three obstacles:

1. **G6.** A safe token's occurrences are typically all in the consumed
   prefix, i.e. inside the open root, which is not committed. In the base
   it then has p < 2: every one of the 67 checked base views violates G6,
   which contradicts decision D (committed vertices satisfy G6).
2. **Closure.** Committed vertices may only reference committed children.
   A physical commit must therefore take `t` with all its staged
   descendants, which may be unsafe chain tokens (292 blocked
   step-instances in the prototype; e.g. `abc = [ab, c]` while `ab` is a
   postfix of the root). It must also apply and project the deltas of the
   committed hosts of `t`.
3. **Consumers.** Only other readers would see the base early, and there
   are none until reads run in parallel (`Sync` handle, §6).

None of this applies to a **stability mark**. A stable token is never
dissolved. Like any committed vertex in a later read, its composition may
still change: insertion can add staged children, and dissolution can
remove them again. So stability needs no closure: stable = safe.

**Interface (proposed, decision E-I).**

- **Stage (context-trace):**
  - stable vertices occupy a slot prefix `[0, s)`;
  - `raw::Stabilize::stabilize(seal, t) -> Option<Swap>` swaps `t` into
    slot `s` and renames both vertices, like `Moved`;
  - `raw::Remove` refuses slots `< s` (a bug: panic). So the store
    enforces "stable tokens are never removed", and a stable token's
    `StageIndex` stays fixed until commit;
  - `Stage::stable(&self) -> impl Iterator<Item = StagedIndex>`.
- **`GraphEdit::stabilize(t, root) -> Result<Option<Swap>, PositionedError>`:**
  vertex-local with strict checks:
  - staged, composite, `t ≠ root`, p ≥ 2;
  - already stable is a no-op.

  The rule itself is reader logic (chain, cursor, walk) and lives in
  context-read. `Publication::moved` becomes `Vec<Rename>`, with
  `Rename::{Moved, Swapped}`. `Candidates` skips stable vertices.
- **Reader:**
  - `ReadStage::stabilize()` runs after `settle` in every step;
  - `Step` gains `stabilized: usize`, and `Settled` is reported per step;
  - `read_traced` additionally returns, per committed token, the step at
    which it became stable (translated at commit), for tests and
    benchmarks (latency: steps between creation and stability).
- **Root, atoms, commit:** the root is never stable (it is the input).
  Atoms are committed at once (§4). `commit(root)` is unchanged: the
  stable prefix is committed with the rest.
- **Later (parallel reads):** a live streaming callback (the stage view
  plus the newly stable tokens of a step), and a physical flush of the
  stable prefix. The flush needs a G6 notion for open reads, which counts
  occurrences in the open root as a pending input, plus closure and delta
  projection. It belongs with the `Sync` `OverlayHandle`.

## 6. Evaluation

Correctness first; performance is measured, not optimized, in this plan
(decided 2026-11-25).

- Benchmarks rerun with dissolution and recorded (no baseline
  comparison; the `pre-staging` baseline is dropped): cost of `settle` per
  read, dissolutions per read, candidates per step, cascade depth;
  after D7 the stability latency (steps from creation to stability).
- Expected asymptotics: `settle` is `O(candidates)` per step (§2.3.1), a
  removal `O(degree)` (§2.2).
- Deferred to a later profiling and optimization plan: the per-read
  overlay overhead (staging plan §11 S4/S6: open, lookups, commit, link
  checks), rebuilding parent maps at commit, a `Sync` `OverlayHandle`
  (only needed for parallel reads).
- `insert_gate` unchanged.

## 7. Decisions and open questions

**Decided 2026-11-25:**

| | Decision |
|---|---|
| A | Tagged staging is removed; this plan removes it (§3.1, D2). |
| B | Only staged vertices can be removed; removal is reconciled immediately (swap-remove with renaming, `Moved`); no tombstones (§2.2). |
| C | `settle` is eager (end of every step) and local, for streamability (§1, §2.3). |
| D | Staged vertices are checked by `settle` and dissolved or discarded (§2.3). |
| F | No stored input mark and no `VertexRead::is_input`: the stage root is the only staged input; tests pass committed inputs explicitly (§2.1). |
| G | Staging plan D-S1 is final: overlay only; `StagingStrategy` and `read_with` go with it. |
| H | Performance is not a priority: profiling and optimization later (§6); `Sync` `OverlayHandle` later; the `pre-staging` benchmark baseline is removed. |
| Q1 | Candidates come from the step's edit records (insertion and root commit `Publication`s) plus the previous root; exact by (S) (§2.3.1). No store-side recording. |
| Q2 | The only runtime G6 check is at commit: committing an invalid vertex set panics. Committed vertices without parents are inputs; already committed vertices are checked only by `validate_retention` in tests (§2.1). |
| Q3 | Dissolution into the root is allowed: by RM the probe of such a postfix is always rejected, for roots with and without parents (§2.8, proven). |
| Q4 | New atoms are committed immediately and stay stable; the stage holds only composites (§4). |

Earlier decisions: commit at input end first, early commit afterwards
(§5.1); standalone insertion without a stage; only reads create inputs.

**Resolved 2026-11-25 (revision 3.2):**

- Roots with parents (§2.8): Lemma RM (the root ends with a longest match
  or occurs nowhere else) and L6 prove that dissolution into any root
  removes only a rejected probe. `Settled::into_root_with_parents` is
  dropped.
- Early-commit rule (§5.1): a token is final as soon as it is safe
  (maximal repeat with fixed contexts); graph-local: off the root's
  postfix chain, or ≥ 2 fixed right contexts by the parent walk. Sound
  (E1) and exact (E2), modulo the D1b proofs of M2, M4, M5.

**Open:**

- **E-I (early-commit interface, §5.1):** a stability mark in the stage
  (stable slot prefix, `GraphEdit::stabilize`, reported per step; no base
  write during the read; proposed), or a physical commit into the base
  while the read runs (needs G6 for open reads, closure over staged
  descendants and delta projection; only useful with parallel reads).

## 8. Tasks

D2 is behavior-neutral and can start at once; D1b runs in parallel.

| Task | Content | Exit |
|---|---|---|
| D0 **Done** | Prototype: dissolution simulation under `context-insert/agents/prototypes`; eager `settle` in `read_legacy_port.py`, per-step comparison with the oracle of the prefix | §0 numbers reproduced; per-step check confirmed |
| D1 **Done** | Proofs (§2.6); G6 as a fixture check in `fixture_validation` (all lanes) | written; G6 test green |
| D1b | Part done (2026-11-25): `early_commit.py` (RM, early-commit rule by definition vs graph-local form, stability of safe tokens, base-view check; §2.8, §5.1) and the RM / L6 proofs. Remaining: prototype `maximal_repeats.py` (oracle = atoms ∪ inputs ∪ MR on all scenario prefixes and random corpora); the candidate set of §2.3.1 against a full scan on every step; proofs M1–M5 and C1 | prototype green; statements written |
| D2 | Remove tagged staging (§3.1), `StagingStrategy`, `read_with`, the `strategies!` macro and the strategy comparison test, the tagged benchmark ids and the `pre-staging` baseline instructions | gate (§9) green with the S4 test set (overlay only); reader output unchanged (same 9 ignored, same `READ_DEVIATIONS`) |
| D3 | Overlay: new atoms committed immediately (`OverlayHandle::atom`, `segments`), `raw::Remove` (swap-remove with renaming, `Moved`), slot-order docs | unit tests (§5); all gates green; reader output unchanged |
| D4 | `GraphEdit::dissolve`/`discard`/`settle`, `Candidates`, `Removal`, `Settled`, `Publication::removed`/`moved`, errors, bookkeeping; `commit` panics on an invalid vertex set; `validate_retention` | unit tests (§5) |
| D5 | Reader: edit records from `forward`, `expand` and `commit`; `settle` after every step with root renaming; un-ignore the 9 scenarios, remove `READ_DEVIATIONS`; `validate_retention` after every read; per-step checks and the step lane fixtures | gate (§9) |
| D6 | Benchmarks recorded (§6) | numbers recorded here |
| D7 | Early commit (§5.1, after D1b and decision E-I): proposed form: stable slot prefix (`raw::Stabilize`, `Rename::Swapped`, `raw::Remove` refusing stable slots), `GraphEdit::stabilize`; `ReadStage::stabilize` after every `settle` (off-chain rule, parent walk on the chain); `Step::stabilized`, stability step per token in `read_traced`; debug full scan of the rule | gate unchanged; no stable token removed (store-enforced); after the last step of an input, every staged token but the root is stable or on the root's postfix chain; stability latency recorded |

## 9. Gate

```sh
cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api
cargo test -p context-read --no-default-features --features test-api
cargo test -p context-trace -p context-search -p context-insert --no-default-features --features test-api
cargo test --release -p context-insert --no-default-features --features test-api insert_gate
cargo test -p context-read --features legacy-tests
cargo test -p context-trace -p context-search -p context-insert --features legacy-tests
cargo build --workspace --all-targets --exclude context-editor-kernel --exclude sandbox-app
cargo bench -p context-read --no-default-features --features test-api   # recorded, no baseline
```

- Every scenario and read-lane case equals its oracle graph; none ignored
  for provisional retention; `READ_DEVIATIONS` removed.
- Every step of the step lane equals its oracle graph (unconsumed atoms
  ignored).
- `validate()` (G1–G5) after every read and every insert case;
  `validate_retention()` (G6) after every read.
- Insert gate unchanged (all cases).
