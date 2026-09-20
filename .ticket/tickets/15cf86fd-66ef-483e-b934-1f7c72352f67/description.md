Residual static_complexity follow-up split out of batch 1c9e7b3e after singleton reductions.

Scope:
- context-stack/tools/cli/context-cli/src/output.rs
- Remaining findings in this file: 2

Goal:
Reduce the paired complexity findings in CLI output helpers while preserving current human-readable formatting.

Validation:
- Passed: rtk cargo check -p context-cli
- Passed: rtk audit --json run . > ../target/tmp/sc1_context_cli_output_after.json
- Audit result: output.rs static_complexity findings 2 -> 0 in ../target/tmp/sc1_context_cli_output_after.json

Implementation summary:
- Grouped print_command_result by result family so the top-level dispatcher no longer carries every variant branch directly.
- Extracted focused graph-diff section helpers so shared/exclusive vertex rendering and summary printing are split into smaller units.
- Preserved existing CLI output text and ordering.

Parent context:
Batch 1c9e7b3e reduced from 38 to 17 findings before splitting this residual cluster.