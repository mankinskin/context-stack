# Goal
Reduce file_length findings from 182 to zero using safe module splits and behavior-preserving moves.

# Planning Scope
Batch sequence:
1. memory-api (90)
2. context-stack (39)
3. tools (27)
4. memory-viewers (19)
5. viewer-api (7)

# Implementation Strategy
- Split by cohesive feature boundaries, not arbitrary line chunks.
- Move tests with code when possible to keep locality.
- Keep public API paths stable unless change is required and documented.

# Validation Plan
- cargo check and relevant tests for each touched workspace area.
- Re-run audit summary by category after each batch.
- Verify no static complexity regressions from splitting.

# Done Criteria
- file_length findings reach zero or remaining exceptions are tracked by explicit follow-up tickets with rationale.