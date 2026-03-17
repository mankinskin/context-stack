---
tags: `#analysis` `#context-read` `#aaa` `#segmentation` `#outer-loop` `#anchor` `#root-manager` `#expansion`
summary: Skill-informed research note for the `aaa` failure. Reframes the issue using the documented outer-loop semantics of `insert_next_match` and narrows the likely fault to segment-boundary context preservation, first-step known-block behavior, or root/anchor commit semantics rather than segmentation itself.
status: 📋 active
date: 2026-03-17
related_analysis: 20260315_ANALYSIS_AAA_DECOMPOSITION_NEXT_STEP.md
related_interview: ../interviews/20260317_INTERVIEW_AAA_DECOMPOSITION_NEXT_STEP.md
related_skill: ../../../../docs/skills/03_context_completion.md
---

# Analysis: `aaa` Segment Boundary and Outer-Loop Semantics

## Purpose

This note revisits the `aaa` failure after incorporating the semantics documented in:

- `docs/skills/03_context_completion.md`

The goal is to refine the interpretation of the bug before implementation planning.

The main conclusion is:

> the `aaa` failure should now be treated primarily as an outer-loop / segment-boundary / left-context preservation problem, not as evidence that segmentation itself is fundamentally wrong.

---

## Current failure

### Expected

For input `aaa`, `context-read` should produce:

- `aa => [[a, a]]`
- `aaa => [[a, aa], [aa, a]]`

### Actual

The current result only contains:

- `aaa => [[a, aa]]`

The alternate decomposition:

- `[aa, a]`

is missing.

This remains the smallest deterministic semantic failure in the read pipeline.

---

## What the skill file clarifies

The skill document gives a clearer contract for the underlying insertion semantics.

## `insert_next_match` is a primitive, not the full algorithm

The key documented rule is:

- `insert_next_match` handles exactly **one segment**
- it does **not** consume the whole input by itself
- the caller must maintain the **outer loop**
- the caller advances the cursor and invokes it again on the remainder

This is explicitly described as the intended model for context completion.

### Why that matters for `aaa`

This means the semantics of `aaa` cannot be judged by looking only at one isolated local match.

Instead, the meaning of `aaa` must come from:

1. what happens at the first cursor position
2. what running root/anchor state is preserved
3. how the next segment is processed
4. what semantic structure is retained across those steps

So the failure is most likely not:

> `insert_next_match` is wrong in isolation

but instead:

> the read pipeline is not preserving or reusing the left context from the previous outer-loop step strongly enough when the next known segment begins

---

## Restated semantic model for reading

Based on the skill file, the read pipeline should be understood as:

1. consume one segment
2. keep the accumulated semantic result
3. advance
4. continue with the remainder
5. construct the final root from the accumulated sequence of successful local completions

That means segmentation is an optimization or orchestration technique inside the outer loop.

It should not change the semantic set of valid decompositions that the final token can retain.

### Consequence

A segment boundary may be operationally convenient, but it must not erase valid left-context information.

That is the critical point for `aaa`.

---

## Revised interpretation of the `aaa` read

For input `aaa` on an initially empty graph, the likely intended story is:

1. The first `a` is unknown.
2. It is inserted as an atom.
3. The running root becomes `a`.
4. The running anchor should also be `a`.
5. The remaining `aa` is processed as a known segment.
6. The known segment must be able to see the already-read `a` as valid left context.
7. The read should create `aa`.
8. The final width-3 token should retain both valid adjacent decompositions:
   - `[a, aa]`
   - `[aa, a]`

This does **not** require abandoning segmentation.

It requires segmentation to preserve the semantics of the surrounding outer loop.

---

## Why both decompositions are expected

The expectation is not arbitrary.

Once `aa` exists, the width-3 structure `aaa` admits two valid binary adjacent decompositions:

- left atom + right known compound:
  - `[a, aa]`
- left known compound + right atom:
  - `[aa, a]`

These are the two natural width-preserving binary factorizations of the same atom span.

### Important point

The read pipeline is not supposed to preserve only one “first discovered” decomposition if a second valid one is semantically available during the same overall read.

The `aaa` case is therefore the smallest example of repeated-pattern symmetry.

That makes it important beyond its size.

---

## What this says about segmentation

Before reading the skill file, a tempting interpretation was:

> if segmentation causes `[aa, a]` to disappear, then segmentation itself may be incorrect or unnecessary

After reading the skill file, that interpretation is weaker.

A better framing is:

> segmentation is still a valid shortcut, but its boundary semantics are currently too weak or too lossy for this case

So the likely issue is not “remove segmentation.”

It is more likely one of:

1. the segment boundary does not preserve the right root/anchor state
2. the known-block startup logic does not exploit the preserved state
3. the commit path materializes only one construction and fails to retain the symmetric sibling decomposition

---

## Strong clue from the current code

The current code strongly suggests a likely failure mode.

## Current overlap logic suppresses atom anchors

`ExpansionCtx::next()` currently distinguishes atom anchors from non-atom anchors and skips overlap probing for atom anchors.

Conceptually, that logic says something like:

- atom anchors have no true postfixes
- therefore overlap detection is skipped

That may be locally reasonable for one narrow postfix-search mechanism.

But for `aaa`, it is likely too strong.

### Why

After the first unknown segment, the left context is exactly the atom `a`.

If the algorithm says:

- “the anchor is only an atom, so there is no overlap-relevant left context”

then it will likely lose the very context needed to recognize or synthesize the symmetric repeated decomposition.

### Key distinction

These two statements are not equivalent:

1. an atom has no nontrivial postfix tree
2. an atom is irrelevant as left context for the next completion step

The first may be true.
The second is likely false for `aaa`.

That is an important narrowing of the bug.

---

## Most likely fault locations now

After incorporating the skill semantics, the most plausible fault locations are:

### 1. First-step known-block behavior in `ExpansionCtx`

This is currently the strongest suspicion.

The first step of the known segment may need to use the existing left context even when the carried anchor is only an atom.

In other words, the startup rule for the known block may be too restrictive.

### 2. Segment-boundary invariants

The handoff from:

- unknown segment append
to
- known segment expansion

may fail to preserve the semantic meaning of the left context in a way that the expansion layer can use.

This does not necessarily mean the stored value is wrong.
It may mean the stored value is semantically underinterpreted.

### 3. `RootManager::commit_state`

Even if the right local construction is detected, the commit path may only materialize one decomposition and fail to retain the sibling decomposition of the same width-3 token.

This remains plausible, but is currently slightly less likely than the first-step boundary problem.

### 4. Immediate-commit orchestration

It is still possible that immediate commit is too eager.

But the skill file weakens this as the first hypothesis, because the documented model explicitly expects the outer loop to operate incrementally and successfully over local single-step results.

So immediate commit is still acceptable unless it can be shown to erase a valid construction that should survive a normal segment transition.

---

## Revised ranking of hypotheses

### Highest confidence

1. the left context from the unknown first `a` is not being used correctly at the start of the known `aa` block
2. atom-anchor suppression is too aggressive for repeated minimal cases

### Medium confidence

3. `RootManager::commit_state` only preserves the primary sequential construction and not the symmetric sibling decomposition

### Lower confidence

4. segmentation as a whole is conceptually wrong
5. the entire immediate-commit model must be replaced with broader buffering

At this stage, the evidence favors a local semantic fix over a large architectural rewrite.

---

## Updated working hypothesis

A more precise hypothesis is now:

> the `aaa` failure occurs because the read outer loop correctly carries the first `a` across the segment boundary operationally, but the known-segment startup logic does not treat that carried atom as usable left context for repeated-pattern completion, so `[aa, a]` is never materialized.

This hypothesis aligns with:

- the documented outer-loop semantics
- the observed current output
- the current code path that suppresses overlap probing for atom anchors

---

## What should be investigated next

Before implementation, the next debugging pass should explicitly verify the boundary semantics around the first known segment.

## Required trace for `aaa`

A useful trace should record:

1. exact segment split for `aaa`
2. root and anchor after the unknown segment
3. initial `BlockExpansionCtx` anchor
4. each `ExpansionCtx::next()` yield
5. whether overlap probing is attempted on the first known step
6. whether atom-anchor suppression is the reason it is skipped
7. each `RootManager::commit_state()` branch
8. final graph patterns for `aa` and `aaa`

This will show whether the missing decomposition is lost in:

- state generation
- overlap probing
- or commit semantics

---

## Implications for implementation planning

The implementation plan should not begin with “remove segmentation” or “replace the entire read model.”

Instead, it should begin with:

1. instrumentation of the `aaa` path
2. confirmation of the segment boundary and carried anchor semantics
3. confirmation of whether the first known step is allowed to exploit an atom left-context anchor
4. a minimal fix that restores the repeated-pattern symmetry in a way that can generalize

### Planning preference

The plan should prioritize:

- a small, semantically justified fix
- preserving the current outer-loop structure
- preserving segmentation as an optimization
- avoiding a special-case hack that only passes `aaa` without helping the broader repeated-pattern family

---

## Relation to larger failures

The `aaa` case still appears to be the best minimal probe for a broader class of issues.

If the problem is indeed:

- segment-boundary context loss
- or atom-anchor underuse at known-block startup

then that likely affects not only:

- `aaa`

but also larger repeated or overlap-sensitive inputs such as:

- `aaaa`
- `ababab`
- `abcabcabc`
- `xyzxyzxyz`

So a correct `aaa` fix should ideally generalize rather than remain case-specific.

---

## Bottom line

After incorporating the documented semantics from the skill file, the best current interpretation is:

- segmentation is probably still valid
- the read algorithm is explicitly outer-loop driven
- `insert_next_match` is not meant to solve the whole input in one step
- therefore the `aaa` failure is best understood as a failure to preserve or exploit left context across the segment boundary
- the strongest immediate suspicion is the startup behavior of the known block when the carried anchor is an atom

## Current recommendation

Before implementation:

1. keep segmentation in place
2. verify that `root = a` and `anchor = a` after the first unknown segment
3. verify whether the first known-step overlap/adjacency logic is skipped because the anchor is an atom
4. determine whether fixing that startup rule is sufficient to recover `[aa, a]`
5. only escalate to broader orchestration changes if that evidence fails

This is now the most grounded next step for the `aaa` investigation.