# Plans Index — context-read

Plans for context-read crate refactorings, algorithm work, and feature additions.

## Active Plans

| Date | File | Status | Summary |
|------|------|--------|---------|
| 2026-03-15 | [20260315_PLAN_CONTEXT_READ_RESTRUCTURE](20260315_PLAN_CONTEXT_READ_RESTRUCTURE.md) | 📋 ready | Three-pass restructuring: migrate `bands/` to `context-trace`, delete dead code, rename `context/` → `pipeline/`, introduce `IntoReadInput` trait, expose `pub fn read`, consolidate overlap chain types, seed `benches/`. |

## Templates

See root `agents/plans/20251203_PLAN_TEMPLATE.md` for the plan template.