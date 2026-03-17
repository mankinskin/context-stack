# Interviews Index

Design interviews for context-read — questions, answers, and their implications.

| Date | File | Status | Summary |
|------|------|--------|---------|
| 2026-03-17 | [20260317_INTERVIEW_AAA_DECOMPOSITION_NEXT_STEP.md](20260317_INTERVIEW_AAA_DECOMPOSITION_NEXT_STEP.md) | 📋 pending | Focused `aaa` investigation: expected `[a, aa]` and `[aa, a]` decompositions, root/anchor semantics after unknown-segment seeding, overlap detection on the third `a`, and decision points for whether the fix belongs in `ExpansionCtx`, `BlockExpansionCtx`, or `RootManager`. |
| 2026-03-15 | [20260315_INTERVIEW_CONTEXT_READ_RESTRUCTURE.md](20260315_INTERVIEW_CONTEXT_READ_RESTRUCTURE.md) | ✅ complete | Module restructuring, public API shape, dead code disposition, overlap chain design, complement stub, `bands/` migration to `context-trace` |
| 2026-03-15 | [20260315_INTERVIEW_COMPLEMENT_AND_C3.md](20260315_INTERVIEW_COMPLEMENT_AND_C3.md) | ✅ complete | Structural path-driven overlap complements, path→TraceCache→split/join design, `context-insert` ownership, higher-level overlap bundling API, and deferred Pass C3 wiring |