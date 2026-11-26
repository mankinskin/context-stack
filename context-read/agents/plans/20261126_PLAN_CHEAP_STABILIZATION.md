---
tags: `#plan` `#context-read` `#context-trace` `#positioned-composition` `#reader` `#staging` `#stabilization`
summary: Plan for early stabilization of staged tokens (replaces task D7 of 20261125_PLAN_DISSOLUTION). Revision 2 - Rule S′ - a parent occurrence (U, o) of a staged composite t is fixed iff t ends strictly inside U (any U, the root included) or U is committed or stable; t ≠ root is stabilized iff it has ≥ 2 fixed parent occurrences. Theorem S′ - such a token is a maximal repeat of every continuation, a vertex of every later settled stage, never dissolved. Purpose - the stable property itself (settle skips stable tokens; foundation for streaming and later optimizations). Cheap, parent-local check, event-driven candidates (E1 added occurrences and created tokens, E2 cascade, E3 root growth) in the settle edit. Revision 1 (Rule S, only committed or stable parents) was sound but vacuous in the reader (history in §6.R1). Verification - rule, fixpoint and string-level soundness checks per step, exhaustive small inputs, mutations; coverage reported (33 % fixtures, 60 % exhaustive).
status: ✅ revision 2 (Rule S′) implemented (2026-11-26, §6). Revision 1 (Rule S) superseded: sound but vacuous in the reader (§6.R1).
date: 2026-11-26
revision: 2 (2026-11-26, Rule S′ replaces Rule S)
parent: 20261125_PLAN_DISSOLUTION.md (§4.6 D7, Appendix C.7 superseded by this plan)
depends_on: 20261122_PLAN_POSITIONED_INSERT.md (G2 complete maximal composition, G3 reciprocal links), 20261125_PLAN_DISSOLUTION.md (G6, `settle`, M1, M4, M5, stability marks)
---

# Plan: Cheap stabilization of staged tokens

## 0. Overview

| Task | Content | Behavior change |
|---|---|---|
| S1 | Replace the decision: remove Theorem E machinery, add the fixed-parent rule | which tokens become stable |
| S2 | Event-driven candidates (edit records + cascade), stabilization inside the settle edit | none (cost only) |
| S3 | Verification: new lane checks, unit tests, reader expectations | — |
| S4 | Benchmarks, gate, documentation (old plan, INDEX) | — |

| S′ (rev. 2) | Rule S′ (§2.2), events E1–E3 (§3), tests and results (§4.4, §5.8, §6) | which tokens become stable |

Order: S1 → S2 → S3 → S4, then S′. S1 and S2 are one code change in practice;
they are separated here so the review can check the rule and the
scheduling independently.

## 1. Purpose and problem

### 1.1 Purpose (revision 2)

A stable staged token can never be dissolved, so `settle` skips it. More
importantly, stability is the foundation for **streaming** (tokens that
are final before the input ends) and a general *stable* property of
staged tokens that later optimizations can rely on. Measured on the
probe, the `settle` savings are small (2–4 % of `settle` checks); the
main value is the stable property itself. The check must therefore be
cheap enough to cost almost nothing (§5.7: within ~5 % of D6).

### 1.2 Problem (revision 1)

Stabilization exists to make the reader cheaper: a stable staged
vertex is never removed again, so `settle` can skip it, and a later
consumer (streaming commit, memory bound) may treat it as final. It is
only worth doing if the check is **much cheaper than the work it saves**.

The D7 implementation decides *exact* permanence (Theorem E): it walks
the right contexts of every unstable vertex on the root's postfix chain
(Lemma W, recursive through parents, descending to the atom at each
occurrence end), and tests uniformity `t = aⁿ` by a content DFS. All of
it is recomputed at every step.

- In periodic input nothing is permanent until the last atom, so every
  step re-walks the whole stage: cost ≈ steps × staged tokens × depth.
- Measured (dissolution plan §6, D6 → D7): `abc^256 x` 41.8 ms → 129.9
  ms, `abc^512 x` 364 ms → 1.11 s, `read/corpus/all` 1.89 → 2.14 ms.
- Cases (c) "end of a committed input" and (d) "uniform `aⁿ`" need
  string-level information (input identity, content) that the graph does
  not hold locally. They are the most expensive part and must not be
  used.

**Principle (binding).** Stabilization is a cheap optimization. A token
is marked stable only if a check on its **parent occurrences alone**
(no search, no context walk, no content inspection) proves that it can
never change again. If the cheap check cannot prove it, the token stays
unstable; it is still committed correctly at the end of the read
(`settle` keeps exactly the oracle vertices, M4). Exactness
("permanent ⇒ stable") is **not** a goal.

## 2. Theory

### 2.1 Contexts are parent occurrences

In the graph, a token's contexts are its parent occurrences. By G2
(insert plan §3.1, *complete maximal composition*), every child
occurrence `t@o ∈ comp(U)` is a **maximal** occurrence inside `U`: no
other vertex occurring inside `U` contains it. So each parent occurrence
`(U, o)` extends `t` on at least one side, and two different parent
occurrences cannot be extended by one common vertex inside both parents.
This is why G6 counts parent occurrences: a staged composite is a vertex
of the oracle graph iff it has ≥ 2 parent occurrences (M4 after
`settle`), i.e. it occurs in at least two distinct contexts.

Notation:

- `Occ(U, o)`: the text occurrences of `t` represented by the parent
  occurrence `(U, o)`: every occurrence of `U`, shifted by `o`.
- An occurrence is *open* if it ends at the cursor (its right context is
  the next input atom, not yet read). All other contexts are *fixed*:
  left contexts are always fixed (reading only appends to the right),
  right contexts of non-open occurrences are fixed.
- A vertex `U` is *fixed* if it is committed or stable.

### 2.2 The rule (revision 2: Rule S′)

> **Rule S′.** After `settle`, a parent occurrence `(U, o)` of a staged
> composite `t` is **fixed** iff
>
> - (i) `t` ends strictly inside `U`: `o + |t| < |U|` (any `U`, the root
>   and unstable staged vertices included), or
> - (ii) `U` is committed or stable.
>
> `t ≠ root` is stabilized iff it has **≥ 2 fixed parent occurrences**
> (counted as occurrences, as in G6; two offsets in the same parent count
> twice).

Why (i) is fixed: the right neighbor of `t@o` is already-read content of
`U`. Staged non-root content never changes (inserts only regroup it),
and the root only grows at its end. Left contexts are always fixed.

The check reads only `parents(t)` and, per parent occurrence, one width
comparison and one bit (`!U.is_staged() || stage.is_stable(U)`): O(1)
per parent occurrence, stopping at 2. An occurrence at the **end** of an
unstable parent (the root's end in particular) does not count: its right
context is the parent's right context, which can still change.

*Revision 1* (Rule S) counted only (ii). It is sound but has no base
case in the reader (§6.R1.1): (i) is the base case.

### 2.3 Theorem S′ (soundness)

Hypothesis H(U) for a fixed vertex `U`:

- (H-L) no atom precedes all occurrences of `U`, and
- (H-R) no atom follows all fixed (non-open) occurrences of `U`.

Input boundaries count as contexts distinct from every atom and from
each other (`^k`, `$k`).

**Lemma S0.** Every committed composite satisfies H. *Proof.* A
committed composite is in `V(C)` (M1 at the last commit): a maximal
repeat of `C` (≥ 2 left and ≥ 2 right contexts in `C`, all fixed) or an
input text (bounded by its own `^k`/`$k`). Contexts only grow (M5). ∎

**Theorem S′.** Let `(U₁, o₁) ≠ (U₂, o₂)` be fixed parent occurrences
(Rule S′) of the staged composite `t` after `settle`, and
`S = Occ(U₁, o₁) ∪ Occ(U₂, o₂)` (text occurrences). Then `S` has ≥ 2
distinct left contexts, and its fixed right contexts are not a single
atom. Hence `t` is a maximal repeat of every continuation, a vertex of
every later settled stage (M4), never dissolved, and satisfies H itself.

*Proof.* Left contexts are always fixed. The right context of an
occurrence of kind (i) is a fixed atom inside `Uᵢ`. For kind (ii) it is
the right context of `Uᵢ`, and those satisfy (H-R): by Lemma S0 for a
committed `Uᵢ`, by induction (below) for a stable one.

Suppose all left contexts of `S` are one atom, or all fixed right
contexts of `S` are one atom `a`.

1. Let `M` be the maximal common extension of the occurrences in `S`
   (extend left/right while all of them agree). Then `M ⊋ t`.
2. `M` stays inside each `Uᵢ` region.
   - Left boundary: passing it would need all occurrences of `Uᵢ` to
     share a left atom. But `Uᵢ` is a current vertex, hence left-maximal
     (M1/M4), or it is the root or an input, with a unique start marker.
   - Right boundary: if `Uᵢ` has an occurrence ending at the cursor, the
     extension stops there (open). Otherwise all right contexts of `Uᵢ`
     are fixed and `Uᵢ` is right-maximal (a current vertex). For a fixed
     `Uᵢ` use (H-R).
3. So `M` occurs ≥ 2 times, inside `U₁` and inside `U₂`, and is not
   extendable over `S`. Hence `M` is a maximal repeat of the current `D`
   and, by M1/M4, a vertex of the current stage.
4. By G2, `t@oᵢ` is a maximal occurrence inside `Uᵢ`. `M` occurs inside
   `Uᵢ` and contains it, so `M = U₁ = U₂` and `o₁ = o₂` (the same
   alignment of `t` in `M`): a contradiction.

So `S` has ≥ 2 left contexts and fixed right contexts that are not a
single atom. These occurrences and contexts persist in every
continuation (M5), so `t` is a maximal repeat of every continuation,
a vertex of every later settled stage (M4), and is never dissolved. It
also satisfies H, which is the induction step for (ii): by induction
over the order of stabilization (base: Lemma S0 and case (i)), every
stable vertex satisfies H, so Rule S′ is sound at every step. ∎

The vacuity proof of Rule S (§6.R1.1) explains why (ii) alone has no
base case: two committed parent occurrences would already make `t` a
maximal repeat of the committed corpus, i.e. committed.

What the theorem does **not** need:

- right-context sets, the postfix chain, input identities, uniformity;
- the parents staying parents: later inserts may wrap `t@oᵢ` into a new
  token, but the text occurrences `S` and their contexts persist.

### 2.4 Coverage (not exactness)

Rule S′ is sufficient, not necessary. It does not stabilize, for
example:

- tokens with an occurrence at the root's end and only one other fixed
  occurrence (`aa` in `aaa`: `root@0` inside, `root@1` at the end);
- tokens whose permanence rests on the end of a committed input, or on
  uniformity (D7 cases c and d).

They stay unstable and are committed normally. Stability arises
**bottom-up** from (i) (two inside occurrences, e.g. `ab` in `abcabd`)
and propagates **top-down** through (ii) (the children of a stable
vertex, e.g. `abc` after `abcabc` in `abcabcabcx`).

Measured coverage (§6.2): 33.0 % on the fixture lane, 55.4 % and 60.1 %
on the exhaustive lanes; reported, not asserted.

## 3. Scheduling: event-driven candidates

`fixed(t)` (the number of fixed parent occurrences of `t`, Rule S′)
rises only through these events:

| Event | Where it is recorded | Candidates |
|---|---|---|
| E1 a parent occurrence of `t` is **added** (in any vertex): an insert or commit adds occurrences or creates tokens, `settle` dissolves `t'` into its host | `VertexEdit.added` and `Publication.created` of the insert and commit publications (`Candidates::record`); the dissolutions inside `settle` | `added[*].child`; every created token and its children; the children of every dissolved vertex (renamed through removals) |
| E2 a parent becomes stable (ii) | the stabilization itself | the children of the stabilized vertex (cascade) |
| E3 the root grows: an occurrence that ended at the root's end now ends inside (i) | `ReadStage::commit` in place returns the root's previously last child (by G2 exactly one occurrence ends at the root's end) | that child (`Candidates::push_gained`) |
| — the previous root loses its exemption (`t ≠ root`) | the reader (`Candidates::push`) | the previous root |

The other commits need nothing extra:

- `NewRoot` creates a new root: the old root and the appended token
  become its children (E1, created).
- `Adopt`/`ReplaceRoot` adopt an existing token without edits. The
  previous root is pushed as a candidate.

Nothing else raises `fixed(t)`:

- renames (swap-remove, stabilization swaps) keep fixedness;
- staged non-root widths never change;
- vertices become committed only at the end of the read.

Lowering `fixed(t)` (an occurrence wrapped into a new token) is harmless
for vertices that are already stable (Theorem S′). For unstable vertices
the rule is simply re-evaluated at the next event.

Algorithm, in the same `GraphEdit` as `settle`, right after it:

```text
work = gained candidates (E1, E3, previous root; renamed through
       settle's removals; created tokens expanded to their children)
while let Some(t) = work.pop():
    if t is not staged, or t == root, or stable(t): continue
    if fixed_parent_occurrences(t) >= 2:            // stops at 2
        stabilize(t)                                // swap into the prefix
        work.extend(children(t))                    // E2 cascade
```

- **Cost.** O(added occurrences + created children + Σ over stabilized
  t of (parents(t) + children(t))), plus the renaming of the work list
  on every swap. Each vertex is stabilized at most once. Nothing scans
  the stage, the chain or contents.
- **Fixpoint.** After the loop, no unstable staged composite `≠ root`
  has ≥ 2 fixed parent occurrences (complete for Rule S′, given the
  event table). The lane checks this (§5.1 F).

## 4. Code changes

### 4.1 S1 — rule and removals (context-trace)

**`context-trace/src/positioned/edit/stable.rs`**

Remove:

- `RightContext`, `RightContexts` (memo, `all`, `over_parents`,
  `fixed`), the `Contexts` alias and `push` helper;
- `fixed_right_contexts`, `atom_at`, `uniform_atom`;
- the imports used only by them (`HashMap`, `BTreeSet`, `AtomPosition`,
  `Navigate`, `NodeRef`, `CompositionRead`, `VertexState`, `AtomOf`,
  ...).

Keep and change `GraphEdit::stabilize(t, root) -> Result<Option<Swap>,
Error>`:

- Same checks as today: `UnknownVertex`, `Atom`, `NotStaged`, `IsRoot`;
  `Ok(None)` if already stable.
- Replace the `ParentCount` (< 2) check with the Rule S check: count the
  fixed parent occurrences (stop at 2). Fewer than 2 →
  `Err(PositionedError::FixedParentCount { vertex, count })` (new
  variant in `positioned/model`). The strict vertex-local check makes it
  impossible to stabilize something unsound.
- Raw stabilize and `rename_records` unchanged.

Add:

- `fn fixed_parent_occurrences(graph: &S, t) -> usize` (private,
  capped at 2). Used by `stabilize` and the worklist.
- `pub fn stabilize_candidates(&mut self, candidates: impl
  IntoIterator<Item = StageIndex>, root: &mut Token<StageIndex>) ->
  usize`: the worklist of §3. Renames the remaining work list and `root`
  through every swap (`Swap::apply`). Returns the count. This mirrors
  `settle`, which is already a `GraphEdit` worklist method, and only
  calls the strict `stabilize(t, root)`, consistent with "helpers edit
  single vertices with checked inputs".

Module doc: "Early stabilization by fixed parent occurrences (plan
20261126 §2, Theorem S)".

**`context-trace/src/positioned/edit/mod.rs`, `positioned/mod.rs`**

- Drop the re-exports `RightContext`, `RightContexts`,
  `fixed_right_contexts`, `uniform_atom`. Keep `Swap`.
- Update the docs that mention Theorem E.

**Unchanged (kept from D7):**

- the stable slot prefix (`StagingGraph::stable`, `raw::Stabilize`,
  `Swap`, the shared `rename`);
- the `remove` assertion (stable vertices cannot be removed);
- the `settle` skip of stable vertices;
- `rename_records`.

### 4.2 S2 — candidates and integration

**`context-trace/src/positioned/edit/dissolve.rs`**

- `Candidates`:
  - add a second list `gained: Vec<StageIndex>`;
  - `record(publication)` additionally pushes `added[*].child.index` of
    every `VertexEdit` (all edits are recorded; the vertex's fixedness is
    checked at use, because the edited vertex may itself be renamed);
  - add an accessor `gained()`.
- `GraphEdit::settle(candidates, root)`:
  - additionally renames `gained` through every removal (`Moved`) and
    drops removed vertices from it;
  - appends `added[*].child` of its own dissolution edits whose vertex is
    fixed (E1 inside settle);
  - returns the surviving gained list in `Settled` (new field
    `gained: Vec<StageIndex>`).

**`context-read/src/root.rs`**

- Delete `ReadStage::stabilize` in its current form: the chain
  `BTreeSet`, the `RightContexts` walk, `uniform_atom`, the
  decide-then-apply list with its O(k²) swap fix-up, and the imports
  `BTreeSet`, `Navigate`, `RightContext`, `RightContexts`,
  `uniform_atom`.
- `ReadStage::settle` runs, in the same `GraphEdit`, `edit.settle(..)`
  then `edit.stabilize_candidates(settled.gained, &mut root)`, and
  returns both counts (`Settled { dissolved, discarded, stabilized, .. }`
  or a small struct). One edit and one `finish` per step instead of two.

**`context-read/src/read.rs`**

- The `commit` closure calls only `root.settle(candidates)`.
- `Step.stabilized` is kept, with the meaning "stabilized by Rule S in
  this step".
- Update the docs that mention Theorem E.

### 4.3 Documentation

- `context-trace/src/positioned/stage/mod.rs`: `Stage::stable` doc
  points to this plan instead of dissolution plan §4.6.
- `20261125_PLAN_DISSOLUTION.md`:
  - status: D7 superseded by this plan;
  - §4.6 and Appendix C.7: a superseded banner with a link (Theorem E is
    no longer used by the implementation or the tests);
  - §6 "D7 results": a note that the numbers are of the superseded
    implementation.
- `INDEX.md`: add this plan and update the dissolution plan's status.

### 4.4 Revision 2 — Rule S′ (code)

- `edit/stable.rs`: `fixed_parent_occurrences` counts (i) or (ii), with
  the width check `offset.0 + |t| < |U|` (`wide::Wide`). `stabilize`
  checks `Atom` without cloning the composition, and applies its swap
  to the edit records in place (`swap_records`: only the two swapped
  entries move, no map rebuild). `stabilize_candidates` reads the
  children of a stabilized vertex after the swap.
- `edit/dissolve.rs`, `Candidates`:
  - `record` adds the children of **all** added occurrences and keeps
    the created tokens (`created`);
  - `push` (previous root) adds to both lists;
  - new `push_gained` (E3).
- `edit/dissolve.rs`, `settle`:
  - renames `created` along with the other lists;
  - adds the children of every dissolution (any host);
  - at the end expands the surviving created tokens to themselves plus
    their children;
  - returns `gained` filtered to staged indices, sorted and deduplicated.
- `context-read/src/root.rs`: `ReadStage::commit` returns
  `(Commit, Option<Publication>, Option<StageIndex>)`. The last element
  is the previous last child of an in-place grown root (E3).
- `context-read/src/read.rs`: pushes it with `Candidates::push_gained`.

## 5. Verification

### 5.1 Per-step lane (`context-read/src/tests/steps.rs`)

Remove:

- `Context`, `fixed_contexts`, `permanent` (Theorem E on strings);
- the checks "permanent ⇒ stable" and `late`;
- the E2 witness continuation and `witnesses`;
- invariant (U) ("every unstable vertex is on the chain"). It no longer
  holds: off-chain tokens without two fixed parent occurrences stay
  unstable.

Keep:

- root not stable;
- (S) staged composites have ≥ 2 parent occurrences;
- every step equals its oracle graph;
- a stable string is never removed later in the read, and every stable
  vertex is committed.

Add:

- **(R) rule.** Every stable staged vertex had, when it was stabilized,
  ≥ 2 fixed parent occurrences. This is checked by `GraphEdit::stabilize`
  itself (an `Err` panics the reader). The lane also asserts it on
  `stage.is_stable` vertices newly stable in this step.
- **(F) fixpoint.** No unstable staged composite `≠ root` has ≥ 2
  fixed parent occurrences. This proves the event table (§3) is
  complete.
- **(T) Theorem S on strings (soundness oracle).** For every stable
  `t`, compute on the corpus strings `C + [P]`:
  - its left contexts over all occurrences;
  - its right contexts over the non-open occurrences, with input ends as
    distinct contexts.

  Assert ≥ 2 left contexts and that the right contexts are not a single
  atom. This tests the theorem's conclusion directly, independently of
  the graph. It runs in tests only, so cost is irrelevant.
- **Stats** (printed, not asserted): stabilized, mean/max latency from
  first appearance, unstable-at-end count, and coverage as
  `stabilized / (staged vertices that survive to the commit)`.

### 5.2 Exhaustive small inputs

A new test in the lane module runs the reader with the checks of §5.1
on every input over `{a, b}` up to length 6, with corpora of up to 2
earlier inputs from the same set (bounded, e.g. pairs up to length 4).
This catches unsoundness in shapes the fixtures lack, such as periodic,
nested or boundary-aligned inputs.

- Default (debug) run time must stay below ~10 s. If it doesn't, put the
  full enumeration behind a test filter and keep a sampled subset in the
  default run.

### 5.3 Unit tests (`context-trace/src/positioned/tests/stage/stable.rs`)

Remove: `fixed_right_contexts_match_the_corpus`,
`uniform_atom_matches_the_content`.

Keep: `stabilize_swaps_and_renames`, `remove_of_a_stable_vertex_panics`.

Change: `graph_edit_stabilize_checks`. Its cases:

- a vertex with ≥ 2 parent occurrences, but < 2 of them fixed, returns
  `FixedParentCount` and leaves the graph unchanged;
- a vertex with 2 committed parent occurrences succeeds;
- the other errors as before.

Add:

- `stabilize_candidates_cascades`: a staged `U` with two committed
  parents and a staged child `t` whose only parents are two occurrences
  in `U`. Both become stable (`t` through the cascade); the count is 2.
- `stabilize_candidates_skips_unfixed`: the same, with `U`'s parents
  being the (staged) root. Nothing is stable.
- A `settle` + `stabilize_candidates` case where a swap-remove renames a
  gained candidate, and the renamed vertex is checked.

### 5.4 Reader expectations (`context-read/tests/reader_behavior.rs`)

- Recompute the `stabilized` column of every `step(..)` expectation
  from the new implementation. Each change must be explained by Rule S;
  the review compares the list.
- Known change: `repeat_aaa` (last step 1 → 0). `aa` has only root
  occurrences.

### 5.5 Unchanged tests

- The M1 test `oracle_vertex_sets_are_maximal_repeats` stays. It is
  independent of stabilization and backs the M1 step of Theorem S.
- Mutation checks to run once and record in §6. Each must make a check
  fail:
  - count all parent occurrences instead of fixed ones → (T) fails;
  - drop the E2 cascade → (F) fails;
  - drop E1 from `settle` → (F) fails;
  - count the root as fixed → (T) fails.

### 5.6 Gate

As in dissolution plan §5.4:

- context-read lib, integration and doc tests (`--no-default-features
  --features test-api` and default);
- context-trace, context-search and context-insert tests; the insert
  gate;
- legacy suites (`--features legacy-tests`);
- the workspace build `cargo build --workspace --all-targets --exclude
  context-editor-kernel --exclude sandbox-app`;
- `--no-default-features` checks of every positioned crate;
- release context-read.

### 5.7 Benchmarks

Run `staging` and `read` benches with the same filters as dissolution
plan §6:

```text
cargo bench -p context-read --no-default-features --features test-api \
  --bench staging -- "read/periodic/abc\^n_x/256" \
  --sample-size 10 --warm-up-time 1 --measurement-time 3
```

Target: within noise (≤ 5 %) of the D6 numbers for `abc^64/256/512 x`
and `read/corpus/all`, and `grammar_worst_case` unchanged. Record a
D6 / D7 / S table in §6 of this plan.

### 5.8 Revision 2 — verification changes

- Lane (R), (F) and the helper `fixed_parent_occurrences` use Rule S′.
  (T) is unchanged: the conclusion of Theorem S′ is the same string
  condition.
- The exhaustive tests are renamed `small_inputs_follow_rule_s_prime`
  and `small_inputs_follow_rule_s_prime_full`.
- Unit tests (`tests/stage/stable.rs`):
  - case (i): two occurrences inside the root count;
  - an end occurrence in an unstable parent doesn't count;
  - an end occurrence in a committed or stable parent counts;
  - the cascade;
  - `settle` renaming and `settle` dissolution gains.

  E3 is covered by a `root.rs` unit test (the in-place commit returns the
  previous last child) and by the reader traces.
- Reader traces for `abcabd`, `abcabca` and `abcabcabcx`.
- Mutations:
  - count end occurrences in unfixed parents as fixed → (R)/(T) fail;
  - drop E1 / E2 / E3 → (F) fails;
  - count the root as a fixed parent at its end → (T) fails.

## 6. Results (revision 2, Rule S′)

Implemented 2026-11-26. Nothing is committed to git.

### 6.1 Summary

- Rule S′ is implemented with event-driven candidates (E1–E3, previous
  root) inside the `settle` edit. No production code scans the stage.
- The stabilization counts are identical to those of the full-scan
  probe (revision 1, §6.R1.9) on all three lanes. The fixpoint check
  (F) is green everywhere, so the event table of §3 is complete on these
  inputs.
- All soundness checks are green: (R), (T), "stable never removed" and
  "stable committed".
- All benchmarks are within ~4 % of D6 measured on the same day (§6.6).

### 6.2 Lane (§5.1, §5.2)

| Lane | Reads | Steps | Dissolved | Stabilized | Latency mean / max | Unstable at end | Coverage |
|---|---|---|---|---|---|---|---|
| per-step (94 scenarios) | 199 | 701 | 16 | 66 | 0.44 / 2 | 134 | 66 / 200 = 33.0 % |
| exhaustive, default: inputs ≤ 6, ≤ 1 earlier input ≤ 4 (3,906 cases) | 3,906 | 14,178 | 70 | 3,474 | 0.76 / 3 | 2,800 | 3,474 / 6,274 = 55.4 % |
| exhaustive, full R6: ≤ 2 earlier inputs ≤ 4 (117,306 cases, `#[ignore]`, release) | 117,306 | 380,250 | 1,030 | 116,038 | 0.64 / 3 | 76,946 | 116,038 / 192,984 = 60.1 % |

Discarded: 0 in every lane.

Run times:

- default lane (both default tests): ~7.7 s in debug;
- full enumeration: 46.4 s in release.

### 6.3 Reader expectations (§5.4)

The `stabilized` column was recomputed for every `step(..)` expectation
in `tests/reader_behavior.rs`. No existing expectation changed:

- `seam_overlap_append` (`xabcdey` on `abcd, bcde`): all 0. No staged
  composite has two fixed parent occurrences at any step (checked by (F)
  in the lane).
- `accepted_materialization` (`abcabca` on `abc`): all 0. In the root
  `abca·abca`, `abca@3` ends the root, so `abca` has 1 fixed occurrence.
- `repeat_aaa`: all 0. `aa` has `root@0` inside and `root@1` at the
  root's end: 1 fixed.

New trace tests (empty graph, per-step `stabilized`):

| Test | Steps (kind, commit, root, stabilized) | Rule S′ explanation |
|---|---|---|
| `abcabd_stabilizes_ab` | Unknown/Adopt/1/0, Unknown/NewRoot/2/0, Unknown/InPlace/3/0, Forward(2)/InPlace/5/0, Unknown/InPlace/6/**1** | After `d` is appended in place, `ab` has `root@0` and `root@3`, both inside (i). `ab@3` is the root's previous last child (E3). |
| `abcabca_stabilizes_nothing` | …, Forward(3)/NewRoot/6/0, Expansion{3→4}/InPlace/7/0 with 1 dissolved | `abc` is dissolved, and `abca@3` ends the root. |
| `abcabcabcx_stabilizes_by_cascade` | …, Forward(3)/NewRoot/6/0, Expansion{3→6}/NewRoot/9/0, Unknown/InPlace/10/**2** | `x` makes `abcabc@3` end inside the root. With `abcabc@0`, `abcabc` is stabilized (i). Then `abc` follows by the cascade: `abc@0` is inside and `abc@3` is in a stable parent (ii). |

A `root.rs` unit test (`append_rows`) checks that the in-place commit
returns the previous last child (E3).

### 6.4 Unit tests (§5.3, §5.8)

`context-trace/src/positioned/tests/stage/stable.rs`, 9 tests (all
green):

- `stabilize_swaps_and_renames` and `remove_of_a_stable_vertex_panics`
  (kept).
- `graph_edit_stabilize_checks`:
  - `FixedParentCount` for `bc` (0) and `baba` (1, `ababab@1` only);
  - `bab` (`baba@0` inside plus `abab@1` committed) is stabilized;
  - stabilizing twice is a no-op.
- `end_occurrences_count_in_fixed_parents_only`:
  - `ab` (`abab@0` inside, `abab@2` at the end of an unstable parent)
    is rejected with count 1;
  - `abab` (two end occurrences in committed hosts) is accepted;
  - then `ab` is accepted.
- `stabilize_candidates_cascades`: `abab` from the candidates, then `ab`
  through E2; count 2.
- `stabilize_candidates_skips_end_occurrences_in_unstable_parents`: in
  the root `abab·x·abab`, nothing is stabilized.
- `occurrences_inside_the_root_are_fixed`: `ab` in the root `abxaby`
  (case (i)).
- `settle_renames_gained_candidates`: the swap-remove of `ba` renames the
  gained `abab`; `abab` is stabilized, then `ab`.
- `settle_dissolution_gains`: dissolving `aab` into the root makes
  `ab@1` end inside the root. With the committed `xabab@3`, `ab` is
  stabilized (E1 in `settle`).

`tests/stage/dissolve.rs` compares the `Settled` counts only (helper
`counts`). The gained list is tested in `stable.rs`.

### 6.5 Mutation checks (§5.5, §5.8)

Each mutation was applied temporarily and reverted afterwards. The
restoration was verified with grep.

| Mutation | Observed |
|---|---|
| Count end occurrences in unfixed parents as fixed (every occurrence fixed) | Lane and exhaustive lane fail at (R): `repeat_aaa`, "new stable `aa` has 1 fixed parent occurrences". With (R) disabled, (T) fails: `aa` with right contexts `{a}`. Also failing: 3 unit tests and 8+ `reader_behavior` tests. |
| Count the root as a fixed parent at its end | Lane fails at (R). With (R) disabled, (T) fails, same as above. Also failing: `stabilize_candidates_skips_end_occurrences_in_unstable_parents` and 8+ `reader_behavior` tests. |
| Drop E2 (cascade) | (F) fails in the lane (`insert_deep_borders`: unstable `ab` with 2 fixed parent occurrences; `insert_shared_vertex`) and in the exhaustive lane (`aaaab`). Also failing: `stabilize_candidates_cascades` and `abcabcabcx_stabilizes_by_cascade`. |
| Drop E3 (root growth) | (F) fails in the lane (`complex_overlap`, `text`, `infix`, `seam_bcd`, …) and in the exhaustive lane. Also failing: `abcabd_stabilizes_ab` and `abcabcabcx_stabilizes_by_cascade`. |
| Drop E1, children of created tokens | (F) fails in the exhaustive lane (`["abb", "abbbb"]`: unstable `bb` with 2 fixed parent occurrences). |
| Drop E1, children of added occurrences | **(F) does not fail**, even on the full exhaustive lane. Caught by the unit test `settle_renames_gained_candidates`. |
| Drop E1 in `settle` (children of dissolutions) | **(F) does not fail**, even on the full exhaustive lane. Caught by `settle_dissolution_gains`. |
| Drop the previous root from the gained list | **(F) does not fail**, even on the full exhaustive lane. Not caught. |

Why the last three are redundant on reader inputs:

- **Added occurrences.** In the reader they are either created tokens,
  which are covered by E1 "created", or tokens appended at the root's
  end, which are not fixed until E3.
- **Previous root.** After `NewRoot` it is a child of the created root
  (E1). After `Adopt` its parent occurrences are unchanged.
- **Dissolutions.** On these inputs their children never become fixed
  through the dissolution alone.

All three events are kept, because the event table needs them for
completeness in general (inserts into existing hosts).

### 6.6 Benchmarks (§5.7)

Setup: `--sample-size 10 --warm-up-time 1 --measurement-time 3`. Machine
load differs between days. "D6 today" is `HEAD` (42d047e, D6) built from
`git archive` in `target/` and measured right before S′.

| Benchmark | D6 (recorded) | D6 today | D7 | S (rev. 1) | S′ | S′ vs D6 today |
|---|---|---|---|---|---|---|
| `read/corpus/all` | 1.892 ms | 1.987 ms | 2.139 ms | 1.934 ms | 2.051 ms | +3.2 % |
| `read/periodic/abc^n_x/64` | 1.010 ms | 1.014 ms | 2.764 ms | 1.009 ms | 1.053 ms | +3.8 % |
| `read/periodic/abc^n_x/256` | 41.77 ms | 40.92 ms | 129.9 ms | 41.25 ms | 41.84 ms | +2.2 % |
| `read/periodic/abc^n_x/512` | 364.3 ms | 366.9 ms | 1.112 s | 369.0 ms | 371.7 ms | +1.3 % |
| `grammar_worst_case` | — | 5.615 ms | 6.27 ms | 5.87 ms | 5.714 ms | +1.8 % |

- Against the recorded D6 numbers, `read/corpus/all` is +8 %. Most of
  that is day-to-day drift: D6 today is itself +5 %.
- A run with `stabilize_candidates` disabled gave `read/corpus/all`
  1.992 ms and `abc^64 x` 1.028 ms. So the S′ worklist itself costs
  about 3 % and 2.4 %; the rest is bookkeeping noise.
- Optimization done: `swap_records` renames the edit records in place
  instead of rebuilding the maps on every swap.
- The work-list renaming per swap (O(work)) does not show: `abc^512 x`
  stabilizes about 511 vertices in its last step and is +1.3 %.

### 6.7 Gate (§5.6)

All green (2026-11-26):

| Command | Result |
|---|---|
| `cargo test -p context-read --no-default-features --features test-api` | 126 passed, 1 ignored |
| `cargo test -p context-read` | 126 passed, 93 ignored |
| `cargo test -p context-read --release` | 126 passed, 93 ignored |
| `cargo test -p context-trace -p context-search -p context-insert --no-default-features --features test-api` (includes the insert gate) | 122 passed |
| `cargo test -p context-trace -p context-search -p context-insert` | 140 passed |
| `cargo test -p context-trace --features legacy-tests` | 197 passed |
| `cargo test -p context-search --features legacy-tests` | 77 passed |
| `cargo test -p context-insert --features legacy-tests` | 67 passed |
| `cargo test -p context-read --features legacy-tests` | 209 passed |
| `cargo build --workspace --all-targets --exclude context-editor-kernel --exclude sandbox-app` | ok |
| `cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features` | ok |
| `cargo test --release ... --lib small_inputs_follow_rule_s_prime_full -- --ignored` | ok (§6.2) |

### 6.8 Implementation notes and deviations (revision 2)

1. E1 is broader than "added occurrences": it also expands created
   tokens to themselves plus their children. Their compositions have no
   `VertexEdit`, so `settle` reads them at the end (vertex-local). This
   is required: the "created" mutation fails (F).
2. `Candidates::push` (previous root) now also adds to the gained list;
   `push_gained` is new (E3).
3. E1 in `settle` pushes the children of every dissolution, whatever the
   host (revision 1: only fixed hosts).
4. `ReadStage::commit` returns a third tuple element (the previous last
   child for E3) instead of a struct.
5. `Settled.gained` is sorted and deduplicated. The worklist pops from
   the end, i.e. the highest slot first.
6. `tests/stage/dissolve.rs` compares only the counts of `Settled`
   (helper `counts`), because the gained list now holds many incidental
   entries.
7. Three sub-events are redundant on reader inputs (§6.5). They are kept
   for completeness of the event table.

## 6.R1 History: revision 1 results (Rule S, superseded)

> **Superseded (2026-11-26)** by revision 2 (Rule S′, §6.R1.1–§6.R1.8). Kept as
> history: the vacuity finding §6.R1.1 motivates case (i) of Rule S′.

Implemented 2026-11-26 (S1–S4). Nothing is committed to git.

### 6.R1.1 Finding: Rule S never fires in the reader

On every reader input checked, Rule S stabilizes **nothing**:

- 0 of 701 lane steps;
- 0 of 380,250 steps of the full exhaustive enumeration.

The fixpoint check (F) passes everywhere, so this is not a scheduling gap.
No unstable staged composite ever has ≥ 2 fixed parent occurrences.

The reason is that the induction of §2.3 has no base case in the reader:

1. Before the first stabilization, the only fixed vertices are committed
   ones. So the first stabilized `t` needs two parent occurrences in
   committed `U₁`, `U₂`. In the overlay a committed host can gain staged
   children through an insert, so this is not excluded syntactically.
2. Apply the argument of Theorem S to the committed corpus `C` alone.
   `U₁`, `U₂` are vertices of `V(C)` and satisfy H with respect to `C`
   (Lemma S0). Their occurrences in `C` are fixed. By G2 at the current
   stage, `t@oᵢ` is maximal inside `Uᵢ`.
3. Steps 1–5 of the proof then show that `t` has ≥ 2 left and ≥ 2 right
   contexts in `C`. So `t` is a maximal repeat of `C`, i.e. `t ∈ V(C)`.
4. Since `t ∈ V(C)`, `t` is already committed: one vertex per string (G1).
   This contradicts `t` being staged.
5. So no first stable vertex exists. By induction, `fixed(t)` counts only
   committed parents, and step 1 applies at every step.

In short: Rule S is **sound, cheap and vacuous**. `Step.stabilized` is
always 0. The only cost left is the O(gained occurrences) worklist per
step, and the benchmarks show it is within noise (§6.R1.5).

The unit tests (§6.R1.3) can stabilize vertices only because their graphs
are not oracle graphs. In them, `xabab` and `yabab` are committed without
`ab` or `abab` being committed vertices.

The plan was implemented as written anyway; see §6.R1.8 for the open
question this raises.

### 6.R1.2 Lane (§5.1, §5.2)

`context-read/src/tests/steps.rs`:

- Removed: Theorem E on strings, the `late` and witness checks, and
  invariant (U).
- Added: (R), (F), (T), the stats, and the exhaustive lane.

| Lane | Reads | Steps | Dissolved | Discarded | Stabilized | Unstable at end | Coverage |
|---|---|---|---|---|---|---|---|
| per-step (94 scenarios) | 199 | 701 | 16 | 0 | 0 | 200 | 0 / 200 = 0 % |
| exhaustive, default: inputs ≤ 6, ≤ 1 earlier input ≤ 4 (3,906 cases) | 3,906 | 14,178 | 70 | 0 | 0 | 6,274 | 0 % |
| exhaustive, full R6: inputs ≤ 6, ≤ 2 earlier inputs ≤ 4 (117,306 cases, `#[ignore]`) | 117,306 | 380,250 | 1,030 | 0 | 0 | 192,984 | 0 % |

- Latency: no data, because nothing was stabilized.
- "Coverage" is stable / (staged composites ≠ root after the last step of
  a read, i.e. those that survive to the commit).
- Run times:
  - default lane (both default tests): 7.6 s in debug;
  - full enumeration: 45.7 s in release
    (`cargo test --release ... --lib steps -- --include-ignored`).

### 6.R1.3 Unit tests (§5.3)

`context-trace/src/positioned/tests/stage/stable.rs`:

- Removed: `fixed_right_contexts_match_the_corpus` and
  `uniform_atom_matches_the_content`.
- Kept: `stabilize_swaps_and_renames` and `remove_of_a_stable_vertex_panics`.
- Changed: `graph_edit_stabilize_checks`.
  - `FixedParentCount` for `bc` (0), `bab` (2 parent occurrences, 1
    fixed) and `ba` (4, 0 fixed). The graph is unchanged.
  - The success case uses a new fixture (`hosts`: committed `xabab` and
    `yabab`), because the existing fixture has no vertex with 2 committed
    parent occurrences.
- Added:
  - `stabilize_candidates_cascades`: `abab` from E1, then `ab` through
    E2; count 2.
  - `stabilize_candidates_skips_unfixed`: `abab` only in the root; count 0.
  - `settle_renames_gained_candidates`: the discard of `ba` moves the
    gained `abab` from s3 to s2. The renamed vertex is stabilized, then
    `ab`.
  - `settle_dissolution_into_a_stable_host_gains`: E1 inside `settle`.
    Dissolving `aba` into the stable `abab` gives `ab` its second fixed
    parent occurrence.
- `positioned/tests/stage/dissolve.rs`: the `Settled` expectations gained
  the new `gained` field (`[abca]` after the move, `[aba]`, or empty).

### 6.R1.4 Reader expectations (§5.4)

`tests/reader_behavior.rs`: the only change is `repeat_aaa`, last step
1 → 0, as predicted. Every other `step(..)` was already 0.

The M1 test `oracle_vertex_sets_are_maximal_repeats` is unchanged and
green.

### 6.R1.5 Benchmarks (§5.7)

Setup: `--sample-size 10 --warm-up-time 1 --measurement-time 3`, same
machine as dissolution plan §6.

| Benchmark | D6 | D7 | S | S vs D6 |
|---|---|---|---|---|
| `read/corpus/all` | 1.892 ms | 2.139 ms | 1.934 ms | +2.2 % |
| `read/periodic/abc^n_x/64` | 1.010 ms | 2.764 ms | 1.009 ms | −0.1 % |
| `read/periodic/abc^n_x/256` | 41.77 ms | 129.9 ms | 41.25 ms | −1.2 % |
| `read/periodic/abc^n_x/512` | 364.3 ms | 1.112 s | 369.0 ms | +1.3 % |
| `grammar_worst_case` | — | 6.27 ms | 5.87 ms | (unchanged or better) |

- All targets are within 5 % of D6.
- `abc^256 x` is the second run. The first run gave 43.9 ms, with a wide
  interval (41.4–47.3 ms).

### 6.R1.6 Mutation checks (§5.5)

Each mutation was applied temporarily and reverted afterwards. The
restoration was verified with grep.

| Mutation | Planned failing check | Observed |
|---|---|---|
| Count all parent occurrences | (T) | Lane fails at (R) first in many scenarios, e.g. `repeat_separated`, "new stable `abc` has 0 fixed parent occurrences". With (R) disabled, (T) fails: `abc` with right contexts `{x}`. Also failing: the exhaustive lane, the `settle` debug assertion "stable … lost its parent occurrences", 4 `reader_behavior` tests, and the unit tests `graph_edit_stabilize_checks` and `stabilize_candidates_skips_unfixed`. |
| Drop the E2 cascade | (F) | **(F) does not fail.** The lane never stabilizes anything (§6.R1.1), so there is no cascade to drop. Caught by the unit tests `stabilize_candidates_cascades` and `settle_renames_gained_candidates`. |
| Drop E1 from `settle` | (F) | **(F) does not fail**, for the same reason. Caught by the unit test `settle_dissolution_into_a_stable_host_gains`. |
| Count the root as fixed | (T) | Same pattern as the first row: (R) fails first, and (T) fails with (R) disabled. Also failing: `stabilize_candidates_skips_unfixed` and 4 `reader_behavior` tests. |

### 6.R1.7 Gate (§5.6)

All green (2026-11-26):

| Command | Result |
|---|---|
| `cargo test -p context-read --no-default-features --features test-api` | 123 passed, 1 ignored |
| `cargo test -p context-read` | 123 passed, 93 ignored |
| `cargo test -p context-read --release` | 123 passed, 93 ignored |
| `cargo test -p context-trace -p context-search -p context-insert --no-default-features --features test-api` | 120 passed |
| `cargo test -p context-trace -p context-search -p context-insert` (default features, includes the insert gate) | 138 passed |
| `cargo test -p context-trace --features legacy-tests` | 195 passed |
| `cargo test -p context-search --features legacy-tests` | 77 passed |
| `cargo test -p context-insert --features legacy-tests` | 67 passed |
| `cargo test -p context-read --features legacy-tests` | 206 passed |
| `cargo build --workspace --all-targets --exclude context-editor-kernel --exclude sandbox-app` | ok |
| `cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features` | ok |

Compiler warnings come only from pre-existing code: `legacy/graph/search_path.rs`
and `logging/typed_debug.rs`.

### 6.R1.8 Implementation notes and deviations

1. `Settled` gained `gained: Vec<StageIndex>` (§4.2), so it is no longer
   `Copy`. `settle` keeps only staged indices in it: atoms and committed
   vertices are never candidates.
2. E1 inside `settle` pushes **all** children of a dissolution into a
   fixed host, not only the occurrences actually added. This is a superset
   and still O(children). It avoids reading the host's edit record back.
3. `ReadStage::settle` returns a small `Settlement { settled, stabilized }`
   struct (§4.2 allowed either form).
4. New error variant `PositionedError::FixedParentCount { vertex, count }`.
   `stabilize` caps `count` at 2.
5. The §5.2 default size is reduced to stay under ~10 s: inputs ≤ 6 with
   ≤ 1 earlier input ≤ 4. The full R6 size is the `#[ignore]` test
   `small_inputs_follow_rule_s_full`. It is green in release (§6.R1.2).
6. (T) asserts what §5.1 states: ≥ 2 left contexts, and right contexts
   that are not a single atom. It is never exercised on real reads (§6.R1.1).
   It is exercised only under the mutations (§6.R1.6).
7. (R) checks strings that are newly stable at the observed step, using
   the lane's own count of fixed parent occurrences.
8. **Open for review.** Rule S is vacuous in the reader (§6.R1.1). The
   stabilization machinery is kept as specified:
   - the stable prefix, `Swap`, the `settle` skip, `stabilize`;
   - `stabilize_candidates`, and the gained records.

   It costs nothing measurable, but it never marks anything. Options:
   - (a) keep it as is, for a later consumer with a different base, e.g.
     streaming commits that make vertices committed mid-read;
   - (b) remove the stabilization machinery;
   - (c) design a cheap base case. This would need a fixed fact about
     occurrences inside the root. It is not covered by Theorem S and is
     not attempted here.

   **Decided (2026-11-26): (a) keep as is.**

### 6.R1.9 Probe: Rule S′ (not adopted in revision 1; adopted in revision 2)

Candidate base case for option (c), probed on 2026-11-26 and then
reverted. A parent occurrence `(U, o)` of `t` also counts if `t` ends
strictly inside `U` (`o + |t| < |U|`), for any `U` (root and unstable
vertices included). Such an occurrence has a fixed right atom, because
staged content never changes and the root only grows at its end. Left
contexts are always fixed.

The proof of Theorem S carries over with one change. In step 3, a
parent's boundary bounds `M` because every current vertex is maximal
(M4), and an open occurrence stops the extension at the cursor. This
replaces H for the parent.

Probe setup: every unstable staged vertex was fed as a candidate, so it
was not event-driven. With events, one more trigger would be needed:
when the root grows, its previously last child (whose occurrence now
ends inside the root) becomes a candidate.

Results, with all lane checks including (T) green:

| Lane | Steps | Stabilized | Coverage |
|---|---|---|---|
| fixture lane | 701 | 66 | 33.0 % |
| exhaustive, default | 14,178 | 3,474 | 55.4 % |
| exhaustive, full | 380,250 | 116,038 | 60.1 % |

## 7. Decisions for review

| # | Question | Proposal |
|---|---|---|
| R1 | Count fixed parent **occurrences** or distinct fixed **parents**? | Occurrences (same unit as G6; Theorem S needs only distinct occurrences; the probe found 14 vs 13, both sound). |
| R2 | Worklist inside `GraphEdit` (`stabilize_candidates`) or in the reader? | Inside `GraphEdit`, next to `settle` (same pattern, same records, one edit per step). |
| R3 | Run stabilization in the same edit as `settle`? | Yes: avoids carrying renamed candidates across edits and a second `finish`. |
| R4 | Keep Theorem E anywhere? | No. Remove it from code and tests; mark C.7 superseded in the dissolution plan. Soundness rests on Theorem S only. |
| R5 | Keep the stable slot prefix mechanism? | Yes, unchanged (needed by the rule; cheap). |
| R6 | Exhaustive lane size (§5.2) | `{a,b}`, inputs ≤ 6, corpora ≤ 2 inputs ≤ 4; adjust to the time budget. |
| R7 (rev. 2) | Rule S (vacuous) or Rule S′? | Rule S′ (approved 2026-11-26): case (i) gives the base case; the stable machinery is kept. |
