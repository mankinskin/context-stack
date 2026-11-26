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
├── lib.rs (32) - exports `Find`, `Searchable`, `Response`, `SearchError`
├── error.rs (42) - `SearchError`
├── policy.rs (58)
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
    └── util.rs (112)
```
