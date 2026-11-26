# context-trace File Index

## Legacy / positioned split

The row model (pattern-row `Hypergraph`, `direction/`, row `graph/`,
`path/`, `trace/`, `logging/path_format.rs`, `tests/`) lives in the crate
[`context-trace-legacy`](../context-trace-legacy/README.md)
(`context_trace_legacy::…`); its files are not indexed here.

- **Shared (model-agnostic):** `logging/`, `graph/kind.rs` (`GraphKind`),
  `graph/vertex/{atom, id, key, token, wide, has_vertex_index}`,
  `graph/vertex/name_resolver.rs` (thread-local `TokenNameResolver` hook used by
  `Token`/`VertexIndex` display), `position.rs` (`AtomPosition`).
- **`positioned/` (no legacy dependency):**

```
src/positioned/
├── mod.rs - exports; `raw` only with hidden feature `edit`
├── check.rs - store-generic structural checks, `validate_store`
├── model/ - composition.rs, location.rs, vertex.rs, error.rs
├── store/ - read.rs (`GraphRead`, `VertexRead`), composition.rs,
│            has_graph.rs, navigate.rs (`Navigate`, `read_interval`),
│            write.rs (`GraphWrite`, sealed `raw` primitives)
├── committed/ - graph.rs (`PositionedGraph`, validation, debug labels),
│                access.rs, serde.rs, table.rs (table construction)
├── interchange/ - mod.rs (v3 format + validator), convert.rs,
│                  diff.rs, fixtures.rs (`Scenario`, `FixtureGraph`)
└── tests/ - composition.rs, diff.rs, fixtures.rs (round trip, name map,
             labels), guard.rs (no content-keyed field, `edit` feature
             users, `crates_depend_only_on_lower_layers`), interchange.rs,
             model.rs, navigation.rs, serde.rs
```

Paths, trace states, the trace cache and its keys moved to
[`context-search`](../context-search/FILE_INDEX.md); `GraphEdit` and the
insert fixture lane to [`context-insert`](../context-insert/FILE_INDEX.md);
the overlay stage and the read fixture lane to
[`context-read`](../context-read/FILE_INDEX.md).

**Rule:** the legacy crates are frozen (no new code). New (positioned-model) code goes into this crate and must not depend on `context-trace-legacy`. Plan: `context-read/agents/plans/20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`.

The snapshot below (2025-11-23) is limited to the files still in this crate;
its totals and statistics predate the legacy extraction.

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
| Logging | 12 | 2,785 | 15.1% |
| Other | 24 | 1,894 | 10.3% |

## Files by Size Category

### ✅ All Large Files Split! (Phase 2 Complete)

**Previously >500 lines (now split):**
- ✅ logging/tracing_utils/config.rs (728 → 305 lines largest)
- ✅ logging/tracing_utils/formatter.rs (591 → 408 lines largest)

### 🟢 Largest Files Now (400-500 lines) - Acceptable
| Lines | File | Purpose | Status |
|-------|------|---------|--------|
| 408 | logging/tracing_utils/formatter/event.rs | Event formatting | Post-split ✅ |
| 396 | logging/tracing_utils/test_tracing.rs | Test tracing utilities | Acceptable ✅ |

### 🟢 Medium (300-400 lines) - Good Size
| Lines | File | Purpose |
|-------|------|---------|
| 391 | graph/vertex/token.rs | Token vertex operations |
| 387 | graph/mod.rs | Main graph module |

### 🟢 Medium (200-299 lines) - Monitor
| Lines | File |
|-------|------|
| 299 | lib.rs |
| 298 | graph/vertex/atom.rs |
| 268 | logging/tracing_utils/mod.rs |

### ✅ Small (<200 lines) - Good Size
91 files under 200 lines (well-structured)

## Module Organization

### graph/ (21 files, 3,850 lines)
**Purpose:** Core graph data structure and operations

```
graph/
├── mod.rs (387) - Main module
├── kind.rs (34)
└── vertex/ (11 files, 2,312 lines)
    ├── token.rs (391) - 🟡 SPLIT: Token operations
    ├── atom.rs (298)
    ├── wide.rs (91)
    ├── has_vertex_index.rs (66)
    ├── vertex_index.rs (64)
    ├── key.rs (48)
    └── mod.rs (35)
```

**Issues:**
- `vertex/token.rs` (391) - Multiple token-related operations

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
├── compact_format.rs (163)
├── format_utils.rs (90)
└── mod.rs (27)
```

**Issues:**
- `tracing_utils/config.rs` (728) - Largest file, needs splitting
- `tracing_utils/formatter.rs` (591) - Multiple formatter types
- `tracing_utils/test_tracing.rs` (396) - Test-specific utilities

## Recommendations

### Immediate Action (>500 lines)
1. **logging/tracing_utils/config.rs** (728) → Split into config types + config builder
2. **logging/tracing_utils/formatter.rs** (591) → Split formatters into separate files

### Review Soon (300-500 lines)
3. **logging/tracing_utils/test_tracing.rs** (396)
4. **graph/vertex/token.rs** (391)
5. **graph/mod.rs** (387)
