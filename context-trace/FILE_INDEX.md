# context-trace File Index

## Legacy / positioned split (2026-11-19)

The row model (pattern-row `Hypergraph`) lives in `src/legacy/`, gated by the
default-on `legacy` feature, and is imported as `context_trace::legacy::…`.
There is no root `pub use legacy::*`; `legacy` mirrors the root items
(`pub use crate::*`), so legacy code imports everything from one namespace.

- **Root (model-agnostic):** `direction/` (`Direction`, `Left`, `Right`),
  `logging/`, `graph/kind.rs` (`GraphKind`), `graph/vertex/{atom, key, token,
  vertex_index, wide, has_vertex_index, has_vertex_key}`,
  `graph/vertex/name_resolver.rs` (thread-local `TokenNameResolver` hook used by
  `Token`/`VertexIndex` display), `position.rs` (`AtomPosition`),
  `tests/test_case/`. Impls of root items for row types carry
  `#[cfg(feature = "legacy")]`.
- **`positioned/` (root, no legacy dependency):** `interchange.rs` (v3
  format + structural validator), `composition.rs` (`Occurrence`,
  `Composition`), `location.rs` (`OccurrenceLocation`,
  `IntervalView`), `graph.rs` (`PositionedGraph`, `PositionedGraphRef`,
  validation, debug-only labels and `PositionedLabels` name resolver),
  `convert.rs` (`test-api`: `from_interchange`, `from_interchange_named` name
  map, `to_interchange`), `diff.rs` (`test-api`), content reading `read.rs`
  (debug and `test-api`: `PositionedGraph::read_interval`), `fixtures.rs`
  (`test-api` v3 fixture loader, `FixtureGraph`), `tests/` (`fixtures.rs` round
  trip, name map, labels; `model.rs` model and validation rules;
  `navigation.rs` `read_interval` and navigation on v3 fixtures;
  `guard.rs` no content-keyed field; `diff.rs`, `interchange.rs`).
- **`legacy/`:** `direction/pattern.rs` (`PatternDirection`), `graph/`
  (`Hypergraph`, `HypergraphRef`, getters, insert, snapshot, validation,
  search_path, visualization, test_graph, child_strings), `graph/vertex/{data,
  entry, has_vertex_data, location, parent, pattern, traversal}`, `path/`,
  `trace/`, `logging/path_format.rs`, `tests/` (env, macros, row tests).
  The paths in the tables below refer to `src/legacy/…` for these modules.

**Rule:** no new code in `legacy`; only fixes needed to keep the legacy tests green. New (positioned-model) code goes into the root modules and must not depend on `legacy` (checked by `cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api`). Plan: `context-read/agents/plans/20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`.

**Generated:** 2025-11-23  
**Updated:** 2025-11-23 (Post-Phase 2 Complete)  
**Git Commit:** f23260f  
**Commit Date:** 2025-11-23  
**Commit Message:** refactor: extract standalone tools and reorganize deps  
**Status:** ✅ Phase 2 Complete - All Large Files Split!

**Total:** 153 files, 19,173 lines  
**Largest File:** 408 lines (down from 728!)  
**Files >500:** 0 (was 6) ✅

## Summary Statistics

| Category | Files | Lines | % of Total |
|----------|-------|-------|------------|
| Graph | 21 | 3,850 | 20.8% |
| Path | 35 | 4,736 | 25.6% |
| Trace | 13 | 1,922 | 10.4% |
| Logging | 12 | 2,785 | 15.1% |
| Tests | 16 | 2,893 | 15.6% |
| Direction | 4 | 408 | 2.2% |
| Other | 24 | 1,894 | 10.3% |

## Files by Size Category

### ✅ All Large Files Split! (Phase 2 Complete)

**Previously >500 lines (now split):**
- ✅ logging/tracing_utils/config.rs (728 → 305 lines largest)
- ✅ graph/vertex/data.rs (699 → 406 lines largest)
- ✅ tests/macros.rs (618 → 292 lines largest)
- ✅ logging/tracing_utils/formatter.rs (591 → 408 lines largest)
- ✅ path/structs/rooted/index_range.rs (510 → 184 lines largest)
- ✅ graph/insert.rs (502 → 118 lines largest)

### 🟢 Largest Files Now (400-500 lines) - Acceptable
| Lines | File | Purpose | Status |
|-------|------|---------|--------|
| 408 | logging/tracing_utils/formatter/event.rs | Event formatting | Post-split ✅ |
| 406 | graph/vertex/data/children.rs | Child operations | Post-split ✅ |
| 397 | tests/state_advance.rs | State advancement tests | Test file ✅ |
| 396 | logging/tracing_utils/test_tracing.rs | Test tracing utilities | Acceptable ✅ |

### 🟢 Medium (300-400 lines) - Good Size
| Lines | File | Purpose |
|-------|------|---------|
| 391 | graph/vertex/token.rs | Token vertex operations |
| 387 | graph/mod.rs | Main graph module |
| 366 | path/structs/rooted/mod.rs | Rooted path structures |
| 363 | logging/path_format.rs | Path formatting for logs |
| 359 | path/structs/rooted/role_path/mod.rs | Role-based rooted paths |
| 350 | graph/vertex/pattern/mod.rs | Pattern matching on vertices |
| 318 | tests/public_api/trace_cache.rs | Trace cache tests |
| 310 | graph/getters/vertex.rs | Vertex getter methods |

### 🟢 Medium (200-299 lines) - Monitor
| Lines | File |
|-------|------|
| 299 | lib.rs |
| 298 | graph/vertex/atom.rs |
| 290 | tests/grammar.rs |
| 282 | trace/child/state.rs |
| 268 | logging/tracing_utils/mod.rs |
| 255 | tests/env/mod.rs |
| 255 | direction/match.rs |
| 246 | graph/vertex/location/child.rs |
| 242 | trace/cache/mod.rs |
| 240 | trace/command.rs |
| 240 | path/accessors/path_accessor.rs |
| 233 | tests/public_api/path_mutators/path_operations.rs |
| 225 | path/accessors/role.rs |
| 223 | graph/vertex/has_vertex_data.rs |
| 222 | trace/mod.rs |
| 212 | path/accessors/range_accessor.rs |
| 207 | path/structs/rooted/pattern_range.rs |

### ✅ Small (<200 lines) - Good Size
91 files under 200 lines (well-structured)

## Module Organization

### graph/ (21 files, 3,850 lines)
**Purpose:** Core graph data structure and operations

```
graph/
├── mod.rs (387) - Main module
├── insert.rs (502) - 🔴 SPLIT: Graph insertion
├── test_graph.rs (118)
├── validation.rs (68)
├── child_strings.rs (60)
├── kind.rs (34)
├── vertex/ (11 files, 2,312 lines)
│   ├── data.rs (699) - 🔴 SPLIT: Vertex data types
│   ├── token.rs (391) - 🟡 SPLIT: Token operations
│   ├── atom.rs (298)
│   ├── pattern/ (4 files, 472 lines)
│   │   ├── mod.rs (350) - 🟡 SPLIT: Pattern matching
│   │   ├── pattern_range.rs (93)
│   │   └── id.rs (29)
│   ├── location/ (3 files, 426 lines)
│   │   ├── child.rs (246)
│   │   ├── pattern.rs (133)
│   │   └── mod.rs (47)
│   ├── has_vertex_data.rs (223)
│   ├── parent.rs (167)
│   ├── wide.rs (91)
│   ├── has_vertex_index.rs (66)
│   ├── vertex_index.rs (64)
│   ├── key.rs (48)
│   ├── has_vertex_key.rs (30)
│   └── mod.rs (35)
└── getters/ (6 files, 661 lines)
    ├── vertex.rs (310) - 🟡 REVIEW: Large getter file
    ├── atom.rs (131)
    ├── pattern.rs (119)
    ├── utils.rs (111)
    ├── child.rs (93)
    ├── parent.rs (61)
    └── mod.rs (77)
```

**Issues:**
- `vertex/data.rs` (699) - Too large, contains multiple vertex data types
- `insert.rs` (502) - Complex insertion logic, should split by operation type
- `vertex/token.rs` (391) - Multiple token-related operations
- `vertex/pattern/mod.rs` (350) - Pattern matching logic

### path/ (35 files, 4,736 lines)
**Purpose:** Path structures and operations for graph traversal

```
path/
├── mod.rs (173)
├── structs/ (11 files, 1,846 lines)
│   ├── rooted/ (7 files, 1,668 lines)
│   │   ├── index_range.rs (510) - 🔴 SPLIT: Index range operations
│   │   ├── mod.rs (366) - 🟡 SPLIT: Rooted path core
│   │   ├── role_path/ (2 files, 402 lines)
│   │   │   ├── mod.rs (359) - 🟡 SPLIT: Role path operations
│   │   │   └── range.rs (43)
│   │   ├── pattern_range.rs (207)
│   │   ├── split_path.rs (110)
│   │   └── root.rs (74)
│   ├── role_path.rs (171)
│   ├── sub_path.rs (137)
│   └── mod.rs (3)
├── accessors/ (10 files, 1,219 lines)
│   ├── path_accessor.rs (240)
│   ├── role.rs (225)
│   ├── range_accessor.rs (212)
│   ├── root.rs (120)
│   ├── has_path.rs (119)
│   ├── border.rs (93)
│   ├── child/ (2 files, 174 lines)
│   │   ├── root.rs (90)
│   │   └── mod.rs (84)
│   ├── calc.rs (89)
│   └── mod.rs (8)
├── mutators/ (14 files, 498 lines)
│   ├── append.rs (140)
│   ├── move_path/ (7 files, 322 lines)
│   │   ├── key.rs (163)
│   │   ├── path.rs (55)
│   │   ├── root.rs (53)
│   │   ├── leaf.rs (36)
│   │   ├── advance.rs (35)
│   │   ├── retract.rs (19)
│   │   └── mod.rs (6)
│   ├── simplify.rs (46)
│   ├── pop.rs (31)
│   ├── lower.rs (26)
│   ├── raise.rs (12)
│   └── mod.rs (6)
└── ...
```

**Issues:**
- `structs/rooted/index_range.rs` (510) - Complex range operations
- `structs/rooted/mod.rs` (366) - Core rooted path logic
- `structs/rooted/role_path/mod.rs` (359) - Role-based path operations
- `accessors/path_accessor.rs` (240) - Multiple accessor methods

### trace/ (13 files, 1,922 lines)
**Purpose:** Bidirectional graph tracing and caching

```
trace/
├── mod.rs (222)
├── command.rs (240)
├── has_graph.rs (134)
├── traceable.rs (11)
├── child/ (3 files, 463 lines)
│   ├── state.rs (282)
│   ├── bands/ (2 files, 261 lines)
│   │   ├── mod.rs (183)
│   │   └── policy.rs (78)
│   ├── iterator.rs (62)
│   └── mod.rs (3)
├── state/ (2 files, 283 lines)
│   ├── parent.rs (186)
│   └── mod.rs (97)
└── cache/ (7 files, 501 lines)
    ├── mod.rs (242)
    ├── key/ (4 files, 288 lines)
    │   ├── directed/ (3 files, 317 lines)
    │   │   ├── mod.rs (195)
    │   │   ├── up.rs (62)
    │   │   └── down.rs (60)
    │   ├── props.rs (39)
    │   ├── prev.rs (34)
    │   └── mod.rs (3)
    ├── vertex/ (2 files, 120 lines)
    │   ├── mod.rs (77)
    │   └── positions.rs (43)
    ├── position.rs (77)
    └── new.rs (42)
```

**Issues:**
- Relatively well-structured
- `child/state.rs` (282) - Monitor for growth

### logging/ (12 files, 2,785 lines)
**Purpose:** Tracing, logging, and debugging utilities

```
logging/
├── tracing_utils/ (8 files, 2,395 lines)
│   ├── config.rs (728) - 🔴 SPLIT: Configuration types/logic
│   ├── formatter.rs (591) - 🔴 SPLIT: Multiple formatters
│   ├── test_tracing.rs (396) - 🟡 SPLIT: Test utilities
│   ├── mod.rs (268)
│   ├── field_visitor.rs (125)
│   ├── path.rs (78)
│   ├── string_utils.rs (62)
│   ├── timer.rs (44)
│   ├── syntax.rs (43)
│   └── panic.rs (35)
├── path_format.rs (363) - 🟡 SPLIT: Path formatting
├── compact_format.rs (163)
├── format_utils.rs (90)
└── mod.rs (27)
```

**Issues:**
- `tracing_utils/config.rs` (728) - Largest file, needs splitting
- `tracing_utils/formatter.rs` (591) - Multiple formatter types
- `tracing_utils/test_tracing.rs` (396) - Test-specific utilities

### tests/ (16 files, 2,893 lines)
**Purpose:** Unit and integration tests

```
tests/
├── macros.rs (618) - 🔴 SPLIT: Test helper macros
├── state_advance.rs (397) - 🟡 REVIEW
├── grammar.rs (290)
├── env/mod.rs (255)
├── public_api/ (6 files, 636 lines)
│   ├── trace_cache.rs (318) - 🟡 REVIEW
│   ├── path_mutators/ (4 files, 310 lines)
│   │   ├── path_operations.rs (233)
│   │   ├── move_key.rs (151)
│   │   ├── move_leaf.rs (147)
│   │   ├── move_root_index.rs (125)
│   │   ├── path_append.rs (24)
│   │   └── mod.rs (18)
│   ├── pattern_strings.rs (95)
│   └── mod.rs (8)
├── path_advance.rs (136)
├── test_string_repr.rs (135)
├── test_env1_string_repr.rs (40)
├── compact_format_demo.rs (87)
├── tracing_demo.rs (58)
├── graph.rs (41)
└── mod.rs (58)
```

## Recommendations

### Immediate Action (>500 lines)
1. **logging/tracing_utils/config.rs** (728) → Split into config types + config builder
2. **graph/vertex/data.rs** (699) → Split by vertex type (atom/pattern/wide)
3. **tests/macros.rs** (618) → Split by test category
4. **logging/tracing_utils/formatter.rs** (591) → Split formatters into separate files
5. **path/structs/rooted/index_range.rs** (510) → Split operations by type
6. **graph/insert.rs** (502) → Split by insertion algorithm

### Review Soon (300-500 lines)
7. **logging/tracing_utils/test_tracing.rs** (396)
8. **graph/vertex/token.rs** (391)
9. **graph/mod.rs** (387)
10. **path/structs/rooted/mod.rs** (366)
11. **logging/path_format.rs** (363)
12. **path/structs/rooted/role_path/mod.rs** (359)
13. **graph/vertex/pattern/mod.rs** (350)
