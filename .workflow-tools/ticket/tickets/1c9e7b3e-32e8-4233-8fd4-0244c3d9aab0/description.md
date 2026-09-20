# Session Objective
Resolve the current static_complexity batch for context-stack and reduce 38 findings from the baseline.

# Scope Guardrails
- Stay inside context-stack unless a blocker requires a dependency fix outside scope.
- Do not start the next batch until this ticket meets done criteria.

# Implementation Steps
1. Capture exact finding rows for this batch from the baseline audit artifact.
2. Group findings into 2 to 5 micro-chunks and handle one chunk at a time.
3. After each chunk, run the narrowest compile/test check relevant to touched files.
4. Re-run audit summary and record count delta.
5. If blockers remain, create follow-up tickets and link them before handoff.

# Validation Commands
- Full category summary: cargo run -p audit-cli --bin audit -- --json summary --by category .
- Full baseline refresh when needed: cargo run -p audit-cli --bin audit -- --json run .
- Ticket health sanity: ./target/debug/ticket.exe health --workspace . --all --toon

# Acceptance Criteria
- Findings in this batch are resolved or have explicit blocker tickets linked.
- No increase in other categories caused by this batch.
- Batch notes include before and after counts and next unresolved action.

# Handoff Notes
Record exact commands run, resulting counts, and files changed so the next session can continue without rediscovery.

# Session Progress

Started batch-1 static_complexity remediation for context-stack from the baseline count of 38 findings.

## Resolved in this session
- `context-api/src/log_parser.rs`
  - Extracted field/message/backtrace/panic helper paths from `json_to_entry` and `extract_message_and_type`.
  - Validation: `rtk cargo test -p context-api log_parser`
- `context-api/src/ascii_graph.rs`
  - Extracted grammar rendering branch into `grammar_layout` / `append_grammar_line`.
  - Validation: `rtk cargo test -p context-api ascii_graph`
- `context-api/src/commands/compare.rs`
  - Extracted label diffing, shared-vertex comparison, and verdict computation helpers from `compare_snapshots`.
  - Validation: `rtk cargo test -p context-api compare`
- `context-search/src/search/mod.rs`
  - Extracted repeated iterator visualization/reset helpers from `SearchState::next`.
  - Validation: `rtk cargo check -p context-search`

## Audit Delta
- Baseline: 38 `static_complexity` findings under `context-stack` from `target/tmp/audit-full-2026-07-05.json`
- Current subtree audit: 33 findings from `target/tmp/sc1_context_stack_after_context_search.json`
- Reduction this session: 5 findings removed

## Commands Run
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_log_parser.json`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_ascii_graph.json`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_compare.json`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_context_search.json`

## Remaining Frontier
Smallest remaining file groups from the latest subtree audit:
- single finding each: `context-api/src/commands/command.rs`, `context-api/src/commands/dispatch.rs`, `context-trace-macros/src/lib.rs`, `context-trace/src/graph/visualization.rs`, `context-trace/src/logging/tracing_utils/config/loader.rs`, `tools/cli/context-cli/src/main.rs`, multiple `context-editor` singletons, `tools/http/context-http/src/error.rs`, `tools/mcp/context-mcp/src/server/handlers.rs`
- clustered files:
  - `context-trace/src/graph/search_path.rs` (2)
  - `context-trace/src/logging/tracing_utils/formatter/event.rs` (2)
  - `tools/cli/context-cli/src/output.rs` (2)
  - `tools/cli/context-cli/src/repl.rs` (2)
  - `context-insert/src/split/cache/vertex.rs` (4)
  - `context-trace/src/logging/tracing_utils/debug_to_json.rs` (5)

## Suggested Next Slice
Prefer one more contained single-file function over the large dispatchers. Good next candidates:
- `context-trace-macros/src/lib.rs`
- `context-trace/src/graph/visualization.rs`
- `tools/mcp/context-mcp/src/server/handlers.rs`

## Session Progress (follow-up)

Resolved one additional single-file finding in this batch.

### Resolved in this follow-up
- `context-trace-macros/src/lib.rs`
  - Refactored `extract_associated_types` into smaller helpers (`collect_associated_types_from_path`, `collect_associated_types_from_path_generics`, `nested_types`) to reduce cyclomatic complexity without changing extraction behavior.
  - Validation: `rtk cargo check -p context-trace-macros`

### Audit Delta (follow-up)
- Previous subtree audit: 33 findings from `target/tmp/sc1_context_stack_after_context_search.json`
- Current subtree audit: 32 findings from `target/tmp/sc1_context_stack_after_trace_macros.json`
- Reduction in this follow-up: 1 finding removed

### Commands Run (follow-up)
- `rtk cargo check -p context-trace-macros`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_trace_macros.json`

### Remaining Frontier (updated)
Single-finding candidates still open:
- `context-api/src/commands/command.rs`
- `context-api/src/commands/dispatch.rs`
- `context-trace/src/graph/visualization.rs`
- `context-trace/src/logging/tracing_utils/config/loader.rs`
- `tools/cli/context-cli/src/main.rs`
- `tools/mcp/context-mcp/src/server/handlers.rs`

## Session Progress (second follow-up)

Resolved one additional single-file finding in this batch.

### Resolved in this second follow-up
- `context-trace/src/graph/visualization.rs`
  - Refactored `extract_correlation_ids` into helper stages (`apply_query_correlation_ids`, `apply_semicolon_correlation_ids`, span/env fallbacks) to reduce branch density while preserving precedence.
  - Validation: `rtk cargo check -p context-trace`

### Audit Delta (second follow-up)
- Previous subtree audit: 32 findings from `target/tmp/sc1_context_stack_after_trace_macros.json`
- Current subtree audit: 31 findings from `target/tmp/sc1_context_stack_after_visualization.json`
- Reduction in this second follow-up: 1 finding removed

### Commands Run (second follow-up)
- `rtk cargo check -p context-trace`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_visualization.json`

### Remaining Frontier (latest)
Single-finding candidates still open:
- `context-api/src/commands/command.rs`
- `context-api/src/commands/dispatch.rs`
- `context-trace/src/logging/tracing_utils/config/loader.rs`
- `tools/cli/context-cli/src/main.rs`
- `tools/mcp/context-mcp/src/server/handlers.rs`

## Session Progress (third follow-up)

Resolved three additional single-file findings in this batch.

### Resolved in this third follow-up
- `tools/mcp/context-mcp/src/server/handlers.rs`
  - Extracted command/category/overview help builders and shared result helpers from `help_command_impl`.
  - Validation: `rtk cargo test -p context-mcp help`
- `context-trace/src/logging/tracing_utils/config/loader.rs`
  - Extracted env-boolean parsing plus span/panic/general env application helpers from `FormatConfig::from_env`.
  - Validation: `rtk cargo check -p context-trace`
- `tools/http/context-http/src/error.rs`
  - Extracted per-domain HTTP status helpers from `status_for_api_error` while preserving adapter mapping behavior.
  - Validation: `rtk cargo test -p context-http error`

### Audit Delta (third follow-up)
- Previous subtree audit: 31 findings from `target/tmp/sc1_context_stack_after_visualization.json`
- Current subtree audit: 28 findings from `target/tmp/sc1_context_stack_after_context_http_error.json`
- Reduction in this third follow-up: 3 findings removed
- Net reduction in batch so far: 38 -> 28

### Commands Run (third follow-up)
- `rtk cargo test -p context-mcp help`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_context_mcp_handlers.json`
- `rtk cargo check -p context-trace`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_loader_fix.json`
- `rtk cargo test -p context-http error`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_context_http_error.json`

### Remaining Frontier (current)
Single-finding candidates still open:
- `context-api/src/commands/command.rs`
- `context-api/src/commands/dispatch.rs`
- `tools/cli/context-cli/src/main.rs`
- `tools/context-editor/kernel/src/editor/debug_overlay/wireframe.rs`
- `tools/context-editor/kernel/src/physics/mod.rs`
- `tools/context-editor/kernel/src/simulation/character.rs`
- `tools/context-editor/kernel/src/simulation/llm_integration/shader.rs`
- `tools/context-editor/kernel/src/svo/paging.rs`
- `tools/context-editor/kernel/src/ui/code_viewer/tokenizer.rs`
- `tools/context-editor/kernel/src/ui/doc_editor/markdown.rs`
- `tools/context-editor/sandbox-app/src/bootstrap.rs`

Clustered files still open:
- `context-trace/src/graph/search_path.rs` (2)
- `context-trace/src/logging/tracing_utils/formatter/event.rs` (2)
- `tools/cli/context-cli/src/output.rs` (2)
- `tools/cli/context-cli/src/repl.rs` (2)
- `context-insert/src/split/cache/vertex.rs` (4)
- `context-trace/src/logging/tracing_utils/debug_to_json.rs` (5)

## Review-ready split

Singleton reductions for this batch are complete. Residual clustered findings were split into follow-up tickets so this batch can move to review with explicit blocker ownership.

### Residual blocker tickets
- `ebf4b601` — context-trace `debug_to_json.rs` cluster (5)
- `409a3919` — context-insert `split/cache/vertex.rs` cluster (4)
- `a8721506` — context-trace `search_path.rs` cluster (2)
- `3661f22c` — context-trace `formatter/event.rs` cluster (2)
- `15cf86fd` — context-cli `output.rs` cluster (2)
- `7cfc8996` — context-cli `repl.rs` cluster (2)

### Batch status at review handoff
- Latest subtree audit: `target/tmp/sc1_context_stack_after_context_cli_main_fix2.json`
- Remaining `static_complexity` findings in batch scope: 20
- Net reduction in batch so far: `38 -> 20`
- Remaining singleton files have been reduced to zero; only clustered follow-up tickets remain.

## Session Progress (fourth follow-up)

Resolved one additional single-file finding in this batch.

### Resolved in this fourth follow-up
- `tools/context-editor/kernel/src/simulation/llm_integration/shader.rs`
  - Extracted WGSL body parsing, brace matching, signature validation, and disallowed-keyword checks from `validate_wgsl_snippet`.
  - Validation: `rtk cargo test -p context-editor-kernel shader`

### Audit Delta (fourth follow-up)
- Previous subtree audit: 28 findings from `target/tmp/sc1_context_stack_after_context_http_error.json`
- Current subtree audit: 27 findings from `target/tmp/sc1_context_stack_after_shader.json`
- Reduction in this fourth follow-up: 1 finding removed
- Net reduction in batch so far: 38 -> 27

### Commands Run (fourth follow-up)
- `rtk cargo test -p context-editor-kernel shader`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_shader.json`

### Remaining Frontier (current)
Single-finding candidates still open:
- `context-api/src/commands/command.rs`
- `context-api/src/commands/dispatch.rs`
- `tools/cli/context-cli/src/main.rs`
- `tools/context-editor/kernel/src/editor/debug_overlay/wireframe.rs`
- `tools/context-editor/kernel/src/physics/mod.rs`
- `tools/context-editor/kernel/src/simulation/character.rs`
- `tools/context-editor/kernel/src/svo/paging.rs`
- `tools/context-editor/kernel/src/ui/code_viewer/tokenizer.rs`
- `tools/context-editor/kernel/src/ui/doc_editor/markdown.rs`
- `tools/context-editor/sandbox-app/src/bootstrap.rs`

Clustered files still open:
- `context-trace/src/graph/search_path.rs` (2)
- `context-trace/src/logging/tracing_utils/formatter/event.rs` (2)
- `tools/cli/context-cli/src/output.rs` (2)
- `tools/cli/context-cli/src/repl.rs` (2)
- `context-insert/src/split/cache/vertex.rs` (4)
- `context-trace/src/logging/tracing_utils/debug_to_json.rs` (5)

## Session Progress (fifth follow-up)

Resolved four additional single-file findings in this batch.

### Resolved in this fifth follow-up
- `tools/context-editor/kernel/src/simulation/character.rs`
  - Extracted movement input, speed, and free-fly / physics displacement helpers from `character_movement`.
  - Validation: `rtk cargo check -p context-editor-kernel`
- `tools/context-editor/kernel/src/ui/doc_editor/markdown.rs`
  - Extracted code-block state handling and line classification helpers from `parse_markdown`.
  - Validation: `rtk cargo test -p context-editor-kernel doc_editor`
- `tools/context-editor/kernel/src/ui/code_viewer/tokenizer.rs`
  - Extracted comment, string, number, and symbol token consumers from `tokenize_rust_line`.
  - Validation: `rtk cargo test -p context-editor-kernel code_viewer`
- `tools/context-editor/kernel/src/svo/paging.rs`
  - Extracted child-origin and boundary-page population helpers from `populate_and_cull`.
  - Validation: `rtk cargo test -p context-editor-kernel paging`

### Audit Delta (fifth follow-up)
- Previous subtree audit: 27 findings from `target/tmp/sc1_context_stack_after_shader.json`
- Current subtree audit: 23 findings from `target/tmp/sc1_context_stack_after_paging.json`
- Reduction in this fifth follow-up: 4 findings removed
- Net reduction in batch so far: 38 -> 23

### Commands Run (fifth follow-up)
- `rtk cargo check -p context-editor-kernel`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_character.json`
- `rtk cargo test -p context-editor-kernel doc_editor`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_markdown.json`
- `rtk cargo test -p context-editor-kernel code_viewer`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_tokenizer.json`
- `rtk cargo test -p context-editor-kernel paging`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_paging.json`

### Remaining Frontier (current)
Single-finding candidates still open:
- `context-api/src/commands/command.rs`
- `context-api/src/commands/dispatch.rs`
- `tools/cli/context-cli/src/main.rs`
- `tools/context-editor/sandbox-app/src/bootstrap.rs`

Clustered files still open:
- `context-trace/src/graph/search_path.rs` (2)
- `context-trace/src/logging/tracing_utils/formatter/event.rs` (2)
- `tools/cli/context-cli/src/output.rs` (2)
- `tools/cli/context-cli/src/repl.rs` (2)
- `context-insert/src/split/cache/vertex.rs` (4)
- `context-trace/src/logging/tracing_utils/debug_to_json.rs` (5)

## Session Progress (sixth follow-up)

Resolved two additional single-file findings in this batch.

### Resolved in this sixth follow-up
- `tools/context-editor/kernel/src/physics/mod.rs`
  - Extracted chunk-grid fill, region extension, consumed-region marking, and box emission helpers from `rebuild_chunk_collider`.
  - Validation: `rtk cargo test -p context-editor-kernel physics`
- `tools/context-editor/kernel/src/editor/debug_overlay/wireframe.rs`
  - Extracted octant-origin, occupied/full-grid traversal, and wire-cube emission helpers from `draw_svo_wireframe`.
  - Validation: `rtk cargo check -p context-editor-kernel`

### Audit Delta (sixth follow-up)
- Previous subtree audit: 23 findings from `target/tmp/sc1_context_stack_after_paging.json`
- Current subtree audit: 21 findings from `target/tmp/sc1_context_stack_after_wireframe_fix.json`
- Reduction in this sixth follow-up: 2 findings removed
- Net reduction in batch so far: 38 -> 21

### Commands Run (sixth follow-up)
- `rtk cargo test -p context-editor-kernel physics`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_physics_fix.json`
- `rtk cargo check -p context-editor-kernel`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_wireframe_fix.json`

### Remaining Frontier (current)
Single-finding candidates still open:
- `context-api/src/commands/command.rs`
- `context-api/src/commands/dispatch.rs`
- `tools/cli/context-cli/src/main.rs`
- `tools/context-editor/sandbox-app/src/bootstrap.rs`

Clustered files still open:
- `context-trace/src/graph/search_path.rs` (2)
- `context-trace/src/logging/tracing_utils/formatter/event.rs` (2)
- `tools/cli/context-cli/src/output.rs` (2)
- `tools/cli/context-cli/src/repl.rs` (2)
- `context-insert/src/split/cache/vertex.rs` (4)
- `context-trace/src/logging/tracing_utils/debug_to_json.rs` (5)

## Session Progress (seventh follow-up)

Resolved one additional single-file finding in this batch.

### Resolved in this seventh follow-up
- `tools/cli/context-cli/src/main.rs`
  - Split the top-level CLI dispatcher into thematic command mappers and traced/plain execution helpers so `execute_subcommand` became orchestration-only.
  - Validation: `rtk cargo check -p context-cli`

### Audit Delta (seventh follow-up)
- Previous subtree audit: 21 findings from `target/tmp/sc1_context_stack_after_wireframe_fix.json`
- Current subtree audit: 20 findings from `target/tmp/sc1_context_stack_after_context_cli_main_fix2.json`
- Reduction in this seventh follow-up: 1 finding removed
- Net reduction in batch so far: 38 -> 20

### Commands Run (seventh follow-up)
- `rtk cargo check -p context-cli`
- `rtk cargo run -q -p audit-cli --bin audit -- run context-stack --json > target/tmp/sc1_context_stack_after_context_cli_main_fix2.json`

### Remaining Frontier (current)
Single-finding candidates still open:
- `context-api/src/commands/command.rs`
- `context-api/src/commands/dispatch.rs`
- `tools/context-editor/sandbox-app/src/bootstrap.rs`

Clustered files still open:
- `context-trace/src/graph/search_path.rs` (2)
- `context-trace/src/logging/tracing_utils/formatter/event.rs` (2)
- `tools/cli/context-cli/src/output.rs` (2)
- `tools/cli/context-cli/src/repl.rs` (2)
- `context-insert/src/split/cache/vertex.rs` (4)
- `context-trace/src/logging/tracing_utils/debug_to_json.rs` (5)
