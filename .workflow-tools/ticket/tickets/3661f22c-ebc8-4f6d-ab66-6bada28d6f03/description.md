Residual static_complexity follow-up split out of batch 1c9e7b3e after singleton reductions.

Scope:
- context-stack/context-trace/src/logging/tracing_utils/formatter/event.rs
- Remaining findings in this file: 2

Goal:
Reduce the paired complexity findings in tracing formatter event helpers without changing rendered event output.

Validation:
- Passed: rtk cargo check -p context-trace
- Passed: rtk audit --json run . > ../target/tmp/sc1_context_trace_formatter_after.json
- Audit result: formatter/event.rs static_complexity findings 2 -> 0 in ../target/tmp/sc1_context_trace_formatter_after.json

Implementation summary:
- Split format_event into helpers for event classification, whitespace policy, indentation, level selection, file-location suffixes, and span-field gating.
- Split span-enter message formatting into trait-context, associated-type, and signature helpers while preserving the rendered formatter message content.
- Kept existing close-event and non-span event rendering behavior intact.

Parent context:
Batch 1c9e7b3e reduced from 38 to 17 findings before splitting this residual cluster.