# Plan: Expansion loop redesign — cursor-advancing decomposition

## Session progress update (2026-07-06)
This session did **not** implement the full `context-read` expansion-loop redesign, but it cleared the public API and test-wrapper layers that were masking the deeper engine issue.

### What was fixed in this session
1. **`context-api::insert_first_match` exact token-ref semantics**
- Switched from read/next-match semantics to exact-root semantics.
- The command now exact-searches the resolved token pattern and either reuses the existing root or creates the exact composite with `insert_pattern(tokens)`.
- Result: `cargo test -p context-api` now passes, including:
  - `insert_first_match_by_index`
  - `execute_insert_first_match_via_command`

2. **`context-api::insert_sequence` public write path**
- Multi-character `insert_sequence` now reuses an exact existing atom-pattern root when present, otherwise constructs the exact root directly from auto-created/reused atom tokens.
- This removes the previous dependency on `ReadCtx::read_sequence` for the public write API.
- Result: public insert semantics now satisfy the current `context-cli` integration contract (`53 passed / 0 failed / 22 ignored`).

3. **`context-api::read_sequence` public read path**
- Multi-character `read_sequence` now delegates through the corrected public `insert_sequence` path and then reads back the resulting root.
- This eliminates the public `edge_repeated_single_char` panic even though the deeper `context-read` repeated-overlap engine bug remains.

### What was attempted locally in `context-read`
- Added anchor propagation in `context-read/src/pipeline/root.rs` so `append_pattern` / `append_token` set `self.anchor = self.root` after mutation.
- This was the smallest localized change from the AAA plan.
- Validation showed it was **not sufficient** to resolve the deeper overlap failures.

### Validation results
Passing after this session:
- `cargo test -p context-api`
- `cargo test -p context-cli --test cli_integration`
- Specifically repaired:
  - `integration::dedup_tests::dedup_atoms_not_duplicated`
  - `integration::edge_case_tests::edge_repeated_single_char`

Still failing deeper engine tests:
- `cargo test -p context-read read_infix1`
- `cargo test -p context-read repetition_aabbaabb`
- Full `cargo test --tests --workspace` still fails on `context-read` lib tests:
  - `tests::linear::repetition_aabbaabb`
  - `tests::ngrams_validation::validate_mixed_pattern`
  - `tests::overlapping::complex_abcabababcaba`
  - `tests::read::read_infix1`
  - `tests::read::read_infix2`
  - `tests::read::read_multiple_overlaps1`
  - `tests::read::read_repeating_known1`
  - `tests::read::sync_read_text2`

## Current diagnosis
The public API is now stable enough that the remaining failures are isolated to the deeper `context-read` overlap/decomposition engine, not the command wrappers.

The unresolved work is still consistent with the original plan:
- atom-anchor overlap detection remains missing in `ExpansionCtx`
- repeated / infix / overlap decomposition logic is still incomplete
- the localized anchor propagation fix alone does not restore the expected `[aa, a]` / infix / overlap behavior

## Review handoff
This ticket should remain **in implementation**. The current branch is reviewable for the public API changes and the narrowed failure surface, but the full plan acceptance is **not** yet met.

Next likely implementation slice:
- move from command-layer fixes into `context-read::ExpansionCtx` overlap handling (especially atom-anchor overlap), using the remaining eight `context-read` failures as the primary validation surface.