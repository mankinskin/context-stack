---
tags: `#plan` `#context-insert` `#algorithm` `#testing` `#api`
summary: > **Status:** Design Phase
status: 📋
---

# Selective Partition Merge - Feature Documentation

> **Status:** Design Phase  
> **Created:** 2026-01-27  
> **Related Test:** `EnvInsertInfix1` in `crates/context-stack/context-insert/src/tests/cases/insert/infix.rs`

## Overview

Transform the simple complete merging process into a constructive merge process with **selective pattern creation**. Only explicitly required partitions should result in token creation.

## Core Principle

> Any repetition of token sequences needs to be replaced with its own token.

The insert interface guarantees a token response for a token sequence request. The largest known sequence is found left-to-right and its vertex is created if it doesn't exist.

## 2026-05-27 spec alignment

This ticket now follows the reviewed merge requirements and examples in:

- [context-read pipeline](../../../.spec/specs/e0913182-7a5e-4c8f-a750-799afd58baae/body.md)
- [graph induction](../../../.spec/specs/16c3ad95-451d-4c09-a118-ca90bcefed9a/body.md)

Corrections to keep in mind while using the older notes below:

- Search hands merge the tightest existing root containing the requested range,
    so every valid merge updates the root decomposition set.
- Wrappers are not automatic for every dirty cut. Use a wrapper only when it is
    beneficial rather than redundant.
- Inner partitions materialize inside the requested token or the chosen wrapper
    token. They are not independent root replacements.
- The `aby` / `abyz` discussion below is a local one-sided-dirty example, not a
    universal rule that every dirty cut must end in wrapper replacement.

## Current State

### Problem
The current merge algorithm creates tokens for ALL possible partition combinations via `range_sub_merges()`. This produces unnecessary tokens that are not explicitly required.

**Example:** `EnvInsertInfix1`
- Root token: `xxabyzw` with pattern `[x, x, a, b, yz, w]`
- Insert request: `[a, b, y]` → create token `aby`
- Target range: `1..=3` (offsets 2-5)
- Operating range: `1..=4` (offsets 2-6, includes wrapper boundary)

**Current behavior creates:**
- `ab` (partition 1..=2) ✓ Required - target overlap
- `aby` (partition 1..=3) ✓ Required - target token
- `abyz` (partition 1..=4) ✓ Required - wrapper token
- `by` (partition 2..=3) ✗ NOT required
- `byz` (partition 2..=4) ✗ NOT required

### Root Cause
`merge_sub_partitions()` iterates all partition ranges without filtering for requirement.

## References

### Key Files
| File | Purpose |
|------|---------|
| `split/cache/vertex.rs` | Offset augmentation, `root_augmentation()` |
| `join/context/node/merge/context.rs` | Main merge loop, `merge_sub_partitions()` |
| `join/context/node/merge/range_map.rs` | `range_sub_merges()` generates all decompositions |
| `interval/partition/info/border/info.rs` | Border info and offset calculations |

### Key Functions
- `root_augmentation()` - Computes target_positions, wrapper_offsets, final_positions
- `add_wrapper_offsets_infix()` - Adds wrapper boundary offsets
- `merge_sub_partitions()` - Creates tokens for partition ranges
- `range_sub_merges()` - Generates all 2-way split patterns

## Facts & Definitions

### Partition Types

1. **Target Partition**
   - The partition range for the query token being inserted
   - Example: `1..=3` for `aby` in the test case

2. **Wrapper Partition Candidate**
     - Arises when the target has an **unperfect split** with the root pattern
     - Is only required when it provides a beneficial reusable replacement that a
         direct root update would not already express cleanly
     - Example: `1..=4` for `abyz` remains one possible local cover for `[a, b, y]`
         in `[a, b, yz]`, but it is not the universal root outcome for dirty cuts
     - Any chosen wrapper still extends to the nearest aligned pattern boundary

3. **Inner Partition**
   - The other side of an unperfect split: the sequence around the child token where the unaligned boundary occurs
    - Must be created as its own token when omitting it would repeat a token sequence or lose required child-token exposure
   - Example: Target `aby` in wrapper `abyz` with pattern `[a, b, yz]`
     - `aby` ends at offset 5, but `yz` spans offsets 4-6 (unperfect split at 5)
     - The inner partition is `ab` - the part around the child `yz` where the split occurs
     - Without inner: `abyz` = `[a, b, yz]` and `aby` = `[a, b, y]` → `[a, b]` sequence is repeated!
     - With inner `ab`: `abyz` = `[ab, yz]` or `[aby, z]`, `aby` = `[ab, y]` → no repetition
    - Inner partitions materialize inside the requested token or chosen wrapper token; they are not standalone root replacements

4. **Overlap Partition**
   - Created when two explicitly required partitions share boundaries
   - Example: `ab` is the overlap between `aby` (target) and `abyz` (wrapper)

### Offset Augmentation

After search, we get an offset range for the target token. The augmentation phase:
1. Adds wrapper partition offsets
2. Adds inner partition offsets (for non-perfect borders)
3. Computes final_positions combining all required offsets

### Required Partition Calculation

A partition is **required** if and only if:
1. It is a **target** partition (the token being inserted), OR
2. It is a **wrapper** partition (enables update of parent pattern), OR
3. It is an **inner** partition (the sequence around an unperfect split that would otherwise repeat), OR
4. It is an **overlap** between two required partitions

Partitions that are merely "covered" by required partitions (like `by` covered by `aby`) are NOT required.

## Requirements

### Functional Requirements

1. **R1:** Only create tokens for required partitions
2. **R2:** Target partition must always be created
3. **R3:** Wrapper and inner partitions created for each unperfect split in a child pattern:
    - **Wrapper:** only when it is beneficial and extends target to the nearest aligned pattern boundary
    - **Inner:** whenever the sequence around the dirty child boundary must be materialized to prevent repetition or preserve child-token exposure
4. **R4:** Overlaps created only between explicitly required partitions

### Non-Functional Requirements

1. **R5:** Partition requirements computed during augmentation phase
2. **R6:** Merge phase uses pre-computed requirement set
3. **R7:** No change to external insert API

## Algorithm Flow

```
Search Phase
    ↓
[Offset Range for Target]
    ↓
Augmentation Phase
    ├── Add wrapper offsets
    ├── Add inner offsets (non-perfect borders)
    ├── Compute final_positions
    └── Label required partitions (top-down from wrappers)
    ↓
[Required Partition Set]
    ↓
Merge Phase (Selective)
    ├── Iterate partition ranges
    ├── Skip if not in required set
    └── Create tokens only for required partitions
    ↓
[Result: Only Required Tokens Created]
```

## Test Case Validation

For `EnvInsertInfix1`:

| Partition | Range | Token | Required? | Reason |
|-----------|-------|-------|-----------|--------|
| Target | 1..=3 | `aby` | ✓ | Insert request |
| Wrapper | 1..=4 | `abyz` | ✓ | Parent pattern update |
| Overlap | 1..=2 | `ab` | ✓ | Overlap of target & wrapper at left |
| - | 2..=3 | `by` | ✗ | Not required - just covered |
| - | 2..=4 | `byz` | ✗ | Not required - just covered |

Expected result: `aby` has exactly 1 child pattern `[ab, y]`, not 2.
