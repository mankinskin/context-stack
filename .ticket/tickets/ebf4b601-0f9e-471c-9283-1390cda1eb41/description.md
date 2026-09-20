Residual static_complexity follow-up split out of batch 1c9e7b3e after singleton reductions.

Scope:
- context-stack/context-trace/src/logging/tracing_utils/debug_to_json.rs
- Remaining findings in this file: 5

Goal:
Reduce the clustered complexity findings in debug_to_json helpers without changing parsing behavior.

Validation:
- Passed: rtk cargo check --manifest-path "$PWD/context-trace/Cargo.toml"
- Passed: cargo run -p audit-cli --bin audit -- --json run context-stack > target/tmp/sc1_context_trace_debug_to_json_after.json
- Audit result: debug_to_json.rs static_complexity findings 5 -> 0 in target/tmp/sc1_context_trace_debug_to_json_after.json

Implementation summary:
- Extracted shared delimiter and nesting helpers for balanced Rust Debug parsing instead of repeating branch-heavy scanners in multiple functions.
- Reused top-level item and field split helpers across struct-field and tuple parsing so nested-bracket handling stays centralized.
- Consolidated signature-store merge/update paths for span, spans, and fields processing while preserving the existing fn_sig and self_type collection behavior.

Parent context:
Batch 1c9e7b3e reduced from 38 to 17 findings before splitting this residual cluster.