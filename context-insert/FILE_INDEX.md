# context-insert File Index

## Legacy / positioned split (2026-11-19)

All modules (`insert/`, `interval/`, `join/`, `overlap/`, `split/`,
`visualization.rs`, `tests/`) live in `src/legacy/`, gated by the default-on
`legacy` feature, and are imported as `context_insert::legacy::…`. The module
paths below refer to `src/legacy/…`.

**Rule:** no new code in `legacy`; only fixes needed to keep the legacy tests green. New (positioned-model) code goes into the root modules and must not depend on `legacy` (checked by `cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api`). Plan: `context-read/agents/plans/20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`.

Python prototypes of positioned insertion (split graph and join, merge, invariant checks) with an execution manual: `agents/prototypes/README.md`.

**Generated:** 2025-11-23  
**Updated:** 2025-11-23 (Post-Workspace Reorganization)  
**Git Commit:** f23260f  
**Commit Date:** 2025-11-23  
**Commit Message:** refactor: extract standalone tools and reorganize deps  
**Status:** ✅ Excellent Organization (No Changes Needed)

**Total:** 55 files, 5,609 lines  
**Largest File:** 385 lines  
**Files >500:** 0 ✅ (Already excellent!)

## Summary Statistics

| Category | Files | Lines | % of Total |
|----------|-------|-------|------------|
| Join | 13 | 1,098 | 19.6% |
| Split | 12 | 1,587 | 28.3% |
| Interval | 13 | 1,621 | 28.9% |
| Insert | 3 | 286 | 5.1% |
| Tests | 3 | 806 | 14.4% |
| Other | 11 | 211 | 3.8% |

## Files by Size Category

### 🔴 Very Large (>500 lines) - None! ✅

### 🟡 Large (300-500 lines) - Review for Splitting
| Lines | File | Purpose |
|-------|------|---------|
| 385 | join/context/node/context.rs | Join node context |
| 346 | tests/interval.rs | Interval tests |
| 312 | tests/insert.rs | Insertion tests |

### 🟢 Medium (200-299 lines) - Monitor
| Lines | File |
|-------|------|
| 280 | split/vertex/mod.rs |
| 223 | split/mod.rs |

### ✅ Small (<200 lines) - Good Size
48 files under 200 lines (very well-structured!)

## Module Organization

### join/ (13 files, 1,098 lines)
**Purpose:** Join phase - merging split results back together

```
join/
├── context/ (8 files, 850 lines)
│   ├── node/ (4 files, 585 lines)
│   │   ├── context.rs (385) - 🟡 REVIEW: Node context logic
│   │   ├── merge.rs (171)
│   │   ├── kind.rs (26)
│   │   └── mod.rs (3)
│   ├── pattern/ (2 files, 124 lines)
│   │   ├── borders.rs (83)
│   │   └── mod.rs (41)
│   ├── frontier.rs (91)
│   └── mod.rs (3)
├── partition/ (3 files, 184 lines)
│   ├── info/ (2 files, 171 lines)
│   │   ├── pattern_info.rs (120)
│   │   ├── inner_range.rs (53)
│   │   └── mod.rs (51)
│   └── mod.rs (80)
├── joined/ (3 files, 216 lines)
│   ├── patterns.rs (138)
│   ├── partition.rs (77)
│   └── mod.rs (1)
└── mod.rs (3)
```

**Structure:** Well-organized, only one file approaching 400 lines

**Issues:**
- `context/node/context.rs` (385) - Could extract some logic but manageable

### split/ (12 files, 1,587 lines)
**Purpose:** Split phase - breaking intervals into smaller pieces

```
split/
├── vertex/ (4 files, 486 lines)
│   ├── mod.rs (280) - Monitor growth
│   ├── pattern.rs (83)
│   ├── output.rs (82)
│   ├── node.rs (46)
│   └── position.rs (31)
├── cache/ (4 files, 413 lines)
│   ├── vertex.rs (185)
│   ├── position.rs (132)
│   ├── mod.rs (67)
│   └── leaves.rs (29)
├── trace/ (2 files, 227 lines)
│   ├── states/ (2 files, 172 lines)
│   │   ├── context.rs (98)
│   │   └── mod.rs (74)
│   └── mod.rs (55)
├── mod.rs (223)
├── context.rs (91)
├── pattern.rs (89)
└── run.rs (71)
```

**Structure:** Excellent - hierarchical and well-sized files

**Issues:** None significant

### interval/ (13 files, 1,621 lines)
**Purpose:** Interval operations and partitioning

```
interval/
├── partition/ (10 files, 1,291 lines)
│   ├── info/ (8 files, 1,078 lines)
│   │   ├── border/ (4 files, 427 lines)
│   │   │   ├── perfect.rs (184)
│   │   │   ├── visit.rs (133)
│   │   │   ├── trace.rs (35)
│   │   │   └── mod.rs (75)
│   │   ├── range/ (4 files, 441 lines)
│   │   │   ├── role.rs (140)
│   │   │   ├── splits.rs (126)
│   │   │   ├── children.rs (95)
│   │   │   ├── mode.rs (84)
│   │   │   └── mod.rs (80)
│   │   ├── mod.rs (106)
│   │   └── borders.rs (43)
│   ├── mod.rs (133)
│   └── delta.rs (54)
├── mod.rs (53)
└── init.rs (43)
```

**Structure:** Deep hierarchy but well-organized, all files manageable

### insert/ (3 files, 286 lines)
**Purpose:** Public insertion API

```
insert/
├── direction.rs (130)
├── context.rs (110)
├── result.rs (103)
└── mod.rs (43)
```

**Structure:** Excellent, well-sized focused files

### tests/ (3 files, 806 lines)
**Purpose:** Unit and integration tests

```
tests/
├── interval.rs (346) - 🟡 REVIEW: Interval tests
├── insert.rs (312) - 🟡 REVIEW: Insert tests
├── mod.rs (148)
```

**Issues:**
- Test files are large but test-heavy crates often have this
- Could group tests by feature

### lib.rs (71 lines)
Clean, focused exports

## Strengths

✅ **Excellent file size discipline** - No files over 400 lines!
✅ **Good hierarchy** - Clear separation of concerns
✅ **Focused modules** - Each module has a clear purpose
✅ **Small coordination files** - Most mod.rs files are tiny

## Recommendations

### Optional Improvements (300-400 lines)
1. **join/context/node/context.rs** (385) → Consider extracting:
   - Context building logic
   - Context validation/checks
   - Keep if cohesive enough

2. **tests/interval.rs** (346) → Group tests by:
   - Partition types
   - Border cases
   - Range operations

3. **tests/insert.rs** (312) → Group tests by:
   - Insertion direction
   - Pattern types
   - Error cases

### Module Organization
Current structure is already quite good. Optional tweaks:

#### join/context/node/
Could extract if context.rs grows:
```
join/context/node/
├── context/
│   ├── core.rs - Context struct
│   ├── builder.rs - Context building
│   └── mod.rs
├── merge.rs
├── kind.rs
└── mod.rs
```

#### split/vertex/
Currently well-organized, monitor mod.rs for growth

## Overall Assessment

**🟢 Best organized crate in the workspace!**

- No files over 500 lines
- Clear module boundaries
- Good use of subdirectories
- Small, focused files
- Logical hierarchy

**Recommendation:** Use this crate as a model for organizing other crates.
