---
tags: `#plan` `#context-read` `#context-insert` `#context-search` `#positioned-composition` `#reader` `#expansion`
summary: Positioned reader (roadmap steps 4–5), revision 2. The legacy reader loop is kept unchanged (segments; per step root-postfix expansion first, else forward match; every accepted match inserted; commit before the next search). Only root edits change: the root stays the input's own vertex, an owned root grows in place through GraphEdit::extend, the postfix chain is the chain of last children, and closure is one composition update without complements. Prototype R0 (read_legacy_port.py) reproduces the oracle on 164 of 166 fixture reads; the 2 others are provisional retention (roadmap step 5). Integration with insertion revision 3.
status: 📋 draft revision 2 — algorithm fixed by legacy and prototype R0; tasks R1–R3 ready after the insert plan
date: 2026-11-22
parent: 20261120_PLAN_POSITIONED_READER_OPEN_WORK.md (steps 4–5)
depends_on: 20261122_PLAN_POSITIONED_INSERT.md (revision 3), 20261120_PLAN_POSITIONED_SEARCH.md (done)
---

# Plan: Positioned Reader

**Crates:** `context-read` (reader), `context-insert` (`insert_match`),
`context-search` (search), `context-trace` (`GraphEdit`).
**Roadmap:** steps 4 and 5 of
[20261120_PLAN_POSITIONED_READER_OPEN_WORK](20261120_PLAN_POSITIONED_READER_OPEN_WORK.md).
**Insertion:**
[20261122_PLAN_POSITIONED_INSERT](20261122_PLAN_POSITIONED_INSERT.md)
(revision 3), implemented first.
**Prototype:**
[`read_legacy_port.py`](../../../context-insert/agents/prototypes/read_legacy_port.py)
(R0; execution manual in the prototypes
[README](../../../context-insert/agents/prototypes/README.md)).

The reader algorithm is the legacy one (`context-read/src/legacy`). This
plan does not redesign it: it maps each legacy operation to vertices that
store a composition (cover list) instead of rows, and states what the
positioned model changes in the root edits.

## 0. Changes from the first draft

| # | First draft | Revision 2 | Why |
|---|---|---|---|
| 1 | Input cover owned by the reader, not a vertex until input end | The root is the input's own vertex, as legacy `RootManager`'s root. An owned root grows in place | Everything read so far is represented by a token, so it follows the graph invariants and is searchable. Repeats inside the input are found by the normal search (22 of 57 insertions of the prototype have the input root as host). The open problem "self-repeats" disappears |
| 2 | Seam scan: search `I[x..]` for each offset `x` right to left after the forward step | Legacy root expansion: walk the root's **postfix tokens largest first**, search `[postfix, rest]`, accept the first that extends, insert it, integrate it into the root. It runs **before** the forward step at every junction | The seam operation of legacy, unchanged. It finds the largest known span that overlaps the consumed input and continues into the unread input. The offset scan was a reinvention and did not insert the overlap token |
| 3 | Retention decision (`Retention` trait, "maximal on both sides" hypothesis) | None in the loop: every accepted match (forward or expansion) is inserted, as legacy `materialize` does | Legacy has no separate retention step. Prototype: 164 of 166 reads equal the oracle; the 2 others are provisional retention (§5) |
| 4 | Publication of the input at the end through `GraphEdit::create` | No publication step: the root is the input's vertex at all times | Follows from 1 |
| 5 | `Cover::of_match`, `Cover::merge`, `Publication::translate` planned here | Not needed | The reader holds only tokens and offsets across an edit, never paths (§4.3) |

## 1. Legacy reader (`context-read/src/legacy`)

### 1.1 Module map

| Module | Role |
|---|---|
| `pipeline/mod.rs` | `ReadCtx`: iterates segments, owns the `RootManager`; `read_sequence` returns the final root |
| `segment.rs`, `input.rs`, `request.rs` | characters → atoms (an atom seen for the first time is created at once); `SegmentIter` splits the atom stream into `NextSegment { unknown, known }` runs |
| `expansion/block.rs` | `BlockExpansionCtx`: for one known run, loop `ExpansionCtx::next` → `RootManager::commit` → `observe_commit` |
| `expansion/mod.rs` | `ExpansionCtx::next`: one step = root expansion (`expand_root`) or, if none, forward match (`match_forward`) |
| `expansion/closure.rs` | `prepare` / `ClosurePlan::apply`: choose the closure bound (editable root: its tail; else the root) and run `context_insert::legacy::Overlap::close` |
| `pipeline/root.rs` | `RootManager`: the root token, ownership, commits of `Span::{Append, ReplaceRoot, ReplaceTail}` |

### 1.2 Algorithm

1. **Segments.** Unknown atoms (first seen) cannot occur in any other
   token; they are appended to the root (`append_pattern`).
2. **Known run.** `BlockExpansionCtx::process` loops `ExpansionCtx::next`.
   Each step produces one `Span`, which is committed at once, so the next
   search sees the updated root and graph. `cursor` counts the atoms of the
   run consumed so far. One step at the junction `j` (the end of the root):
   1. **Root expansion** (`expand_root`, only if `cursor > 0`). Walk the
      root's postfix tokens, largest first (`postfix_iter`: the widest last
      child at each level; the root itself is not yielded). Skip postfixes
      wider than `cursor` (the seam bound). For postfix `P`, search
      `[P, rest of run]`. The first response that matches more than `P` is
      accepted and materialized (split + join of that response) into the
      expansion `E`, which starts with `P`.
      - What it finds: anything read so far is a token, so its postfixes
        are tokens that follow the invariants. Going from the largest
        postfix down, the first that extends gives the **largest known span
        that overlaps the consumed input and continues into the unread
        input**. `E` is compatible with the context right of the junction,
        but its occurrence here has not been seen with the root's context
        left of `P`.
   2. **Closure** (`closure::prepare`, `Overlap::close`). The root `X`
      (or its tail, if the root is editable) and `E` overlap on `P`.
      `Overlap::close` builds `C = [L, E] = [X, T]` frame by frame along the
      anchor path (complements `L`, `T` by scoped split/join, anchor frames
      translated through each `Publication`). A self-expansion
      (`E == X`) is closed in one step (`ClosurePolicy::SingleStep`), and an
      editable root is not edited in place when `E` is the root.
      Commit `ReplaceTail` (editable root) or `ReplaceRoot`.
   3. **Forward match** (`match_forward`), if no postfix extends.
      `insert_next_match(rest of run)`: search the longest occurring prefix
      and materialize it if it is not a token. If `[root, token]` is
      already a token, commit `ReplaceRoot`; otherwise `Append(token)`.
3. **Root** (`RootManager::commit`). The root is a vertex. A root created
   by this read (owned: one row, no parents) grows in place
   (`append_to_owned_pattern`, `replace_owned_tail`). An adopted token is
   never edited: appending to it allocates `[root, token]`, which is owned.
   Appending the root to itself also allocates a new root.
4. **Result.** After the last segment, the root is the input's vertex.

## 2. Target semantics

**`read(graph, I)` turns the graph of corpus `C` into `oracle(C + [I])`**,
with one known exception: tokens that a prefix of `I` makes frequent but
`I` as a whole does not (provisional retention, §5).

Oracle examples, read with the prototype:

| Corpus `C` | Read `I` | Steps (prototype trace) | Result |
|---|---|---|---|
| `abcd, bcde` | `xabcdey` | append `x`; forward `abcd`; expansion `bcd → bcde@2`; append `y` | `xabcdey = x@0, abcd@1, bcde@2, y@6` = oracle |
| `abc` | `abcabca` | forward `abc`; forward `abc` (new root `abc@0, abc@3`); expansion `abc → abca@3` (inserted into the root) | `abcabca = abca@0, abca@3`, `abca = abc@0, a@3` = oracle |
| (empty) | `aaa` | append `a` (unknown); forward `a` (new root `aa`); expansion `a → aa@1`, a self-expansion (`E` is the root) | `aaa = aa@0, aa@1` = oracle |
| (empty) | `abcabca` | as with corpus `abc`, but `abc` is created by the read itself | extra token `abc`; the oracle has `abca = a@0, b@1, c@2, a@3` (§5) |

## 3. Algorithm in the positioned model

### 3.1 Loop (unchanged)

```text
read(graph, chars):
  root := RootManager::new()
  for NextSegment { unknown, known } in segments(chars):   # atoms created on first sight
    for a in unknown: root.commit(Append(a))
    cursor := 0
    while cursor < |known|:
      rest := known[cursor..]
      span := expand(root, rest, cursor)                    # 3.2
           or forward(root, rest)                           # 3.3
      cursor += consumed(span)
      root.commit(span)                                     # 3.4, before the next search
  return root.token
```

Searches are positioned search (`Find::find_ancestor`) with token queries.
Every non-token match is turned into a token by `insert_match(response)`
(insert plan revision 3), which consumes the search's response.

### 3.2 Root expansion (successor of `expand_root`)

```text
expand(root, rest, cursor):
  if cursor == 0: return None
  for P in postfixes(root.token):                 # largest first
    if |P| > cursor: continue                     # seam bound (legacy)
    response := search([P] ++ rest)
    if response matches more than |P|:
      E := token of response (insert_match if not EntireRoot)
      return Expand { postfix: P, expansion: E }  # consumes |E| − |P|
  return None
```

**Postfix chain.** `postfixes(v)` is the chain of last occurrences:
`d1 = comp(v).last`, `d2 = comp(d1).last`, … down to an atom
(`Composition::ending_at(width)`). In the positioned model this chain is
**exactly the set of all postfix tokens of `v`**, largest first:

- the last occurrence is the only maximal occurrence ending at `|v|`
  (offsets and ends strictly increase, G2);
- every postfix token `Q` of `v` (`Q ≠ v`) lies, by G2, inside a maximal
  occurrence ending at `|v|`, i.e. inside `d1`, and is then `d1` or a
  postfix token of `d1`. Induction gives the claim.

Legacy `postfix_iter` walks the widest last child over all rows; with rows
that is not always every postfix token. Positioned compositions make it
complete without extra work.

The search query `[P, rest]` starts with the token `P`, so the search
climbs from `P`'s parents. The input root is a vertex, so occurrences of
`P` inside the input itself are found as well (self-repeats, §3.6).

### 3.3 Forward match (successor of `match_forward`)

`response := search(rest)` gives the longest occurring prefix `I[j, m)`.
`EntireRoot` (or a single atom): the token `t` exists. Otherwise
`t := insert_match(response)`. The span is `Append(t)`, or
`ReplaceRoot { token: Y }` if `[root, t]` is a token `Y` (complete search
of `[root, t]`, legacy `find_complete`).

### 3.4 Root commit (successor of `RootManager::commit`)

The root keeps legacy ownership: **owned** = created by this read and
parentless. Only an owned root is edited, and only through
`GraphEdit::extend` (insert plan §8).

| Span | Legacy commit | Positioned commit |
|---|---|---|
| `Append(t)`, no root | adopt `t` | adopt `t` (not owned) |
| `Append(t)`, owned root `R`, `t ≠ R` | `append_to_owned_pattern` | `extend(R, t@|R|)` |
| `Append(t)`, otherwise | `insert_pattern([R, t])`, owned | `create([R@0, t@|R|])`, owned |
| `ReplaceRoot { token: Y }` | adopt `Y` | adopt `Y` (not owned) |
| `Expand { P, E }`, owned root `R`, `E ≠ R` (legacy `ReplaceTail`) | `Overlap::close` on the tail + `replace_owned_tail` | `extend(R, E@o)` with `o = |R| − |P|` |
| `Expand { P, E }`, otherwise (legacy `ReplaceRoot` after closure; `E == R` is the self-expansion) | `Overlap::close` on the root, adopt the closure | `create(maximal([R@0, E@o]))`, owned |
| `Expand`, `R[0, o) + E` already a token | (closure reuses an existing union) | adopt it (never seen on the fixtures) |

`extend(R, t@o)` sets `comp(R) := maximal(comp(R) ∪ {t@o})` and
`|R| := o + |t|`. For `Append`, `o = |R|`. For `Expand`, `o < |R|`:
`maximal` removes the occurrences inside `E` (the old tail up to `P`),
which is what legacy `ReplaceTail` does with rows.

**Closure needs no complement and no new token.** In legacy, the root
`L P` and the expansion `P T` share `P`, and closure builds `L`, `T` and
`[L, E] = [X, T]` frame by frame because a row cannot express an overlap.
A positioned composition lists `E@o` next to the occurrences it overlaps,
so closure is one composition update. The only new overlaps are between
`E@o` and the root's occurrences `c@a` crossing `o` (`a < o < a + |c|`).
Their overlap `[o, a + |c|)` lies inside `P@o`, so it is the overlap of two
token occurrences in `R`, `c@a` and `P@o`, and is a token by:

> **Lemma (intersection closure).** If tokens `A` and `B` occur at
> overlapping positions inside a vertex `v`, their overlap is a token.
> *Sketch:* induction on `|v|`. By G2, `A` and `B` lie inside maximal
> occurrences `a'` and `b'` of `comp(v)`. If `a' = b'`, recurse into it.
> Otherwise `Z = a' ∩ b'` is a token (G4), `A ∩ Z` and `B ∩ Z` are tokens by
> induction in `a'` and `b'`, and both occur in `Z`, so their overlap
> `A ∩ B` is a token by induction in `Z`.

So G4 holds for the root after every expansion. The prototype checks G2
and G4 after every owned-root edit: 0 violations (§6).

### 3.5 Invariants of the root

The root is a vertex at all times and satisfies G1–G5 (insert plan §3.1)
after every commit:

- **G2/G4:** `extend` and `create` keep the composition maximal; G4 by the
  lemma; completeness (no token crossing a junction is missed) is what the
  loop order guarantees: every junction is probed by expansion before
  anything is appended after it, largest postfix first. Checked against the
  brute-force composition on every edit of the prototype; a written proof
  is open (§7).
- **G1:** the content of an owned root does not occur elsewhere (the
  forward and expansion searches would have matched it). Asserted by the
  prototype on every edit.
- **G3:** maintained by `GraphEdit` (parent links store `(parent, offset)`
  without the parent's width, so growing a parentless root touches only the
  links of added and removed children).
- **Parentless while owned:** the root becomes a child only through
  `create([R@0, …])`, which ends ownership. So `extend` never invalidates
  another composition.

### 3.6 Self-repeats

No special case. The root is a vertex with reciprocal links, so a search
for `[P, rest]` or for `rest` finds occurrences inside the root like any
other host, and `insert_match` inserts into the root like into any other
vertex (the root is the response root; insert plan §2). Prototype: 22 of
57 insertions have the input root as host. The self-expansion rule (§3.4,
`E == R` → new root) keeps legacy `ClosurePolicy::SingleStep`: without it,
6 repeat scenarios (`aaa`, `ababab`, …) lose a token and 1 fails.

## 4. Integration with insertion

### 4.1 Calls

| Reader | Insertion (revision 3) |
|---|---|
| forward match is not a token | `insert_match(response)` → `Created { token, .. }` |
| expansion match is not a token | `insert_match(response)`; the root may be the response root |
| match is a token | no call (`EntireRoot`); legacy `insert_next_match` returns `Complete`/`NoExpansion` |
| root edits | `GraphEdit::extend` / `GraphEdit::create` (insert plan §8), not insertion |
| new atoms | `GraphEdit::atom` (insert plan D8: no mutation outside the layer) |

The reader uses only the returned token. The `Publication` is not needed:
after `insert_match`, the reader reads the root's composition from the
graph again (it may have changed when the root was a split node or host).

### 4.2 Contract per step

`insert_match` turns the graph into `oracle(C' + [N])`, where `C'` includes
the input read so far (the root). The reader then commits one span. After
each commit, the root is a valid vertex of the input read so far, so the
next search runs on a valid graph.

### 4.3 No stale state

Each search runs after the previous commit, and its response is consumed
by `insert_match` before the next edit. Across edits the reader keeps only
tokens (`root`, `P`, `E`) and input offsets; tokens never change their
string. So no response is stale (generation check, insert plan P2) and
no path translation is needed. Legacy needed `Publication::translate_chain`
only for the anchor path of row closure, which positioned closure does not
use.

### 4.4 What insertion must support for the reader

- A response whose root is the input root (an owned, parentless vertex).
  Nothing special: insertion edits the root's composition through
  `add_occurrences` (same width).
- `GraphEdit::extend` in the shared edit layer (insert plan §8).

## 5. Retention

Legacy has no separate retention decision: every accepted match is
inserted. The prototype reproduces the oracle with that rule on 164 of 166
reads. The two exceptions are **provisional tokens**:

| Scenario | Created during the read | Oracle |
|---|---|---|
| `repeat_abcabca` | `abc` (repeat in `abcabc`) | `abca = a@0, b@1, c@2, a@3`, no `abc` |
| `repeat_ababa` | `ab` (repeat in `abab`) | `aba = a@0, b@1, a@2`, no `ab` |

A prefix of the input makes the token frequent; at the end of the input all
its occurrences lie inside one larger token, so the oracle does not retain
it. This is the roadmap's known fact "retention is not monotone within an
input" (step 5). The legacy reader has the same behavior.

Proposal (P-R1): step 4 keeps the legacy rule. Its gate expects oracle
equality on all scenarios except these, which are listed as expected
deviations and become the entry tests of step 5 (demoting provisional
tokens at input end: a token created by this read whose occurrences all lie
inside one other token is dissolved into that token). This replaces the
"supplied retention" of the first draft and changes the roadmap's step 4
milestone (`abcabca` without `abc`), which needs the step 5 rule.

## 6. Evidence (prototype R0)

`python3 read_legacy_port.py` reads every input of every v3 scenario into
the graph of its predecessors and compares with the oracle:

| Check | Result |
|---|---|
| Reads equal to the oracle graph | 164 / 166 (66 scenarios) |
| Mismatches | 2, both provisional retention (§5) |
| G2 / G4 violations after an owned-root edit | 0 / 0 |
| G1 violations (owned root content exists) / errors | 0 / 0 |
| Forward searches / expansion probes | 160 / 105 |
| Insertions / with the input root as host | 57 / 22 |
| Commits: append in place / new root `[R, t]` | 140 / 147 |
| Closures: in place / new root / self-expansion | 22 / 23 / 8 |
| Without the seam bound (`--no-seam-bound`) | identical (the bound never filters on the fixtures) |
| Without the self-expansion rule | 6 repeat scenarios lose a token, 1 fails (`deep_fibonacci`) |

## 7. Open questions

1. **Provisional retention** (§5, P-R1): confirm that step 4 keeps the
   legacy rule and step 5 adds demotion.
2. **Proof of G2 for root edits.** The prototype checks it on every edit;
   the argument "every junction is probed largest postfix first before
   anything is appended" needs a written proof, in particular for tokens
   that cross a junction but start at a non-token postfix.
3. **Seam bound.** Legacy skips postfixes wider than the consumed part of
   the run. The fixtures never exercise it; keep it (legacy) and add a
   fixture that does, or drop it after a proof.
4. **Search reuse.** The roadmap asks that no position covered by a
   `Response` is searched again. Expansion probes re-search the consumed
   postfix `P`, but as one token, not atom by atom. Define the counter as
   "no atom is consumed twice by a forward search" or reuse the forward
   response for the first probe.

## 8. Phases

| Phase | Content | Exit |
|---|---|---|
| R0 | Prototype `read_legacy_port.py` | **Done** (§6) |
| R1 | `GraphEdit::extend` (with insert plan task 3.1); `context-read` crate setup; promote `segment.rs`/`input.rs` out of `legacy` if needed; positioned `RootManager` with the commits of §3.4 | Unit tests: commits keep `validate()` green; ownership rules |
| R2 | Forward match and segments (§3.3), no expansion | Named cases without expansion (`abcd, bcde` read as inputs; `abc` then `abcabc`) |
| R3 | Root expansion and closure (§3.2, §3.4), self-expansion rule | Step 4 gate: every scenario of `context-read/tests/reader_behavior.rs` equals its v3 fixture (`diff` of the whole graph), except the expected deviations of §5 |
| R4 | Step 5: demotion of provisional tokens, streaming retention | Roadmap step 5 gate |

## 9. Gate (step 4)

```sh
cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api
cargo test -p context-read --no-default-features --features test-api
cargo test -p context-read --features legacy-tests
```

Complete-graph equality with the v3 fixtures for every scenario except the
expected deviations; `validate()` after every read; no algorithm reads
content or builds content-keyed structures.
