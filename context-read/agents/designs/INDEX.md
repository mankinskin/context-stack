# Designs Index — context-read

Algorithm and data-structure design documents for context-read.
Use these before implementing non-trivial changes — each document captures
the problem space, candidate approaches, constraints, and the chosen direction.

| Date | File | Status | Summary |
|------|------|--------|---------|
| 2026-03-15 | [20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md](20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md) | 📋 design-session-pending | How to build the complement token from the downward path into the anchor vertex. Blocking all 10 failing overlap-collapse tests. Candidate approaches: clean-split via `insert_pattern`, manual `TraceCache` walk, search/checkpoint API. |