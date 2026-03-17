# Plans Index — context-read

Plans for context-read crate refactorings, algorithm work, and feature additions.

## Active Plans

| Date | File | Status | Summary |
|------|------|--------|---------|
| 2026-03-17 | [20260317_PLAN_AAA_DECOMPOSITION_FIX](20260317_PLAN_AAA_DECOMPOSITION_FIX.md) | 📋 ready | Fix missing `[aa, a]` decomposition for `aaa` by relaxing the atom-anchor suppression guards in `ExpansionCtx::next()`. Includes instrumentation phase, regression matrix for repeated-minimal cases, and generalisation verification. |
| 2026-03-15 | [20260315_PLAN_CONTEXT_READ_RESTRUCTURE](20260315_PLAN_CONTEXT_READ_RESTRUCTURE.md) | 📋 ready | Three-pass restructuring: migrate `bands/` to `context-trace`, delete dead code, rename `context/` → `pipeline/`, introduce `IntoReadInput` trait, expose `pub fn read`, consolidate overlap chain types, seed `benches/`. |
| 2026-03-15 | [20260315_PLAN_COMPLEMENT_AND_C3](20260315_PLAN_COMPLEMENT_AND_C3.md) | 📋 ready | Fix structural overlap complements via path→TraceCache→split/join in `context-insert`, add durable overlap bundling API, migrate `context-read` collapse to that API, then wire Pass C3 after semantic collapse is green. |

## Templates

See root `agents/plans/20251203_PLAN_TEMPLATE.md` for the plan template.