---
tags: `#plan` `#context-read` `#context-trace` `#positioned-composition` `#reader` `#retention` `#dissolution`
summary: Dissolution of provisional tokens for the positioned reader (reader plan phase R5, roadmap step 5). A token created by the running read can lose its token status later in the same input (all its occurrences end up inside one larger token; `abcabca` keeps `abc` today). Retention invariant G6 (every committed vertex that is neither an atom nor an input has ≥ 2 parent occurrences; holds on 1630/1630 oracle graphs). Staged tokens with a single parent occurrence are dissolved into their parent (`GraphEdit::dissolve`, worklist `settle` after every root commit); `commit` asserts G6; early commit follows after its proof. Built on the staging graph (staged vs committed vertices), which must be implemented first. Fixes the 10 provisional-retention deviations (offline simulation: 94/94 scenarios, 140/140 read-lane cases equal the oracle).
status: 📋 draft revision 2.2 — maximal-repeat characterization (§2.7, D1b); inputs only from reads; D0 and D1 done (prototype: 199/199 reads and 701/701 steps equal the oracle; proofs §2.6, open case U = root); after the staging graph (20261124_PLAN_STAGING_GRAPH); decided: eager settle, commit at input end first, early commit after its proof, standalone insertion without stage
date: 2026-11-24
parent: 20261122_PLAN_POSITIONED_READER.md (§5 provisional retention, phase R5)
depends_on: 20261124_PLAN_STAGING_GRAPH.md (implemented), 20261122_PLAN_POSITIONED_INSERT.md (revision 3, G1–G5, GraphEdit D8, no rollback D10)
---

# Plan: Dissolution of provisional tokens

**Reader:** [20261122_PLAN_POSITIONED_READER](20261122_PLAN_POSITIONED_READER.md)
(§5 provisional retention, phase R5).
**Prerequisite:** [20261124_PLAN_STAGING_GRAPH](20261124_PLAN_STAGING_GRAPH.md)
(vertex states, store traits, both stage implementations, `ReadStage`;
behavior-neutral). This plan is the first one that changes the reader's
output.
**Insertion:** [20261122_PLAN_POSITIONED_INSERT](20261122_PLAN_POSITIONED_INSERT.md)
(invariants G1–G5 §3.1, `GraphEdit` D8, no rollback D10).

## 0. Problem

The reader inserts every accepted match at once (legacy rule). A token that
a prefix of the input makes frequent can lose that status later in the same
input: all its occurrences end up inside occurrences of one larger token,
and the oracle does not retain it.

Example `abcabca`: after `abcabc`, `abc` is a border of the input read so
far and correctly a token (root `[abc@0, abc@3]`). The final `a` shifts
the border chain to `abca`, `abcabca`. Every occurrence of `abc` now lies
inside `abca`; the oracle has `abca = a@0, b@1, c@2, a@3` and no `abc`.

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
| Dissolutions / into the current root (open case of §2.6) | 16 / 0 |
| Random inputs (one-off: all `{a,b}` strings of length 2–10, 1500 random `{a,b,c}` strings, 600 input pairs) | 4744 reads: decisions identical, all equal the oracle; 548 dissolutions, 0 into the root |

The G6 fixture check (D1, `every_oracle_graph_satisfies_retention` in
`context-read/tests/fixture_validation.rs`) covers 2339 oracle graphs of
the v3, insert and read lanes: 0 violations.

## 1. Goals and non-goals

Goals:

- Every read yields `oracle(C + [I])`: no expected deviations, no tests
  ignored for provisional retention, `READ_DEVIATIONS` empty.
- G6 as a checked invariant of every committed graph (`validate()`,
  `commit`).
- Dissolution is one strict `GraphEdit` operation (D8) with written
  preconditions, implemented once for both stage implementations.

Non-goals:

- No change to the reader loop, search or insertion.
- No change to the staging traits beyond the additions of §3.
- No rollback (insert plan D10): dissolution is a forward edit, not an
  undo; committed vertices keep their content (staging plan §2.1).

## 2. Model

### 2.1 Retention invariant G6

> **G6.** Every committed vertex that is neither an atom nor a corpus input
> has at least two parent occurrences (entries `(parent, offset)`).

A vertex with a single parent occurrence `(U, o)` occurs only inside `U`
at offset `o`, so its string has no context of its own; the oracle does not
retain it. G6 holds on every oracle graph (§0). G6 is a graph invariant
next to G1–G5 (insert plan §3.1).

G6 needs an `input` mark on vertices. An **input** is the whole string of
one `read` call (one corpus entry); `read` is the only public interface of
the context stack, so only reads create inputs (decided 2026-11-24). The
mark is set in two places:

- `StageHandle::commit(root)` marks the read's final root. This is the only
  production path; there is no general-purpose `mark_input` on
  `GraphEdit` (D8).
- The fixture loaders (`test-api`) mark every corpus input of a loaded
  oracle graph, since those inputs were read: the manifest already lists
  the corpus strings, and each one is located by a structural search
  (`find_ancestor` of its atoms; no content index, no new fixtures). The
  oracle interchange has no mark, so `diff` ignores it.

Insertion never marks. `insert`, `insert_match` and `insert_next_match`
are internal building blocks of the reader; a token they create is an
ordinary token. The insert gate (`insert` outside a read, expecting
`oracle(C + [query])`) is a test of the insertion algorithm, not a public
use: its graphs satisfy G1–G5, and G6 is not checked there (the query is
not a read input).

Staged vertices may violate G6 (provisional). Only staged vertices can be
dissolved, so G6 on committed vertices is never repaired, only checked.

### 2.2 Dissolution

```text
dissolve(t):   precondition: t staged, not an atom, not the stage root,
               exactly one parent occurrence (U, o)
  comp(U) := maximal(comp(U) − t@o ∪ (comp(t) + o))
  links of t's children: (t, a) → (U, o + a), dropped where filtered by maximal
  remove t
```

- `U` may be staged or committed. A committed `U` only changes its
  composition shape, not its content (allowed by staging plan §2.1).
- **G2 after dissolution.** `comp(t)` is complete inside `t`; occurrences
  crossing `t@o` are already in `comp(U)`; `maximal` removes occurrences
  that are now inside another one. Proof: §2.6, T1.
- **G4** by the intersection lemma (reader plan §3.4).
- **Zero parent occurrences** (a staged vertex that lost its only parent
  occurrence through a dissolution): removed without substitution
  (`discard`).

### 2.3 `settle`

```text
settle(stage, root):
  while some staged t ≠ root has ≤ 1 parent occurrence:
    if 0: discard(t) else: dissolve(t)
```

- **Termination:** each step removes one vertex.
- **Cascades:** a child of `t` can lose an occurrence through `maximal`;
  `Dissolution::children` returns the vertices whose parent count changed,
  which become the next worklist entries.
- **Order:** the result does not depend on the order (§2.6, T2); the
  simulation agrees (§0).
- **Eager (decided):** runs after every root commit of the reader, so the
  stage holds no G6 violation between steps (except the root).

### 2.4 Decisions never depend on dissolved tokens

**Lemma (redundancy).** A staged token `t` with a single parent occurrence
`(U, o)` occurs only inside `U` at offset `o`. Dissolving it does not change
any reader decision:

| Decision | Depends on | Effect of dissolving `t` |
|---|---|---|
| forward `insert_next_match(rest)` | the string of the longest occurring prefix | none; at most `Complete` becomes `Created` |
| probe `[P] ++ rest`, `P = t` | `t` is a postfix of the root | `U` ends where `t` ends and is probed first; `[t] ++ rest` matches only if `[U] ++ rest` does |
| `find_complete([R, t'])` | existence of `Y` | if `Y` was dissolved, a new root with the same string is created |
| root, `P`, `E` within one step | identities | `settle` runs only between steps; the root is exempt; `E` has ≥ 2 parent occurrences after the commit |

So the reader may hold only the root across `settle`, and decisions rely on
strings, never on staged identities. Proof and its open case: §2.6, L3–L5.

### 2.5 Walkthrough `abcabca`

| Step | Graph (`S` = staged) |
|---|---|
| segments | `a`, `b`, `c` committed |
| unknown `a`, `b`, `c` | root S1 `ab`, extended in place to `abc` |
| forward `abc` | finds S1 itself (no edit), `t = R`; new root S2 `[S1@0, S1@3]` |
| probe `[S1, a]` | matches `S1@0, a@3` in S2; insertion creates S3 `abca = [S1@0, a@3]`; S2 `[S3@0, S1@3]` |
| commit `Expand` | `extend(S2, S3@3)`: S2 `[S3@0, S3@3]` |
| `settle` | S1 has one parent occurrence `(S3, 0)`: dissolve, S3 `[a@0, b@1, c@2, a@3]` |
| `handle.commit(S2)` | S2 input; S3 committed (2 parent occurrences); G6 holds; equals the oracle |

Without this plan (staging graph only) the `settle` row is missing and S1
`abc` is committed with a single parent occurrence: today's deviation.

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

**Open case of the redundancy lemma (`U = R`).** If the single parent
occurrence of `t` is in the root itself, L4 does not apply: the root is not
probed, so without dissolution `t` (the root's last child) is the first
probe, while with dissolution the next smaller postfix `P'` is probed. Both
accept if `t + rest[0]` occurs (then `P' + rest[0]` occurs too), but the
expansion of `P'` may be longer, since `P'` can occur outside `R`. The
decision sequences can then differ. D0 never observed this case: all 16
fixture dissolutions and 548 random ones go into a fresh expansion token,
never into the current root, and all decisions are identical (§0). A proof
that the case cannot arise (a staged token whose only occurrence is in the
root would have been dissolved by an earlier `settle` or would occur
outside the root) is open (Q3); D4 adds a debug assertion
(`Dissolution::host != root`) to catch it.

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
dissolves.

**Statements to prove (D1b):**

| # | Statement | Consequence |
|---|---|---|
| M1 | `oracle(C) = atoms ∪ inputs ∪ MR(C)` | correctness of every operation reduces to its vertex set (G1 + G2 determine the graph) |
| M2 | In a graph whose vertex set is `atoms ∪ inputs ∪ MR`, a vertex that is not an atom or input has `p ≥ 2` iff it is a maximal repeat (one direction is L1) | G6 holds on oracle graphs; G6 violations are exactly non-maximal staged tokens |
| M3 | G4 and G5 for such vertex sets: the overlap of two overlapping maximal repeats in a vertex is a vertex; every non-vertex string has a unique minimal extension to a vertex | G4/G5 without reference to the oracle |
| M4 | One reader step plus eager `settle` maps the vertex set of `C + [I[..c]]` to that of `C + [I[..c']]`: new maximal repeats are the inserted tokens and the root; lost ones are those right-branching only through the cursor, removed by `settle` | per-step oracle equality (D0: 701 / 701); closes the open case of §2.6 (a token whose only occurrence lies in the root is not a maximal repeat, so an earlier `settle` removed it); the early-commit rule (§5.1) |

Approach: M1 by comparing the oracle's retention rule (`ngrams`) with the
branching conditions; M2 from L1 and its converse (two occurrences with
different left or right contexts cannot both lie inside one `U` at a
common offset); M3 from the closure properties of maximal repeats; M4 by
case analysis of the commit table (reader plan §3.4) on the contexts that
change when the cursor moves.

## 3. Additions to the staging traits (context-trace)

All additions extend the traits of staging plan §3; no signature there
changes.

```rust
pub trait VertexRead<'a>: Copy {
    // … staging plan §3.3
    fn is_input(self) -> bool;
}

mod raw {
    pub trait Write: super::GraphRead {
        // … staging plan §3.5
        /// Precondition: no links (checked by GraphEdit).
        fn remove(&mut self, i: Self::Index);
    }
}

impl<'g, S: GraphWrite + ?Sized> GraphEdit<'g, S> {
    /// §2.2. Staged, composite, not the root, exactly one parent occurrence.
    pub fn dissolve(&mut self, v: S::Index, root: Token<S::Index>)
        -> Result<Dissolution<S::Index>, PositionedError>;
    /// Removes a staged, parentless vertex that is not the root.
    pub fn discard(&mut self, v: S::Index, root: Token<S::Index>)
        -> Result<(), PositionedError>;
}

pub struct GraphEdit<'g, S: GraphWrite + ?Sized> {
    // … staging plan §3.5
    removed: Vec<S::Index>,
}
// Publication gains `removed`.

pub struct Dissolution<I> {
    pub host: I,
    /// Children whose parent count changed: the next `settle` candidates.
    pub children: Vec<I>,
}

/// §2.3 worklist.
pub fn settle<S: Stage + ?Sized>(edit: &mut GraphEdit<'_, S>, root: Token<StageIndex>);

pub enum CommitError {
    BaseChanged { opened: u64, now: u64 },
    Retention(StageIndex),                 // new: G6
}
// new PositionedError variants: ParentCount { vertex, count }, IsRoot.
// NotStaged and Atom exist since the staging plan.
```

- `dissolve` matches `NodeRef::Composite` and returns
  `PositionedError::Atom` otherwise.
- `StageHandle::commit(root)` (signature unchanged) additionally marks the
  root input and asserts G6 on every vertex it commits.
- `validate()` checks G6 on committed vertices of every store.

## 4. Store changes

| | (a) overlay | (b) tag |
|---|---|---|
| `input` mark | field of `StagedVertex`; set on the translated root at commit | `PositionedVertex { input: bool }`; set at commit |
| Removal of staged vertices | `vertices: Vec` → `Slab` (removal frees the slot; `StagedIndex` stays stable) | `vertices: Vec<Option<PositionedVertex>>` (tombstones); `vertex()`, `vertices()`, interchange, `diff` and `validate()` skip removed slots |
| Dissolving into a committed host | copy-on-write composition override and link deltas (already used by insertion) | in place |
| Commit | skips removed slots when building the `Translation` | tombstones stay (no compaction planned) |

Committed vertices are never removed, so the committed store
(`PositionedGraph` outside a stage) needs no removal path; `remove` on a
committed index is rejected by `GraphEdit` (`NotStaged`).

## 5. Reader integration (context-read)

- `ReadStage`: `settle(edit, root)` after every root commit (reader plan
  §3.4), in the same `GraphEdit` as the commit.
- `read_with`: `handle.commit(root)` marks the root and asserts G6; with
  eager `settle` nothing is left to dissolve.
- `insert` outside a read (insert gate only): no stage, no `settle`, no
  input mark, no G6 check (§2.1).
- Tests:
  - `context-read/tests/reader_behavior.rs`: remove
    `#[ignore = "provisional retention (reader plan §5)"]` from the 9
    scenarios of §0.
  - Read lane: `READ_DEVIATIONS` becomes empty (remove the mechanism).
  - Unit tests: `dissolve` preconditions (committed vertex, atom, root, 0
    or ≥ 2 parent occurrences rejected), postconditions (G1–G5 via
    `validate()`, links moved, `removed` published), `settle` cascades,
    `commit` rejects an G6 violation.
  - A per-step check: after every step (eager `settle`) the stage equals
    `oracle(C + [I[..cursor]])` on the periodic scenarios (confirmed by
    D0 on every step of every scenario), ignoring atoms of the current
    unknown segment that are not consumed yet.

### 5.1 Early commit (phase 2, after D1 and D4)

Decided: first the simple boundary (commit at input end, D0–D5); once that
is proven and green, staged tokens are committed **as early as possible**,
as soon as they can no longer violate G6.

- **Rule (agreed; proof in D1b):** commit a staged token `t` as soon as
  it is a maximal repeat (§2.7) with **fixed** contexts only: its
  occurrences have ≥ 2 distinct left contexts and ≥ 2 distinct right
  contexts, where a right context is fixed if the next atom is already
  consumed or the occurrence ends a finished input. The occurrence ending
  at the read cursor does not count (its right context is not read yet).
  Left contexts never change and new input only adds occurrences, so such
  a `t` stays a maximal repeat, never reaches `p(t) ≤ 1` (§2.7, M2), and
  is never dissolved. "≥ 2 parent occurrences now" is not enough: in
  `abcabc|`, `abc` has two, but its only fixed right context is `a`, and a
  following `a` dissolves it.
- **Closure:** a committed vertex may only reference committed children.
  In (a) the base cannot store a `StagedIndex`, so an early commit commits
  `t` together with all its staged descendants, children first.
- **Interface:** `StageHandle::commit_early(&self, vertices)` returning a
  partial `Translation`; the reader then has to translate held tokens (only
  the root, which is never committed early).
- **Benefit:** smaller stage, less overlay indirection on long inputs;
  measured with the §6 benchmarks.

## 6. Evaluation

- Benchmarks of the staging plan (§7.1 there) rerun with dissolution: cost
  of `settle` per read, dissolutions per read, cascade depth, for both
  strategies. The staging plan's numbers are the baseline.
- `insert_gate` unchanged.

## 7. Open questions

1. ~~Commit boundary~~ — decided: input end first, early commit afterwards
   (§5.1).
2. ~~Eager or deferred `settle`~~ — decided: eager (§2.3).
3. **Proofs:** written in §2.6 (L1 locality, T1 G1–G5 after `dissolve`,
   L2 monotonicity, T2 confluence, L3–L5 redundancy). Open, planned in
   D1b (§2.7): the maximal-repeat characterization M1–M4, from which the
   redundancy lemma's open case (parent = root), G6 against the oracle and
   the early-commit rule (§5.1) follow.
4. ~~`input` mark API~~ — decided: only `read` creates inputs; the mark is
   set by `StageHandle::commit` and by the `test-api` fixture loaders;
   insertion never marks (§2.1).

## 8. Tasks

Start after the staging plan's task S4 (the reader on `OverlayStaging`);
the (b) parts follow the staging plan's S5. D0 and D1 have no code
prerequisites and may run earlier.

| Task | Content | Exit |
|---|---|---|
| D0 **Done** | Prototype: dissolution simulation under `context-insert/agents/prototypes`; eager `settle` in `read_legacy_port.py`, per-step comparison with the oracle of the prefix | §0 numbers reproduced; per-step check of §5 confirmed |
| D1 **Done** (open case: Q3) | Proofs (§2.6); G6 as a fixture check in `fixture_validation` (all lanes) | written; G6 test green |
| D1b | Maximal-repeat characterization (§2.7): prototype `maximal_repeats.py` (oracle = atoms ∪ inputs ∪ MR on all scenario prefixes and random corpora; MR-based early-commit rule in the reader port: how early tokens commit, assert that no early-committed token is dissolved later); proofs M1–M4 | prototype green; M1–M4 written; §5.1 rule confirmed |
| D2 | Store removal and `input` mark (§4), (a) first; `VertexRead::is_input`, `raw::Write::remove`; input marking in `commit` and the fixture loaders (§2.1); `validate_retention()` (G6) as a separate check (insert-gate graphs are not checked for G6, §2.1) | unit tests; all staging gates still green (no reader change yet) |
| D3 | `GraphEdit::dissolve`/`discard`, `Dissolution`, `Publication::removed`, errors, `settle`; `commit` asserts G6 | unit tests (§5) for both strategies |
| D4 | Reader: eager `settle` in `ReadStage` (debug assertion: no dissolution into the root, §2.6); un-ignore the 9 scenarios, remove `READ_DEVIATIONS`; `validate_retention()` after every read in the reader tests | gate (§9) |
| D5 | Benchmarks (§6) | numbers recorded here |
| D6 | Early commit (§5.1, after D1b): `commit_early`, the reader calls it after `settle` for staged tokens satisfying the rule | gate (§9) unchanged; no early-committed token is dissolved (debug assertion); benchmarks recorded |

## 9. Gate

```sh
cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api
cargo test -p context-read --no-default-features --features test-api
cargo test -p context-trace -p context-search -p context-insert --no-default-features --features test-api
cargo test --release -p context-insert --no-default-features --features test-api insert_gate
cargo test -p context-read --features legacy-tests
cargo bench -p context-read
```

- Every scenario and read-lane case equals its oracle graph for both
  strategies; none ignored for provisional retention; `READ_DEVIATIONS`
  removed.
- `validate()` (G1–G5) after every read and every insert case;
  `validate_retention()` (G6) after every read.
- Insert gate unchanged (all cases).
