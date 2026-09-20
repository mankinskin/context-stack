# Goal
Reduce static_complexity findings from 108 to zero through staged complexity reduction, not broad rewrites.

# Planning Scope
Batch sequence:
1. context-stack (38)
2. tools (29)
3. memory-api (28)
4. memory-viewers (7)
5. viewer-api (6)

# Implementation Strategy
- Start with highest-count area to maximize reduction early.
- Prefer extraction of pure helper functions and smaller command handlers.
- Preserve behavior with snapshot tests and targeted unit coverage.

# Validation Plan
- Run crate-local tests after each refactor cluster.
- Run audit summary by category after each batch.
- Capture complexity deltas in batch notes.

# Done Criteria
- static_complexity category reduced to zero or documented unavoidable residuals with explicit waivers and owner tickets.