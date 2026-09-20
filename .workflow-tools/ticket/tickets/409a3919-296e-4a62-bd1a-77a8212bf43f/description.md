Residual static_complexity follow-up split out of batch 1c9e7b3e after singleton reductions.

Scope:
- context-stack/context-insert/src/split/cache/vertex.rs
- Remaining findings in this file: 4

Goal:
Reduce the clustered complexity findings in split/cache/vertex.rs while preserving cache partition logic.

Validation:
- Passed: rtk cargo check -p context-insert
- Passed: rtk audit --json run . > ../target/tmp/sc1_context_insert_vertex_after.json
- Audit result: vertex.rs static_complexity findings 4 -> 0 in ../target/tmp/sc1_context_insert_vertex_after.json

Implementation summary:
- Extracted shared range calculators for target and wrapper partition computation in root_augmentation.
- Extracted shared wrapper split insertion helpers and reused them across postfix, prefix, and infix wrapper-offset builders.
- Preserved existing cache partition and wrapper-boundary semantics while removing repeated branch-heavy code.

Parent context:
Batch 1c9e7b3e reduced from 38 to 17 findings before splitting this residual cluster.