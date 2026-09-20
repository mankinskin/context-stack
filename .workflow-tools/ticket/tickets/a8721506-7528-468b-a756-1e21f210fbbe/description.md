Residual static_complexity follow-up split out of batch 1c9e7b3e after singleton reductions.

Scope:
- context-stack/context-trace/src/graph/search_path.rs
- Remaining findings in this file: 2

Goal:
Reduce the paired complexity findings in search_path transition/application helpers while preserving path graph semantics.

Validation:
- Passed: rtk cargo check -p context-trace
- Passed: rtk audit --json run . > ../target/tmp/sc1_context_trace_search_path_after.json
- Audit result: search_path.rs static_complexity findings 2 -> 0 in ../target/tmp/sc1_context_trace_search_path_after.json

Implementation summary:
- Extracted shared VizPathGraph helpers for root promotion/demotion, child path mutation, cursor updates, and completion state.
- Grouped related visualization transition arms behind helper dispatch so apply_transition no longer carries every branch directly.
- Preserved existing search path reconstruction semantics and error conditions.

Parent context:
Batch 1c9e7b3e reduced from 38 to 17 findings before splitting this residual cluster.