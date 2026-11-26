# context-insert File Index

## Legacy / positioned split

The row-model insertion lives in the crate
[`context-insert-legacy`](../context-insert-legacy/README.md)
(`context_insert_legacy::…`); its files are not indexed here. This crate holds
the positioned insertion only.

**Rule:** the legacy crates are frozen (no new code). New (positioned-model) code goes into this crate and must not depend on the legacy crates. Plan: `context-read/agents/plans/20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`.

Python prototypes of positioned insertion (split graph and join, merge, invariant checks) with an execution manual: `agents/prototypes/README.md`.

**Updated:** 2026-11-26 (legacy extraction)

## Module Organization

```
src/
├── lib.rs (23) - exports `insert`, `insert_match`, `insert_next_match`,
│                 `InsertOutcome`, `InsertError`; modules `edit`, `fixtures`
├── insert/mod.rs (165) - `insert_match` and wrappers
├── split/mod.rs (373) - `SplitGraph` (split positions with roles)
├── join/mod.rs (149) - bottom-up `join` through `GraphEdit`
├── edit/ - mutation layer (moved from context-trace)
│   ├── mod.rs (340) - `GraphEdit`, `EditExt`
│   ├── operations.rs (215) - `atom`, `create`, `add_occurrences`, `extend`
│   ├── publication.rs (38) - `Publication`, `VertexEdit`
│   ├── dissolve.rs (337) - `Candidates`, `Removal`, `Settled`
│   ├── stable.rs (115) - stabilization
│   └── stage.rs (31) - `Stage` contract
├── fixtures.rs (139) - insert lane (`test-api`): `insert_cases`, `insert_manifest`
└── tests/
    ├── mod.rs (162)
    ├── split.rs (181)
    ├── insert.rs (333) - insert lane (`context-read/tests/fixtures/v3/insert`)
    ├── edit.rs (364) - `GraphEdit` operations
    ├── fixtures.rs (22) - insert lane cases
    └── interchange.rs (103) - table import vs. `GraphEdit` import
examples/
└── insert_manifest.rs (36) - rewrite the insert lane manifest
```
