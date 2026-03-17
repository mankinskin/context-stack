---
tags: `#plan` `#context-read` `#aaa` `#decomposition` `#overlap` `#expansion` `#root-manager` `#segmentation`
summary: Corrected implementation plan for fixing the `aaa` decomposition failure. The root cause is that `ExpansionCtx` never yields a `WithOverlap` state when the anchor is an atom, because `postfix_iter` on an atom yields nothing. The fix requires a dedicated overlap detection path for atom anchors that checks whether the anchor is a prefix of the matched token `t1`, and if so, builds the `WithOverlap` state directly using the anchor as both the postfix and the left complement.
status: 📋 ready
date: 2026-03-17
related_interview: ../interviews/20260317_INTERVIEW_AAA_DECOMPOSITION_NEXT_STEP.md
related_analysis: ../analysis/20260317_ANALYSIS_AAA_SEGMENT_BOUNDARY_AND_OUTER_LOOP.md
related_analysis_2: ../analysis/20260315_ANALYSIS_AAA_DECOMPOSITION_NEXT_STEP.md
blocking: `repetition_aaa_both_decompositions` in `src/tests/linear.rs`, `validate_three_repeated` in `src/tests/ngrams_validation.rs`
---

# Plan: `aaa` Decomposition Fix

**Date:** 2026-03-17
**Scope:** fix the missing `[aa, a]` decomposition for `aaa` and generalise the fix to all repeated-pattern symmetry cases
**Primary crate:** `context-read`
**Secondary crate:** none — no changes outside `context-read` are expected

---

## Objective

After reading `aaa` from an empty graph the graph must contain:

```
aa  => [[a, a]]
aaa => [[a, aa], [aa, a]]
```

Currently only `[a, aa]` is produced. The fix must also generalise naturally to:

- `aaaa`
- `abab`, `ababa`, `ababab`
- `abcabcabc`, `xyzxyzxyz`

without any case-specific logic.

---

## Semantic invariant

The root is always an eventually-consistent hypergraph node. After each commit it must satisfy:

> the root represents the composed string with a minimal child neighbourhood but full reachability of all existing tokens that represent any part of that string.

Every valid binary adjacent decomposition of the current root — expressible in terms of already-known tokens — must be present as a child pattern of the root.

---

## Exact execution trace for `aaa` today

Understanding the precise current behaviour is necessary before describing the fix.

### Segment split

`SegmentIter` classifies the three characters lazily:

- position 0: `a` is new → `NewAtomIndex::New` → goes into `unknown`
- position 1: `a` is now known → `NewAtomIndex::Known` → goes into `known`
- position 2: `a` is known → `NewAtomIndex::Known` → goes into `known`

Result: one `NextSegment { unknown: [a], known: [a, a] }`.

### After the unknown segment

`RootManager::append_pattern([a])` is called.

- `root = a`, `anchor = None` (anchor is not set by the unknown path), `flat_root = true`

Note: `anchor` is `None` here because `append_token` / `append_pattern` do not set `self.anchor`. The anchor is only set by `commit_state`. This means `BlockExpansionCtx` is constructed with `anchor = root.anchor() = None`.

### Inside `BlockExpansionCtx`

```
atoms   = [a, a]       // the known segment
anchor  = None         // from RootManager::anchor()
cursor  = 0
```

### `ExpansionCtx::next()` — step 1

- `remaining = [a, a]`, `remaining.len() == 2` → multi-token path
- `anchor_is_atom = self.anchor.map(|a| *a.width() == 1).unwrap_or(false)` → `false` (anchor is `None`)
- Guard 1: `if !anchor_is_atom` → **condition is true**, so overlap probe runs
- `insert_next_match([a, a])` → `aa` (width 2)
- `find_overlap(anchor=None, ...)` — but anchor is `None` so the inner `if let Some(anchor)` guard is not entered
- overlap probe effectively skipped because `anchor` is `None`
- yields `BandState::Single(aa)`, cursor → 2

### `commit_state(Single(aa))`

- `try_extend_tail_with(aa)` — fails (`aa` is width 2, not 1)
- `root = a`, `flat_root = true`, `can_extend_with(aa)`:
  - root vertex `a` has one child pattern and no parents → `true`
- branch: **`extend_root(aa)`** — extends root `a` in-place to `[a, aa]`
- `anchor = root = [a, aa]`

### `ExpansionCtx::next()` — step 2

- cursor = 2 = atoms.len() → returns `None`

### Final state

- `root = aaa = [a, aa]`
- `anchor = aaa`
- `aa => [[a, a]]`, `aaa => [[a, aa]]`
- `[aa, a]` is never created

---

## Root cause analysis

There are two compounding problems.

### Problem 1 — anchor is `None` at the start of the known block

`RootManager::append_pattern` / `append_token` do not set `self.anchor`. Only `commit_state` sets it. So when the unknown segment ends and `BlockExpansionCtx` is constructed, `root.anchor()` returns `None` even though `root = a` is a perfectly valid left context.

This means the expansion loop starts with no left-context information at all.

### Problem 2 — `postfix_iter` on an atom yields nothing

Even if the anchor were correctly set to `a`, the existing `find_overlap` logic could not produce the `[aa, a]` decomposition.

`find_overlap` works by iterating `anchor.postfix_iter()`. The `postfix_iter` implementation (`PostfixExpandingPolicy`) descends into child patterns and takes the last child at each level. An atom has no child patterns. Therefore `postfix_iter` on an atom yields **nothing** — the iterator returns `None` immediately.

Consequence: `find_overlap(anchor=a, t1=aa, cursor=0)` iterates zero postfixes and returns `None`. Even with a correct anchor, no overlap state is ever built for atom anchors.

### Why `[a, aa]` is produced but `[aa, a]` is not

The sequential path still works: `insert_next_match([a, a])` → `aa` → `extend_root` → root becomes `[a, aa]`. The `extend_root` in-place mutation is what produces the one working decomposition. But because `aa` is a **semantic compound** that already has its own child pattern `[a, a]` and is about to acquire parents, the system must not merely extend `a` in-place — it must create a new root node `aaa` that records all valid decompositions. The in-place extension happens to produce `[a, aa]` as a side effect, but it never explores the symmetric `[aa, a]`.

### The correct picture

After `insert_next_match([a, a])` produces `aa`, the system should recognise:

- the previous root `a` (width 1) is the left context
- the matched token `aa` (width 2) spans exactly `anchor.width + aa.width - anchor.width = aa.width` atoms starting at cursor 0
- the anchor `a` is a **prefix** of `aa` — i.e. `aa` starts with `a`
- therefore `aa` can also be placed starting at atom position 0 (the anchor's atom), producing the decomposition `[aa, a]` where `aa` covers atoms 0–1 and `a` covers atom 2

This is an overlap of the anchor `a` into the beginning of `aa`. The right complement of this overlap is `a` (the atom at position 2, which is `t1_cursor + t1_width - anchor_width = 0 + 2 - 1 = 1` in the atom slice, but that is just the single trailing `a` — which is not in the slice because the slice only has `[a, a]` and both were consumed producing `aa`).

The key insight from the author:

> `[aa, a]` should be created because `aa` is the previous root, cannot be modified, and is thus replaced by a larger root where it is included with the right complement from the overlap.

So: after `aa` is found and the previous root is `a`, the expansion must yield a `WithOverlap` state encoding:

- primary band: `[complement, t1]` = `[a, aa]` — the sequential decomposition
- overlap band: `[complement, t2]` = `[a, aa]` where t2 = aa starting from position 0

Wait — both decompositions have `complement = a`. The overlap in this case is `aa` itself appearing at position 0. The complement (left side before the overlap token begins) is the empty prefix, i.e. the token that represents "the part of the root before `aa` starts" — but `aa` starts at atom 0, so there is no left complement. The complement is the atom `a` in a different sense: the **right** complement is what remains after `aa` ends.

The `bundle_overlap` mechanism in `context-insert` will produce `aaa` with both child patterns when given:

- `t1 = aa` (sequential match)
- `t2 = aa` (overlap match, same token, starting one position earlier)
- the paths encoding where each sits relative to the anchor

---

## What `WithOverlap` must encode for `aaa`

For the non-atom anchor case (e.g. `ababab`), the existing mechanism works:

```
anchor = abab
postfix of anchor = ab  (width 2)
t1 = abab  (matched from remaining)
overlap_start = cursor + t1_width - postfix_width
t2 = insert_next_match(atoms[overlap_start..]) → ababab
complement = left part of anchor before postfix = ab
primary band = [complement, t1] = [ab, abab]
overlap band  = [complement, t2] = [ab, ababab]
```

For the atom anchor case (`aaa`):

```
anchor = a          (width 1, the unknown-segment root)
t1 = aa             (matched from known segment [a, a])
"postfix" = a       (the atom itself — it IS its own postfix, width 1)
overlap_start = 0 + 2 - 1 = 1
t2 = insert_next_match(atoms[1..]) = insert_next_match([a]) → SingleIndex error
```

The problem: `atoms[overlap_start..] = atoms[1..] = [a]` is a single atom. `insert_next_match` requires ≥ 2 tokens and fails. So t2 cannot be found via the existing query mechanism.

However, **t2 is just `aa` again** — because the anchor `a` (width 1) is itself a prefix of `t1 = aa` (width 2). The overlap is: `aa` starting at atom 0 (the anchor's position) covers the same span as `[anchor + first_atom_of_t1]`. The `t2` for this overlap is `t1` itself when `t1` starts with the anchor.

This special case — **anchor is a prefix of t1** — can be detected directly without querying `atoms[overlap_start..]`. The condition is:

```
t1 starts with anchor
⟺  anchor.width < t1.width
    AND insert_next_match([anchor_atom, ...t1_prefix_atoms]) = t1
```

For `aaa`: anchor = `a`, t1 = `aa`. Does `aa` start with `a`? Yes — `aa`'s first atom is `a`. So t2 = `aa` = t1. The right complement after t2 ends is `atoms[anchor.width + t2.width - anchor.width ..] = atoms[t1.width..] = atoms[2..] = []` — empty. But the **right complement of the full span** is `atoms[t1.width..] = []`... 

Actually the right complement is not from the atom slice. It is from the position after t2 ends in the **full string** (including the anchor atom). The full span is `anchor + atoms = [a] + [a, a]` = 3 atoms. t2 = `aa` spans atoms 0–1 of the full span. The remaining atom is atom 2 = `a`. That `a` is the right complement — and it is `atoms[t1.width - (anchor.width) ..] = atoms[1..] = [a]`. As a token, it is the atom `a` itself.

So for `aaa` the `WithOverlap` state should encode:
- `complement = a` (the right complement of t2 within the full span — i.e. the atom after t2 ends)

Wait — let us re-read how `build_overlap_state` defines `complement`:

```rust
let start_bound = anchor.width().0 - postfix.width().0;
// complement is the part of anchor to the LEFT of postfix
let complement = ComplementBuilder::new(expansion_link).build(&graph);
let primary = Band { pattern: [complement, t1] }
let overlap = Band { pattern: [complement, t2] }
```

So `complement` is the **left** part of `anchor` that precedes the shared postfix. For `aaa`:

- anchor = `a` (width 1)
- postfix = `a` (width 1, the atom is its own postfix)
- `start_bound = anchor.width - postfix.width = 1 - 1 = 0`
- complement = the part of anchor to the left of postfix at start_bound 0 = **nothing / empty**

When `start_bound = 0` the complement is the anchor root itself (per the existing `ComplementBuilder::build` logic: `if intersection_start == 0 { return root; }`).

So for the atom-anchor case:
- `complement = anchor = a`
- `t1 = aa`
- `t2 = aa` (same token — the overlap token is t1 itself starting from the anchor's position)
- `primary = [a, aa]` → `[a, aa]` = `[complement, t1]`
- `overlap = [a, aa]` → but wait, this is the same pattern

That cannot be right. Let us re-examine.

For the existing non-atom case, `t1` and `t2` are **different tokens**:

```
anchor = abab, postfix = ab
t1 = abab   (sequential: matches from cursor)
t2 = ababab (overlap: matches from overlap_start, wider than postfix)
```

- `primary = [complement, t1] = [ab, abab]` — the full span under the sequential view
- `overlap = [complement, t2] = [ab, ababab]` — the full span under the overlap view

For `aaa`, if t2 = t1 = `aa` the two bands are identical. That means `bundle_overlap` receives two identical inputs and cannot produce two different child patterns.

**This means t2 must be different from t1.** The correct t2 for the `aaa` case is not `aa` — it must be the token that spans `[anchor + t1]` = `[a, aa]` = `aaa`. But `aaa` does not yet exist at this point.

So the correct overlap state for `aaa` is:

- anchor = `a` (width 1)
- postfix = `a` (the atom, width 1)
- t1 = `aa` (sequential match at cursor 0)
- overlap_start = cursor + t1.width - postfix.width = 0 + 2 - 1 = 1
- t2 = token matching `atoms[1..] + one more atom` — but atoms[1..] = `[a]` which is a single atom

The single-atom query is the hard constraint. `insert_next_match` requires ≥ 2 tokens. This is **by design** — a single atom is not a compound and has no match. So `find_overlap` correctly returns `None` here.

---

## Reframed root cause

The `aaa` case does not fit the **existing overlap detection model** at all. The existing model assumes:

> the anchor's postfix expands with the remaining atoms into a **wider** compound t2

For `aaa`, there are no remaining atoms after `aa` is matched. The third `a` was consumed inside `aa`. There is nothing left to expand into.

The correct model for `aaa` is different:

> after committing `aa` as the new compound, the **previous root `a`** is now the left context. The new compound `aa` spans atoms 0–1 of the full string. Because `aa` starts at the position of the previous root (atom 0), it can be used as the **left child** of the new root — giving `[aa, a]`. The right complement `a` is atom 2.

But atom 2 is not in the `atoms` slice — it **is** atom 1 of the slice (index 1, zero-based), which became the second element of `aa`. The `a` that appears as the right complement in `[aa, a]` is the atom at position `anchor.width` in the original atom sequence `[anchor] + atoms = [a, a, a]`.

The full-string atom slice is `[a, a, a]`. `ExpansionCtx` only has `[a, a]` (the known segment). It is missing the leading anchor atom.

**This is the fundamental gap**: `ExpansionCtx` cannot see the anchor atom as part of its query sequence. The overlap that produces `[aa, a]` requires starting `insert_next_match` from position 0 of `[anchor] + atoms = [a, a, a]` to find `aa` there — and then the right complement is `atoms[aa.width - anchor.width..] = atoms[1..] = [a]`.

---

## The correct fix

The fix has two parts.

### Part 1 — Set `anchor` after the unknown segment

`RootManager::append_token` / `append_pattern` must set `self.anchor = self.root` after appending. This ensures that when `BlockExpansionCtx` is constructed, `root.anchor()` returns the atom `a` rather than `None`.

This is a small, safe change. The anchor was conceptually always meant to be the semantic tail of the root. For a root built from unknown atoms, that tail is the root token itself (or its last child for multi-atom unknown segments).

### Part 2 — Atom-anchor overlap via prefix-of-t1 detection in `find_overlap`

When the anchor is an atom, `postfix_iter` yields nothing, so the existing postfix loop cannot find overlaps. A dedicated path is needed.

The condition is: when anchor is an atom and `t1.width > anchor.width`, check whether the atom sequence starting at `cursor - anchor.width` (i.e. including the anchor atom in the query) would produce a token wider than `t1`. Equivalently: does `t1` start with the anchor atom? If yes, t2 is the result of `insert_next_match` on `[anchor_atom] + atoms[cursor..cursor+t1_width]`.

But `anchor_atom` is not in the `atoms` slice. The `atoms` slice only contains the known segment. So `ExpansionCtx` needs access to the anchor as a queryable atom.

**The most direct approach**: in `find_overlap`, when anchor is an atom (width 1), construct the extended query `[anchor_atom] + atoms[cursor .. cursor + t1_width - 1]` and call `insert_next_match` on it. If the result is wider than `t1_width - 1` (i.e. it spans the anchor position too), the overlap is confirmed and t2 is that result.

Concretely for `aaa`:

```
anchor = a   (atom, width 1)
t1 = aa      (width 2)
cursor = 0
extended_query = [a, a]   (anchor atom + atoms[0..1] = atoms[0..0+2-1] = atoms[0..1] = [a])
actually extended_query = [anchor_token] + atoms[cursor .. cursor + t1_width - anchor_width]
                        = [a] + atoms[0..1] = [a] + [a] = [a, a]
insert_next_match([a, a]) = aa   (width 2 > anchor_width 1 → overlap confirmed)
t2 = aa
postfix = anchor = a   (the anchor is its own postfix for width-1 anchors)
next_cursor = cursor + t1_width = 0 + 2 = 2
```

This produces the correct overlap: `postfix = a`, `t2 = aa`, `next_cursor = 2`.

Then `build_overlap_state(anchor=a, t1=aa, postfix=a, t2=aa)`:

```
start_bound = anchor.width - postfix.width = 1 - 1 = 0
complement = ComplementBuilder { start_bound=0 } → returns anchor root = a
primary = [a, aa]
overlap = [a, aa]
```

Again both bands are identical. `bundle_overlap([a, aa], [a, aa])` cannot produce two distinct child patterns.

**This confirms**: the existing `WithOverlap` / `bundle_overlap` model as currently defined cannot directly encode the `aaa` case, because the two decompositions are `[a, aa]` and `[aa, a]` — they have **different left children** (`a` vs `aa`), not a shared complement with different right expansions.

---

## Revised model: the overlap for `aaa` is a root-replacement overlap

Looking at this more carefully through the lens of what the author described:

> `[aa, a]` should be created because `aa` is the previous root, cannot be modified, and is thus replaced by a larger root where it is included with the right complement from the overlap.

The previous root is `a` (width 1). The new compound `aa` (width 2) is found. The new root `aaa` (width 3) is created. At commit time, the system should:

1. Create `aaa = insert_pattern([a, aa])` — the sequential decomposition `[a, aa]`
2. Also add `insert_pattern([aa, a])` as a second child pattern of the same `aaa` node — the overlap decomposition `[aa, a]`

The second child pattern `[aa, a]` arises because:
- `aa` spans atoms 0–1 of the full string
- `a` spans atom 2 of the full string
- both are known tokens
- their concatenation equals the full root

This is not a postfix-expansion overlap in the existing sense. It is a **left-starting compound detection**: after finding the sequential decomposition `[a, aa]`, the system checks whether the **combined token** (the new root) can also be decomposed starting with a known compound at position 0. `insert_next_match([a, a, a])` starting at position 0 of the full string — including the anchor atom — would find `aa` (width 2), and the remainder `a` (width 1) is the right complement.

**This is exactly the operation that `ExpansionCtx` is missing**: after finding `t1 = aa` at cursor 0 of `atoms = [a, a]`, it should also query `insert_next_match([anchor] + atoms)` starting at position 0 to see whether the anchor participates in a wider left-starting compound. For `aaa`: `insert_next_match([a, a, a])` starting at 0 → finds `aa` (or `aaa` if it existed — but it doesn't yet, so `aa`) → remainder is `[a]` → decomposition `[aa, a]`.

The result `aa` at position 0 has width 2 ≤ total width 3, so the remainder `a` (width 1) is a valid right complement. The new root `aaa` gets child pattern `[aa, a]` added alongside `[a, aa]`.

---

## Concrete fix: extend `ExpansionCtx` with an anchor-inclusive probe

After `insert_next_match(remaining)` returns `t1`, and before yielding `BandState::Single(t1)`, `ExpansionCtx::next()` must run an additional probe when an anchor exists:

```
anchor_inclusive_probe(anchor, t1, cursor):
    query = [anchor_token] + atoms[cursor .. cursor + t1.width]
    // This is a sequence of anchor.width + t1.width atoms
    // starting at the anchor's virtual position (one step before cursor)
    result = insert_next_match(query)
    if result.token != t1 and result.width > anchor.width:
        // found a left-starting compound that crosses the anchor boundary
        t2 = result.token
        right_complement_start = cursor + t2.width - anchor.width
        right_complement = atoms[right_complement_start]  // or a token from the graph
        // yield WithOverlap encoding [right_complement, t1] vs [right_complement, t2]
        // where t2 starts at the anchor position
```

Wait — but if `result = aa` and `t1 = aa` they are the same token, and we're back to the same-bands problem.

For `aaa` specifically: `insert_next_match([a, a, a])` when neither `aaa` nor anything wider than `aa` is known yet returns `aa` — which equals `t1`. So the probe returns the same token.

The problem is **structural**: the `aaa` case requires creating a **new token** `aaa` with **two child patterns** simultaneously. Neither child pattern can be derived from an existing `WithOverlap` state using the current `bundle_overlap` API, because `bundle_overlap` works by taking two tokens (`t1`, `t2`) that share an overlap region, and the resulting bundled token is a new compound that contains both views.

For `aaa`, the two views are:
- view 1: `aaa = [a, aa]` — `a` then `aa`
- view 2: `aaa = [aa, a]` — `aa` then `a`

These do **not** share an overlap region in the existing sense. They are two different adjacent-binary decompositions of the same span. The `bundle_overlap` path is designed for cases like `abcabcabc` where a shared postfix-prefix region exists in the middle. For `aaa`, there is no such shared middle region.

---

## Final correct fix model

The `aaa` decomposition is a special case of what happens when **the new compound exactly spans `[anchor + known_segment]`** — i.e. when `t1.width == anchor.width + atoms.len()`. In this case the entire known segment was consumed forming t1, and t1 together with the anchor covers the full span. The two decompositions are:

- `[anchor, t1]` — sequential: anchor first, then t1
- `[t1_from_left, right_complement]` — where `t1_from_left` starts at the anchor position

For `aaa`: anchor = `a` (width 1), t1 = `aa` (width 2), full span = 3.
- `[a, aa]` — anchor then t1
- `[aa, a]` — `aa` starts at position 0 (anchor position), right complement = `a` (position 2)

The second decomposition requires knowing that `insert_next_match` applied to `[anchor_atom] + atoms` (starting from the anchor's position) produces a compound of width ≥ anchor.width + 1. For `aaa`: `insert_next_match([a, a])` = `aa` (width 2 > anchor.width 1) → `aa` starts at the anchor position → right complement = `atoms[aa.width - anchor.width..] = atoms[1..] = [a]` = atom `a`.

But `insert_next_match([a, a])` = `aa` = `t1`. So t1_from_left = t1 = `aa`. The right complement = `atoms[t1.width - anchor.width..] = atoms[1..] = [a]` = the atom `a`.

So the full span decomposition as `[t1, right_complement]` = `[aa, a]` is valid. This does not require `bundle_overlap` at all. It only requires calling `insert_pattern([t1, right_complement])` as a second child pattern on the new root.

**The fix in `RootManager::commit_state` for the `Single(t1)` path**:

When `commit_state` creates the new root as `insert_pattern([anchor, t1])` (i.e. `wrap_root(t1)` / `insert_pattern([root, t1])`), it must also check:

> if the anchor is set AND `t1.width > anchor.width` AND `insert_next_match([anchor_atom, ...t1_prefix])` produces a compound that starts at the anchor's position (i.e. returns a token of width `anchor.width + remainder.width` for some known remainder):

Actually the simplest formulation is:

> when `commit_state(Single(t1))` is about to call `wrap_root(t1)`, check whether the **current root** (which is the anchor) has width 1 and whether `t1.width ≥ 2`. If so, also add the pattern `[t1, right_atom_complement]` to the new root, where `right_atom_complement` is the last atom of t1's span that is not covered by the remaining atoms after t1.

No — this is getting complicated because `commit_state` does not have direct access to the atom slice.

**The cleanest correct fix** is:

`ExpansionCtx::next()`, after finding `t1`, should check whether the anchor covers the beginning of t1. Specifically:

```
if let Some(anchor) = self.anchor {
    // Check if anchor-as-left-child + right_complement gives a valid second decomposition.
    // This is the case when t1 starts with the anchor's atom sequence and
    // there is a known right complement.
    let anchor_width = *anchor.width();
    let t1_width = t1.width().0;
    if anchor_width <= t1_width {
        // The right complement starts at cursor + (t1_width - anchor_width)
        let right_start = self.cursor + t1_width - anchor_width;
        if right_start < self.atoms.len() {
            let right_complement_atom = self.atoms[right_start];
            // t1 used as left-starting compound with right_complement_atom
            // yields a second valid decomposition of the full span.
            // Encode this as a WithOverlap state.
            // postfix = anchor (the shared region)
            // t2 = t1  (same compound, used starting from anchor position)
            // right_complement = right_complement_atom
        }
    }
}
```

But t2 = t1 again. The `WithOverlap` representation cannot encode "t1 and t1 are the same compound, but in two different positions" with the current `bundle_overlap` API.

**Conclusion: the current `BandState::WithOverlap` / `bundle_overlap` model is insufficient for the `aaa` case.** The `aaa` fix requires a different mechanism: directly inserting a second child pattern `[t1, right_complement]` onto the new root in addition to the sequential `[anchor, t1]` pattern.

---

## Proposed fix: `commit_state` second-pattern insertion

Since `commit_state` is where the root is updated and new nodes are created, it is the correct place to also insert the symmetric decomposition when the conditions are met.

The fix is in `RootManager::commit_state`, in the `BandState::Single` path, specifically in the `wrap_root` branch (semantic root — no in-place extension).

After `insert_pattern([root, t1])` creates the new root:

```
new_root = insert_pattern([root, t1])   // gives [a, aa] for aaa
```

Additionally check: can `t1` be placed as the **left** child of new_root, with `root` as the **right** child? I.e. does `insert_pattern([t1, root])` produce the same new_root? For `aaa`: `insert_pattern([aa, a])` — this is a **different** pattern, so it produces or finds a token for the `[aa, a]` decomposition. We want this to be an additional child pattern of `new_root`, not a separate root.

The correct operation is not `insert_pattern([t1, root])` (which creates a new token) but `add_pattern_to([new_root, pattern=[t1, root]])` — i.e. adding `[aa, a]` as a second child pattern to the already-created `aaa` node.

Whether this operation exists in the graph API needs to be checked.

However, the condition must be precise to avoid adding spurious patterns. The condition is:

1. `root` (the previous root, which is the anchor) is a known compound with width ≥ 1
2. `t1` is a known compound with width ≥ 2
3. `t1.width > root.width` — otherwise `[t1, root]` would have t1 wider than the root, which changes the decomposition structure
4. `t1` starts with the same atoms as `root` — i.e. `root` is a prefix of `t1`

For `aaa`: root = `a` (width 1), t1 = `aa` (width 2). Is `a` a prefix of `aa`? Yes — `aa`'s first child pattern is `[a, a]`, and `a` is the first element. So condition 4 is satisfied.

The right complement in this case is `root` itself, since `new_root.width = root.width + t1.width` and the right complement has width `new_root.width - t1.width = root.width`.

So the second pattern is `[t1, root]` = `[aa, a]` — which uses the same `root` token as the right complement.

For generalisation: this is the case where **the sequential left child is the right complement of the overlap child**. The pattern `[t1, root]` uses root (the anchor) as a trailing suffix.

### Generalisation check

Does this condition generalise to `abcabcabc`?

For `abcabcabc`, after reading `abcabc`:
- root = `abcabc` (width 6)
- t1 = `abc` (width 3, matched from the third repetition)
- `t1.width (3) < root.width (6)` → condition 3 fails

So this condition does not fire for `abcabcabc`. Good — `abcabcabc` is handled by the existing non-atom overlap mechanism (postfix of `abcabc` is `abc`, which expands to `abcabc`, giving `[abc, abcabc]`).

Does it fire for `ababab`?

For `ababab`, after reading `abab`:
- root = `abab` (width 4)
- t1 from the third `ab`: `ab` (width 2)
- `t1.width (2) < root.width (4)` → condition 3 fails

Also does not fire. Good — `ababab` is handled by the existing overlap mechanism too.

So the condition `t1.width > root.width` correctly isolates the `aaa`-family cases (where the full known segment forms a token wider than the anchor/root) from the general overlap cases (where the known segment produces a shorter token than the current root).

### When does this fire?

This fires when `t1` spans the **entire known segment plus the anchor width** — i.e. when the anchor and the full known segment together form `t1`'s span. That is exactly the segment-boundary overlap case: `anchor + known_segment = t1` in terms of atom widths.

Concretely: `anchor.width + known_segment.len() = t1.width` → for `aaa`: `1 + 2 = ... wait, t1.width = 2 and anchor.width = 1` so `anchor.width + known_segment.len() = 1 + 2 = 3 ≠ t1.width = 2`.

Hmm. The condition `t1.width > root.width` is not quite right either. Let me re-examine.

For `aaa`: root = `a` (1), t1 = `aa` (2). `t1.width = 2 > root.width = 1`. ✓
For `ababab` after `abab`: root = `abab` (4), t1 = `ab` (2). `t1.width = 2 < root.width = 4`. ✗ (correct — does not fire)
For `abcabcabc` after `abcabc`: root = `abcabc` (6), t1 = `abc` (3). `t1.width = 3 < root.width = 6`. ✗ (correct)

What about `aaaa` after `aaa`?
- root = `aaa` (3), t1 = `a` (1) or `aa` (2) depending on what `insert_next_match` returns for `[a]` or `[a, a]`
- If `aaa` has parents (from the previous read), `can_extend_with` is false, `wrap_root` is called
- t1 = `aa` (2), root = `aaa` (3), `t1.width = 2 < root.width = 3` → does not fire

For `aaaa` the expected decompositions of the root are `[[aa, aa]]`. That is produced by the existing sequential path (two calls to `insert_next_match([a, a])` = `aa`, wrap `[aa, aa]`). No overlap is needed there.

So the condition `t1.width > root.width` correctly scopes this to the `aaa`-family without touching `aaaa`, `ababab`, `abcabcabc`.

---

## Precise fix specification

### Fix location

`src/pipeline/root.rs` — `RootManager::commit_state`, inside the `BandState::Single` branch, in the `wrap_root` sub-branch.

### Condition for the additional pattern

After `wrap_root(t1)` creates `new_root = insert_pattern([old_root, t1])`, additionally insert `[t1, old_root]` as a second child pattern of `new_root` when all of the following hold:

1. `old_root` is a known compound (it has at least one child pattern — i.e. it is not a flat atom or a flat unknown container, i.e. `!flat_root`)

   Actually: `old_root` may be an atom. For `aaa`, root = `a` is an atom. We do want this to fire for atom roots too. So condition 1 should simply be: `old_root` is set (not `None`).

2. `t1.width > old_root.width` — the new token is strictly wider than the previous root

3. `old_root` is a prefix of `t1` — i.e. calling `insert_next_match([old_root_atoms])` from the beginning of `t1`'s atom span returns `old_root`. This can be checked by asking: does `t1`'s first child at the appropriate depth equal `old_root`?

   For the simple case: is `old_root` a prefix of `t1`? This is the same as asking whether `t1.prefix_iter()` yields `old_root` at some point.

   However, for `aaa`: `t1 = aa`, `old_root = a`. Does `aa`'s prefix iter yield `a`? `prefix_iter` descends via the first child at each level: `aa → [a, a] → a` (first child). Yes — `prefix_iter` on `aa` yields `a`. So condition 3 can be checked as: `t1.prefix_iter(graph).any(|(_, prefix)| prefix == old_root)`.

4. The right complement (= `old_root`) is a valid token in the graph (it always is, since it is the current root).

   Actually the right complement is not always `old_root`. It is the token of width `new_root.width - t1.width = old_root.width`. For `aaa`: `3 - 2 = 1 = old_root.width = 1`. So right complement = `old_root`. This holds when `new_root.width - t1.width = old_root.width`, i.e. always (since `new_root = [old_root, t1]` so `new_root.width = old_root.width + t1.width`). The right complement is always `old_root`.

So conditions simplify to:

```
old_root is set
AND t1.width > old_root.width
AND t1 has old_root as a prefix  (t1.prefix_iter().any(|(_, p)| p == old_root))
```

When these hold, after `wrap_root(t1)`:

```rust
let new_root = self.root.unwrap(); // just set by wrap_root
// Add the symmetric decomposition [t1, old_root] as a second child pattern.
self.graph.insert_pattern_to(new_root, vec![t1, old_root]);
```

The graph API call needed is one that **adds a child pattern to an existing token** rather than creating a new token. This may be `insert_pattern_of` or similar — the exact API needs to be confirmed against `context-insert` / `context-trace`.

### Also fix anchor after the unknown segment

`RootManager::append_token` should set `self.anchor = self.root` after updating `self.root`. This ensures the anchor is correctly propagated to `BlockExpansionCtx` when a known segment follows an unknown segment.

---

## Execution plan

### Phase 0 — Confirm the trace with instrumentation

Add a temporary trace test for `aaa` that logs:

1. segment split (unknown / known)
2. `RootManager` state after the unknown segment (root, anchor, flat_root)
3. initial anchor passed into `BlockExpansionCtx`
4. each `ExpansionCtx::next()` yield
5. each `commit_state` branch taken (extend_root vs wrap_root, which sub-branch)
6. final graph patterns for `aa` and `aaa`

Run with `LOG_FILTER=trace` and check `target/test-logs/`. Confirm:

- anchor is `None` when `BlockExpansionCtx` starts (Problem 1)
- `commit_state` takes the `extend_root` path (Problem 2 — wrong branch)
- `[aa, a]` is never attempted

Only proceed after confirmation.

---

### Phase 1 — Regression matrix

Audit and extend the test suite for all repeated-pattern cases before touching logic.

| Input     | Expected root patterns                       | File                    |
|-----------|----------------------------------------------|-------------------------|
| `aa`      | `[[a, a]]`                                   | `ngrams_validation.rs`  |
| `aaa`     | `[[a, aa], [aa, a]]`                         | `linear.rs` ✅          |
| `aaaa`    | `[[aa, aa]]`                                 | add to `linear.rs`      |
| `abab`    | `[[ab, ab]]`                                 | `linear.rs` ✅          |
| `ababa`   | `[[ab, aba], [abab, a]]`                     | add to `linear.rs`      |
| `ababab`  | `[[ab, abab], [ababa, b]]`                   | `ngrams_validation.rs`  |

Add missing tests. All are expected to fail before the fix except those already passing.

---

### Phase 2 — Fix `append_token` / `append_pattern` anchor propagation

In `src/pipeline/root.rs`:

**`append_token`**: after updating `self.root`, add `self.anchor = self.root`.

**`append_pattern`**: after updating `self.root` (all branches), add `self.anchor = self.root`.

This is a small localised change. Verify it does not break any existing tests before proceeding to Phase 3.

Note: only set anchor when root is actually set (guard against the `new.len() == 0` case).

---

### Phase 3 — Fix `commit_state` to insert the symmetric pattern

In `src/pipeline/root.rs`, `RootManager::commit_state`, `BandState::Single` branch:

Locate the `wrap_root` sub-branch:

```rust
Some(_) => {
    debug!("commit_state Single: semantic root — wrap_root");
    self.wrap_root(token);
    self.flat_root = false;
},
```

After `self.wrap_root(token)`, add the symmetric-decomposition check:

```rust
// After wrap_root, new_root = [old_root, token].
// Check if token also has old_root as a prefix, which means
// [token, old_root] is a valid second decomposition of new_root.
// This handles the aaa-family: anchor=a, token=aa → also insert [aa, a].
if let Some(old_root) = old_root_before_wrap {
    if *token.width() > *old_root.width() {
        let token_has_old_root_as_prefix = token
            .prefix_iter(self.graph.clone())
            .any(|(_, prefix)| prefix == old_root);
        if token_has_old_root_as_prefix {
            let new_root = self.root.expect("wrap_root just set root");
            // Add [token, old_root] as a second child pattern of new_root.
            // API TBD: self.graph.add_child_pattern(new_root, vec![token, old_root]);
        }
    }
}
```

The `old_root_before_wrap` must be captured before calling `wrap_root`. Update the branch to:

```rust
Some(old_root_token) => {
    let old_root_before_wrap = old_root_token; // capture before mutation
    debug!("commit_state Single: semantic root — wrap_root");
    self.wrap_root(token);
    self.flat_root = false;
    // ... symmetric check here
},
```

#### Confirm the graph API for adding a child pattern to an existing token

Before implementing Phase 3, find the correct `context-insert` or `context-trace` API call for:

> add pattern `[token, old_root]` as a new child pattern of the already-existing `new_root` node

Search for `add_child_pattern`, `insert_pattern_to`, `insert_into_pattern`, or equivalent in `context-insert` / `context-trace`. If the API does not exist, a Phase 3a step to add it may be needed.

---

### Phase 4 — Verify and stabilise

1. Run the Phase 0 trace test. Confirm in the log:
   - anchor is now set after the unknown segment
   - `commit_state` now takes `wrap_root` (not `extend_root`) for `aa` against root `a`
   - `[aa, a]` is added as a second child pattern of `aaa`

2. Run `repetition_aaa_both_decompositions` — expected green.

3. Run the full Phase 1 regression matrix.

4. Run the full test suite:
   ```bash
   cargo test -p context-read
   cargo test -p context-insert
   cargo test -p context-trace
   ```

For any new failures, inspect whether:
- the `extend_root` path is now incorrectly blocked (Phase 2 anchor change may cause `commit_state` to take a different branch for previously-flat roots)
- the prefix check fires spuriously for cases it should not

---

### Phase 5 — Clean up

1. Remove or convert the Phase 0 trace test into a permanent regression test.
2. Update `agents/analysis/20260317_ANALYSIS_AAA_SEGMENT_BOUNDARY_AND_OUTER_LOOP.md` with confirmed root cause.
3. Mark this plan complete.
4. Write `agents/implemented/20260317_IMPLEMENTED_AAA_DECOMPOSITION_FIX.md`.
5. Update `agents/implemented/INDEX.md`.

---

## Risks and mitigations

### Risk 1 — `extend_root` vs `wrap_root` branch change

After Phase 2 sets the anchor, `commit_state` receives `Single(aa)` with `root = a` and `anchor = a`. If `flat_root = true` and `can_extend_with(aa) = true`, the code currently takes `extend_root`. After Phase 2, `anchor` is set but `flat_root` and `can_extend_with` are unchanged, so the branch is still `extend_root`. Phase 3 targets `wrap_root`, which does not fire here.

**This means Phase 2 alone does not route to `wrap_root`.** Phase 3 must also address the `extend_root` branch, or the `extend_root` path must be modified to also check for the symmetric pattern.

**Mitigation**: determine whether `extend_root` should be blocked when the incoming token is a semantic compound that already has parents (i.e. is not a "flat" extension). For `aaa`: `aa` has child pattern `[a, a]` and is about to acquire a parent. Using `extend_root` on `a` to produce `[a, aa]` is semantically correct for the sequential decomposition, but it prevents the symmetric `[aa, a]` from being added. The symmetric check should run in the `extend_root` branch too, or `extend_root` should be replaced by `wrap_root` for semantic compounds.

The rule should be: if the incoming token `t1` is a **semantic compound** (has child patterns of its own), always use `wrap_root` (create a new node) rather than `extend_root` (mutate in place). This ensures the new root node is a proper named entity that can carry multiple child patterns.

This may be the intended meaning of `flat_root`: `flat_root = true` means the root is a work-in-progress flat container, not yet a semantic compound. Once a semantic token (`aa`) arrives, the root must become a proper compound node via `insert_pattern`, not an in-place extension.

**Proposed rule change**: in `commit_state Single`, replace the `extend_root` branch condition:

```rust
Some(_) if self.flat_root && self.can_extend_with(token) => {
    self.extend_root(token);
}
```

with a narrower condition that also checks that the incoming token is itself a flat atom (not a compound):

```rust
Some(_) if self.flat_root && self.can_extend_with(token) && *token.width() == 1 => {
    self.extend_root(token);
}
```

When the incoming token is a compound (width > 1), fall through to `wrap_root` even if `flat_root = true`. This ensures a proper new node is always created when a semantic compound is first appended, and the symmetric-pattern check in `wrap_root` can then fire.

### Risk 2 — Spurious symmetric patterns for non-prefix cases

The prefix check `token.prefix_iter().any(|(_, p)| p == old_root)` may be expensive or may yield false positives for deep compound trees. Verify by running the full test suite.

### Risk 3 — Graph API for adding a child pattern

If `add_child_pattern(new_root, pattern)` does not exist, a new API must be added in `context-insert` or `context-trace`. This is a Phase 3a risk. Check before implementing.

### Risk 4 — `aaaa` interaction

For `aaaa`, after reading `aaa` (which now has two child patterns), reading the fourth `a` must still produce `aaaa = [[aa, aa]]`. Verify that the symmetric check does not fire spuriously for `aaaa`: old_root = `aaa` (width 3), token = `aa` (width 2). `token.width (2) > old_root.width (3)` → false → check does not fire. Correct.

---

## Execution order summary

```
Phase 0 — trace aaa, confirm anchor=None and extend_root branch
Phase 1 — regression matrix
Phase 2 — fix anchor propagation in append_token / append_pattern
Phase 3 — fix commit_state:
  3a — confirm/add graph API for adding child pattern to existing token
  3b — narrow extend_root to atom-only incoming tokens
  3c — add symmetric-pattern check in wrap_root branch
Phase 4 — verify: trace, aaa test, matrix, full suite
Phase 5 — clean up, implemented summary
```

---

## Key files

| File | Role |
|------|------|
| `src/pipeline/root.rs` | primary change: `append_token`, `append_pattern`, `commit_state` |
| `src/expansion/mod.rs` | read-only during this fix (no changes to `ExpansionCtx`) |
| `src/expansion/block.rs` | read-only during this fix |
| `src/tests/linear.rs` | `repetition_aaa_both_decompositions` + new matrix tests |
| `src/tests/ngrams_validation.rs` | `validate_three_repeated`, `validate_triple_repeat` |