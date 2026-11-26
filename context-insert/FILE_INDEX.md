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
├── lib.rs (18) - exports `insert`, `insert_match`, `insert_next_match`,
│                 `InsertOutcome`, `InsertError`
├── insert/mod.rs (165) - `insert_match` and wrappers
├── split/mod.rs (373) - `SplitGraph` (split positions with roles)
├── join/mod.rs (149) - bottom-up `join` through `GraphEdit`
└── tests/
    ├── mod.rs (157)
    ├── split.rs (183)
    └── insert.rs (328) - insert lane (`context-read/tests/fixtures/v3/insert`)
```
