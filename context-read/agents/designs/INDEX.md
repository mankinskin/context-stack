# Designs Index — context-read

Algorithm and data-structure design documents for context-read.
Use these before implementing non-trivial changes — each document captures
the problem space, candidate approaches, constraints, and the chosen direction.

| Date | File | Status | Summary |
|------|------|--------|---------|
| 2026-03-15 | [20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md](20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md) | ✅ complete | Structural overlap complement construction around a shared overlap token. Chosen direction: path → `TraceCache` → recursive split/join in `context-insert`, with `context-read` remaining orchestration-only and Pass C3 deferred until semantic collapse is green. |