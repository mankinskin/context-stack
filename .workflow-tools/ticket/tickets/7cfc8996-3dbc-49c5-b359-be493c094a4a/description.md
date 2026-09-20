Residual static_complexity follow-up split out of batch 1c9e7b3e after singleton reductions.

Scope:
- context-stack/tools/cli/context-cli/src/repl.rs
- Remaining findings in this file: 2

Goal:
Reduce the paired complexity findings in REPL command handling while preserving current interactive behavior.

Validation:
- Passed: rtk cargo check -p context-cli
- Passed: rtk audit --json run . > ../target/tmp/sc1_context_cli_repl_after.json
- Audit result: repl.rs static_complexity findings 2 -> 0 in ../target/tmp/sc1_context_cli_repl_after.json

Implementation summary:
- Split the REPL loop into prompt/read/input helpers so run no longer carries all line-handling branches directly.
- Replaced the monolithic execute_repl_line match tree with a table-driven dispatcher and focused per-command handlers.
- Preserved existing command names, aliases, tracing behavior, workspace activation semantics, and compare/log command behavior.

Parent context:
Batch 1c9e7b3e reduced from 38 to 17 findings before splitting this residual cluster.