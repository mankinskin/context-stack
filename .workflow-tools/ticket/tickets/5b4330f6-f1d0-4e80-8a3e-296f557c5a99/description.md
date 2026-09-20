# Add context-trace JSON format compatibility test against log-viewer parser

## Problem

`crates/context-stack/context-trace` uses a `PrettyJsonWriter` to produce structured log output, and `crates/context-stack/context-api/src/log_parser.rs` parses it. There are no automated tests ensuring the writer's output stays parseable as the two crates evolve independently. Any field rename or format change in `context-trace` can silently break the log-viewer without a failing test.

## Scope

Add an integration test (or a dedicated test module) that:

1. Runs a minimal context-trace operation (e.g. a single `insert_sequence` + `search_sequence` on a test graph) with tracing enabled and writing to a temp file.
2. Passes that file to `crates/context-stack/context-api::log_parser::parse_log_file()`.
3. Asserts:
   - At least one `LogEntry` is returned.
   - Expected fields are present (`level`, `timestamp`, `target`, `fields.message` or equivalent).
   - No parse errors for any line.
4. Runs in `cargo test -p context-api` (or a suitable crate that can depend on both).

## Acceptance Criteria

- Test passes on `main`.
- Test fails (and guards the boundary) if `context-trace` changes its JSON schema in a breaking way.
- Test is documented in `CHEAT_SHEET.md` under a "Log format stability" section.

## Files

- New: `crates/context-stack/context-api/tests/log_parser_compat.rs` (or similar)
- Read: `crates/context-stack/context-api/src/log_parser.rs`
- Read: `crates/context-stack/context-trace/src/graph/visualization.rs`

## Depends on

- [LOG-2a] (field names should be normalised before this test is pinned)
