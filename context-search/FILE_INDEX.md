# context-search File Index

## Legacy / positioned split

The row-model search lives in the crate
[`context-search-legacy`](../context-search-legacy/README.md)
(`context_search_legacy::…`); its files are not indexed here. This crate
holds the positioned search only; its module layout and names mirror the
legacy search, only navigation (positions instead of rows) differs.

**Rule:** the legacy crates are frozen (no new code). New (positioned-model) code goes into this crate and must not depend on the legacy crates. Plan: `context-read/agents/plans/20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`.

**Updated:** 2026-11-26 (legacy extraction)

## Module Organization

```
src/
├── lib.rs (89) - exports `Find`, `Searchable`, `Response`, `SearchError`,
│                 paths, trace states, `TraceCache` and keys
├── error.rs (42) - `SearchError`
├── policy.rs (58)
├── path/ - search paths (moved from context-trace)
│   ├── mod.rs (58)
│   ├── role.rs (270) - `PathRole`, `Start`, `End`, position arithmetic
│   ├── role_path.rs (329) - `RolePath`, `StartPath`, `EndPath`, `QueryPath`
│   ├── root.rs (160) - `PathRoot`, `QueryRoot`
│   ├── range.rs (148) - `RangePath`, `HasRolePath`, `SpanCoverage`
│   └── navigate.rs (117) - `PathNavigate` (`end_at`, `advance_end_path`)
├── trace/ - trace states and cache (moved from context-trace)
│   ├── mod.rs (189) - `TraceCtx`, `TraceDirection`, `QueryAnchor`
│   ├── child.rs (166) - `ChildState`, `RootChildState`
│   ├── state/ (mod.rs 41, parent.rs 185) - `ParentState`
│   ├── cache/ (mod.rs 214, position.rs 69, vertex/) - `TraceCache`
│   ├── key/ (mod.rs 176, up.rs 113, down.rs 112) - `DirectedKey`, `UpKey`, `DownKey`
│   └── traceable/ (mod.rs 146, role.rs 65) - `Traceable` commands
├── cursor/ - cursor typestates and checkpoints
│   ├── mod.rs (164)
│   ├── checkpointed.rs (314)
│   ├── query.rs (71) - `QueryCursor`
│   ├── child.rs (63) - `ChildCursor`
│   └── state_machine.rs (47)
├── compare/
│   ├── mod.rs (5)
│   ├── iterator.rs (51)
│   ├── parent.rs (91)
│   └── state/ (core.rs 108, decomposition.rs 92, transitions.rs 110, mod.rs 17)
├── match/
│   ├── mod.rs (283)
│   ├── iterator.rs (123)
│   └── root_cursor/ (core.rs 116, advance.rs 84, mod.rs 13)
├── search/
│   ├── mod.rs (185)
│   ├── context.rs (45)
│   ├── searchable.rs (167)
│   └── stats.rs (126)
├── state/
│   ├── mod.rs (5)
│   ├── start.rs (82)
│   ├── matched.rs (334)
│   └── response.rs (124)
└── tests/
    ├── mod.rs (5)
    ├── search.rs (342)
    ├── deep.rs (250)
    ├── exhaustive.rs (585)
    ├── validate.rs (335)
    ├── navigation.rs (136) - `Navigate`/`PathNavigate` on v3 fixtures
    ├── trace/ (mod.rs 269, query.rs 131, exhaustive.rs 275) - paths, states, cache
    └── util.rs (112)
```
