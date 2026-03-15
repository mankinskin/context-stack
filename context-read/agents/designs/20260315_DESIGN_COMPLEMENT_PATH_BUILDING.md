---
tags: `#design` `#context-read` `#complement` `#trace-cache` `#algorithm`
summary: Design session for building the complement token from the downward path into the anchor vertex. Required before the complement trace-cache stub can be implemented.
status: 📋 design-session-pending
date: 2026-03-15
plan: 20260315_PLAN_CONTEXT_READ_RESTRUCTURE.md
related: 20260315_PLAN_EXPANSION_LOOP_REDESIGN.md
blocking: 10 failing context-read unit tests (all overlap-collapse paths)
---

# Design: Complement Path Building

**Date:** 2026-03-15
**Scope:** `context-read/src/complement.rs` — `build_trace_cache_stub` → full implementation
**Blocked by:** design session (this document)
**Unblocks:** all 10 currently failing `context-read` unit tests

---

## Problem Statement

When the expansion loop detects an overlap between the anchor token and an
incoming token `t1`, it builds a `BandState::WithOverlap` and calls
`collapse()`. Collapse calls `ComplementBuilder::build`, which needs to
produce the *complement token* — the prefix of the anchor that lies to the
left of the overlap region.

To create that token, `build` calls `graph.insert_init((), init_interval)`,
which internally uses `init_interval.cache` (a `TraceCache`) to walk the
downward path from the anchor root to the intersection point and extract the
prefix sub-token.

The current stub returns `TraceCache::new(root)` — an empty cache containing
only the root entry. `insert_init` then fails with `MissingCacheEntry(...)` 
because no downward path entries exist in the cache for any position below
the root.

---

## What `TraceCache` Needs to Contain

A `TraceCache` for a token `root` and end-bound `intersection_start` must
contain one entry per child-location on the path from `root` down to the
atom at position `intersection_start - 1`.

Concretely, for anchor = `abcde` (width 5) and `intersection_start = 3`
(overlap starts at position 3, so complement covers positions 0..3 = `abc`):

```
root = abcde
  path down to position 2 (0-indexed, last position of the complement):
    abcde → child pattern [ab, cde] → take left child ab  (covers 0..2)
         or
    abcde → child pattern [abc, de] → take left child abc (covers 0..3, exact match)
```

The `TraceCache` must record each step of this descent so that `insert_init`
can reconstruct the sub-token without re-traversing the graph.

---

## Candidate Approaches

### Approach 1 — Walk the downward path manually

Starting from `root`, at each level find the child pattern whose left child
covers exactly `intersection_start` atoms (or the nearest split point), record
the `ChildLocation` into the cache, and recurse into the left child until we
reach an atom or an exact-width match.

**Pros:** Direct; no new infrastructure needed.  
**Cons:** Requires understanding the exact `TraceCache` entry format
(`DownKey`, `DownPosition`, `DirectedPositions`) and the invariants
`insert_init` expects. Risk of building a cache that satisfies the shape but
not the invariants.

### Approach 2 — Use the existing search/checkpoint API

`context-search` can produce a `TraceCache` as a side-effect of a bounded
search from `root` to position `intersection_start`. The search already
knows how to walk downward paths and populate caches.

**Pros:** Reuses tested infrastructure; invariants are guaranteed by the
search machinery.  
**Cons:** Introduces a dependency on `context-search` inside
`context-read/src/complement.rs` (currently only `context-insert` is used
there). May be heavier than needed for a simple prefix extraction.

### Approach 3 — Build the complement token directly without `insert_init`

Instead of going through `insert_init`, walk the anchor's child patterns to
find the sub-pattern that covers exactly `[0, intersection_start)`, then call
`graph.insert_pattern(sub_pattern)` directly (idempotent, returns the
existing token if it already exists).

**Pros:** Avoids `TraceCache` entirely for the common case; simpler code path.  
**Cons:** Only works when the anchor has a child pattern that splits cleanly
at `intersection_start`. If no such split exists in the stored patterns, we
must recurse further or fall back to `insert_init`. Does not handle the
general case.

---

## Recommended Direction

**Approach 3 as the primary path, Approach 1 as fallback.**

1. Walk the anchor's child patterns. If any pattern has a prefix sub-list
   whose total width equals `intersection_start`, call `insert_pattern` on
   that prefix and return.
2. If no clean split exists at `intersection_start` in any stored pattern,
   fall back to the manual downward walk (Approach 1) to build a minimal
   `TraceCache` and call `insert_init`.

This keeps the common case simple (most anchors will have a clean split
because the graph is incrementally built from clean decompositions) while
remaining correct for the general case.

---

## Data to Gather Before Implementation

Before the design session produces a final implementation, the following
questions need answers from code inspection or experiment:

1. **`TraceCache` entry format** — What is the exact structure of a
   `DownKey` / `DownPosition` entry? What fields are required for
   `insert_init` to consider the cache valid for a given `(root, end_bound)`
   pair?

2. **`insert_init` contract** — Does `insert_init` require a *complete* path
   from root to the target position, or only the entries it actually accesses
   during the insertion? Knowing this determines how many levels the manual
   walk must populate.

3. **Clean-split frequency** — In practice, how often does the anchor have a
   stored child pattern that splits cleanly at `intersection_start`? The
   answer affects whether Approach 3 covers enough cases to be worthwhile as
   the primary path.

4. **Existing downward-walk utilities** — Is there already a function in
   `context-trace` or `context-search` that walks a token downward to a given
   atom position and returns the path? If so, it should be reused rather than
   reimplemented.

---

## Known Constraints

- `complement.rs` currently depends on `context-insert` (`insert_init`,
  `InitInterval`, `TraceCache`). Any solution must remain compatible with
  this dependency or explicitly justify adding `context-search`.
- The complement token must be the same vertex that would be produced by
  reading the prefix string independently — i.e., `insert_pattern` and
  `insert_init` must produce the same result for a given prefix. This is an
  invariant of the graph and must not be violated.
- The complement is consumed immediately by `build_overlap_state` in
  `expansion/mod.rs` — it does not need to be stored or cached beyond the
  single `collapse()` call.

---

## Files to Change

| File | Change |
|------|--------|
| `src/complement.rs` | Replace `build_trace_cache_stub` with the real implementation once design is settled |
| `src/expansion/chain/mod.rs` | `build_prefix_complement` and `build_postfix_complement` may need to call the new helper |
| `context-trace/src/graph/vertex/traversal.rs` | Possible home for a new downward-walk utility (if one is needed and does not already exist) |

---

## Session Notes

*(To be filled in during the design session.)*

- Date of session:
- Participants:
- Chosen approach:
- Key invariants discovered:
- Implementation sketch: