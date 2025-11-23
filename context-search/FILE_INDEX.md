# context-search File Index

**Generated:** 2025-11-23  
**Git Commit:** 6d74dcb (6d74dcbc4733fc3f0645eae86346b033fea9d24f)  
**Commit Date:** 2025-11-23 15:20:32 +0100  
**Commit Message:** Refactor path accessors and traits for improved clarity and consistency

**Total:** 46 files, 8,181 lines

## Summary Statistics

| Category | Files | Lines | % of Total |
|----------|-------|-------|------------|
| Match | 3 | 1,257 | 15.4% |
| Compare | 3 | 919 | 11.2% |
| State | 7 | 1,360 | 16.6% |
| Search | 6 | 579 | 7.1% |
| Cursor | 5 | 707 | 8.6% |
| Tests | 10 | 2,661 | 32.5% |
| Logging | 2 | 357 | 4.4% |
| Other | 10 | 341 | 4.2% |

## Files by Size Category

### 🔴 Very Large (>500 lines) - Split Priority
| Lines | File | Purpose |
|-------|------|---------|
| 815 | match/root_cursor.rs | Root cursor matching logic |
| 725 | compare/state.rs | State comparison and decomposition |
| 544 | tests/state_advance.rs | State advancement tests |

### 🟡 Large (300-500 lines) - Review for Splitting
| Lines | File | Purpose |
|-------|------|---------|
| 497 | tests/state_advance_integration.rs | Integration tests |
| 434 | tests/search/ancestor.rs | Ancestor search tests |
| 424 | state/start.rs | Start state logic |
| 369 | tests/examples.rs | Example tests |
| 368 | tests/traversal.rs | Traversal tests |
| 350 | state/end/mod.rs | End state logic |
| 345 | search/mod.rs | Main search module |
| 319 | logging/mod.rs | Logging utilities |

### 🟢 Medium (200-299 lines) - Monitor
| Lines | File |
|-------|------|
| 293 | cursor/mod.rs |
| 276 | match/iterator.rs |

### ✅ Small (<200 lines) - Good Size
31 files under 200 lines (well-structured)

## Module Organization

### match/ (3 files, 1,257 lines)
**Purpose:** Match iteration and root cursor logic

```
match/
├── root_cursor.rs (815) - 🔴 SPLIT: Root cursor operations
├── iterator.rs (276)
└── mod.rs (166)
```

**Issues:**
- `root_cursor.rs` (815) - Largest file, complex root cursor logic
  - Should split into: initialization, advancement, state transitions

### compare/ (3 files, 919 lines)
**Purpose:** Token comparison and state management

```
compare/
├── state.rs (725) - 🔴 SPLIT: CompareState operations
├── parent.rs (116)
├── iterator.rs (78)
└── mod.rs (3)
```

**Issues:**
- `state.rs` (725) - Recently refactored but still large
  - Contains: state transitions, prefix decomposition, cursor advancement
  - Could split into: core state, transitions, decomposition

### state/ (7 files, 1,360 lines)
**Purpose:** Search state machine and state types

```
state/
├── start.rs (424) - 🟡 SPLIT: Start state logic
├── end/ (4 files, 578 lines)
│   ├── mod.rs (350) - 🟡 SPLIT: End state core
│   ├── postfix.rs (91)
│   ├── range.rs (79)
│   └── prefix.rs (58)
├── matched/ (1 file, 119 lines)
│   └── mod.rs (119)
├── result.rs (97)
├── inner_kind.rs (48)
└── mod.rs (91)
```

**Issues:**
- `start.rs` (424) - Start state initialization and transitions
- `end/mod.rs` (350) - End state logic, multiple match types

### search/ (6 files, 579 lines)
**Purpose:** Search algorithms and entry points

```
search/
├── mod.rs (345) - 🟡 REVIEW: Main search logic
├── context.rs (93)
├── bft.rs (74)
├── final_state.rs (50)
├── searchable.rs (26)
└── ext.rs (21)
```

**Issues:**
- `mod.rs` (345) - Core search implementation
  - Could extract: algorithm variants, result handling

### cursor/ (5 files, 707 lines)
**Purpose:** Cursor types for query traversal

```
cursor/
├── mod.rs (293) - Monitor growth
├── checkpointed.rs (194)
├── path.rs (142)
├── state_machine.rs (47)
└── position.rs (31)
```

**Structure:** Well-organized, manageable sizes

### tests/ (10 files, 2,661 lines)
**Purpose:** Unit and integration tests

```
tests/
├── state_advance.rs (544) - 🔴 SPLIT: State tests
├── state_advance_integration.rs (497) - 🟡 SPLIT: Integration tests
├── search/ (4 files, 838 lines)
│   ├── ancestor.rs (434) - 🟡 SPLIT: Ancestor search tests
│   ├── mod.rs (183)
│   ├── consecutive.rs (116)
│   └── parent.rs (105)
├── examples.rs (369) - 🟡 SPLIT: Example tests
├── traversal.rs (368) - 🟡 SPLIT: Traversal tests
├── macros.rs (17)
└── mod.rs (5)
```

**Issues:**
- Large test files make debugging harder
- Should group by feature/component

### logging/ (2 files, 357 lines)
**Purpose:** Debug logging and formatting

```
logging/
├── mod.rs (319) - 🟡 REVIEW: Logging utilities
└── cursor_format.rs (38)
```

### container/ (4 files, 150 lines)
**Purpose:** State container and traversal order

```
container/
├── bft.rs (72)
├── dft.rs (45)
├── order.rs (17)
└── mod.rs (16)
```

**Structure:** Good, small focused files

### traversal/ (2 files, 143 lines)
**Purpose:** Traversal policies

```
traversal/
├── mod.rs (80)
└── policy.rs (63)
```

## Recommendations

### Immediate Action (>500 lines)
1. **match/root_cursor.rs** (815) → Split into:
   - `root_cursor/core.rs` - RootCursor struct and basic operations
   - `root_cursor/advance.rs` - Advancement logic
   - `root_cursor/state.rs` - State transitions

2. **compare/state.rs** (725) → Split into:
   - `state/core.rs` - CompareState struct and basic operations
   - `state/transitions.rs` - State transition logic
   - `state/decomposition.rs` - Token decomposition (prefix methods)

3. **tests/state_advance.rs** (544) → Split by test category:
   - Group related tests together
   - Consider splitting by state type being tested

### Review Soon (300-500 lines)
4. **tests/state_advance_integration.rs** (497) → Split by scenario
5. **tests/search/ancestor.rs** (434) → Split by test type
6. **state/start.rs** (424) → Extract state transition logic
7. **tests/examples.rs** (369) → Split by example type
8. **tests/traversal.rs** (368) → Split by traversal mode
9. **state/end/mod.rs** (350) → Extract match type handlers
10. **search/mod.rs** (345) → Extract algorithm variants
11. **logging/mod.rs** (319) → Split logging utilities

### Module Restructuring Opportunities

#### compare/ module
Current structure is flat but logical. Could benefit from:
```
compare/
├── state/
│   ├── core.rs - CompareState struct
│   ├── transitions.rs - State transitions
│   ├── decomposition.rs - Prefix decomposition
│   └── mod.rs
├── parent.rs
├── iterator.rs
└── mod.rs
```

#### match/ module
Need better organization:
```
match/
├── root_cursor/
│   ├── core.rs - RootCursor struct
│   ├── advance.rs - Advancement logic
│   ├── state.rs - State management
│   └── mod.rs
├── iterator.rs
└── mod.rs
```

#### state/ module
Already hierarchical but some files too large:
```
state/
├── start/
│   ├── core.rs - Start state struct
│   ├── transitions.rs - Transition logic
│   └── mod.rs
├── end/
│   ├── core.rs - End state struct
│   ├── postfix.rs
│   ├── range.rs
│   ├── prefix.rs
│   └── mod.rs
├── matched/mod.rs
├── result.rs
├── inner_kind.rs
└── mod.rs
```
