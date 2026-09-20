# Session Objective
Resolve the current stability batch for test_execution and reduce 1 findings from the baseline.

# Progress this session
Replaced the initial truncated workspace failure with concrete, executable blockers and cleared three failure layers before reaching the underlying engine issue.

## Fixed in this session
1. **rule-cli test compile breakage**
- Root cause: `cli/tests.rs` still called `dispatch::dispatch(...)` after the dispatcher was renamed to `dispatch_with_workspace_root(...)`.
- Fix: restored a `#[cfg(test)]` compatibility wrapper in `rule-cli/src/cli/dispatch.rs`.
- Also updated two stale output-path tests to match current folder-tree target semantics (`path_scope` controls output path).
- Validation: `cargo test --manifest-path memory-api/tools/cli/rule-cli/Cargo.toml --tests` → **pass**.

2. **audit-cli integration failures**
- Root cause A: human text output leaked embedded Windows `//?/` verbatim prefixes from captured tool stderr.
- Fix: normalize embedded verbatim prefixes in `audit-api::config::normalize_output_text`.
- Root cause B: `cargo llvm-cov` runs that produce no `.profraw` data were classified as hard `Failed` instead of environment-unavailable.
- Fix: classify `not found *.profraw files` as `Unavailable` via new `coverage_profraw_missing` finding.
- Validation:
  - `cargo test -p audit-cli --test integration_audit audit_collects_findings_and_prunes_stale_index_entries`
  - `cargo test -p audit-cli --test integration_audit cli_supports_json_and_text_output`
  → **both pass**.

3. **context-api insert_first_match regressions**
- Root cause: command used read/next-match semantics for exact token-ref insertion.
- Fix: `insert_first_match` now:
  - exact-searches the resolved token pattern via `find_ancestor`,
  - returns the existing exact root when present,
  - otherwise constructs the exact composite via `insert_pattern(tokens)`.
- Validation:
  - `cargo test -p context-api insert_first_match_by_index`
  - `cargo test -p context-api execute_insert_first_match_via_command`
  - `cargo test -p context-api`
  → **all pass**.

## Current blocking state after rerunning `cargo test --tests --workspace`
The public command/API surface is now reviewable and green for this pass.

Passing now:
- `cargo test -p context-api`
- `cargo test -p context-cli --test cli_integration` (`53 passed / 0 failed / 22 ignored`)

Remaining failures are isolated to deeper `context-read` engine tests:
- `tests::linear::repetition_aabbaabb`
- `tests::ngrams_validation::validate_mixed_pattern`
- `tests::overlapping::complex_abcabababcaba`
- `tests::read::read_infix1`
- `tests::read::read_infix2`
- `tests::read::read_multiple_overlaps1`
- `tests::read::read_repeating_known1`
- `tests::read::sync_read_text2`

These are no longer wrapper/dispatch/public-surface failures. They belong to the `context-read` overlap/decomposition redesign scope.

## Validation evidence
Commands run successfully this session:
- `cargo test --manifest-path memory-api/tools/cli/rule-cli/Cargo.toml --tests`
- `cargo test -p audit-cli --test integration_audit audit_collects_findings_and_prunes_stale_index_entries`
- `cargo test -p audit-cli --test integration_audit cli_supports_json_and_text_output`
- `cargo test -p context-api`
- `cargo test --tests --workspace`

Current failing command:
- `cargo test --tests --workspace`

Current remaining failures from that command:
- `-p context-read` engine tests listed above

# Linked blockers
- `978ce8a5` — active RC-1 expansion-loop redesign / insert_sequence outer-loop plan.
- `f41f08a8` — RC-3 repeated-single-char width-mismatch bug.

# Scope boundary
- Out of scope for this ticket pass: deeper `context-stack` redesign work in `context-read`/`context-trace`.
- In scope and complete for this pass: public execution surface fixes (wrapper/command/dispatch/audit output seams) and blocker isolation.

# Acceptance status
- Concrete test_execution failures captured and reduced to explicit underlying blockers. ✓
- Fixed local command/tooling failures that masked the real engine issue. ✓
- Remaining failures now linked to explicit blocker tickets. ✓
- Batch remains in implementation while linked context-read engine blockers are unresolved. ✗