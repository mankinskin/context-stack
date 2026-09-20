# Session Objective
Resolve the stability compiler_warning finding. This is an aggregate metric finding representing ALL compiler warnings, not a single warning.

# Result: Mechanical Pass Complete (Option 1)
Baseline (native `cargo check --workspace`): **209 warnings**.
After mechanical `cargo fix` pass: **168 warnings**.

## Before / After by code
| code | before | after | notes |
|---|---|---|---|
| unused_mut | 29 | 0 | fixed via cargo fix |
| unused_variables | 6 | 0 | fixed via cargo fix |
| unused_imports | 8 | 2 | 6 fixed; 2 remaining in `context-read/src/tests/cursor.rs` are FALSE POSITIVES (imports `ToInsertCtx`/`ErrorState` are consumed inside `properties!`/`signature!` macro expansions — removing them breaks the build; confirmed and reverted) |
| dead_code | 165 | 165 | deferred to per-submodule child tickets (see residual plan) |
| unused_features | 1 | 1 | `#![feature(slice_index_methods)]` in context-insert/lib.rs — folded into context-stack child |

Commands run:
- `cargo fix -p context-insert -p context-read --lib --allow-dirty --allow-no-vcs`
- `cargo fix -p viewer-ctl -p log-viewer-dioxus -p ticket-viewer-dioxus -p spec-viewer-dioxus --bins --allow-dirty`
- `cargo fix -p viewer-api-dioxus --lib --allow-dirty`
- `cargo fix -p context-read --tests --allow-dirty`
Re-verified with `cargo check --workspace` (0 errors) and `cargo build -p viewer-ctl -p log-viewer`.

Note: workspace-wide `cargo fix --workspace` fails on feature-unification (spec-http `viewer_api` crate resolution; log-viewer `LogParser` re-export). Per-package fixes were used instead and are the correct approach here.

# Residual Plan (Option 3): dead_code triage children (blockers)
`dead_code` (165) requires per-item judgment (delete vs `#[allow(dead_code)]` for intentional frontend scaffolding), so it is split into per-submodule children linked as `depends_on` blockers:
- `40edd5d1` context-stack dead_code triage (~9 + 1 unused_features)
- `9c329f10` viewer-api dead_code triage (~109 — dominant, graph3d/data.rs=70)
- `cde503fd` memory-viewers dead_code triage (~38)
- `ef9eb7fb` log-viewer dead_code triage (~8)

Extra residual not in the 4 named buckets: 2 `dead_code` in the `memory-api` submodule (`ticket-http/.../edges.rs`, `ticket-cli/.../helpers.rs`) — track under stability follow-up if needed.

# Acceptance
- Mechanical warnings (unused_mut/imports/variables) resolved. ✓
- No increase in other categories; workspace still compiles. ✓
- Residual (dead_code) documented with explicit blocker tickets linked. ✓
- This ticket closes after the 4 dead_code children are done.