# Analysis Index — context-read

Algorithm analysis, state investigations, and failure post-mortems for the context-read crate.

| Date | File | Status | Summary |
|------|------|--------|---------|
| 2026-03-17 | [20260317_ANALYSIS_AAA_SEGMENT_BOUNDARY_AND_OUTER_LOOP.md](20260317_ANALYSIS_AAA_SEGMENT_BOUNDARY_AND_OUTER_LOOP.md) | 📋 active | Skill-informed reframing of the `aaa` failure: segmentation is valid, the bug is atom-anchor suppression in `ExpansionCtx` preventing the symmetric `[aa, a]` decomposition from being materialised across the segment boundary. |
| 2026-03-15 | [20260315_ANALYSIS_AAA_DECOMPOSITION_NEXT_STEP.md](20260315_ANALYSIS_AAA_DECOMPOSITION_NEXT_STEP.md) | 📋 active | Original `aaa` failure analysis: minimal semantic failure, established that root-latch hypothesis does not explain missing `[aa, a]`, recommended tracing `ExpansionCtx` as the next debugging step. |