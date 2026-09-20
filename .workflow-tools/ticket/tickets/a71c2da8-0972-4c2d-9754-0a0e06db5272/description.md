# Goal
Research and codify the repository policy for tracing instrumentation, runtime log capture, and managing generated logs plus executions through `log-api` without collapsing distinct stores into one ownership bucket.

## Why this work exists
Tracing, logs, journals, sessions, and execution evidence now span `context-*`, `memory-api`, `test-api`, `log-api`, and viewer surfaces. The repository needs one policy that explains what each store must know atomically on its own, then separately explains how realistic workflows query and compose those stores together.

## Research questions
- What spans, events, result summaries, and correlation ids are the minimum required instrumentation for domain operations?
- Which facts must each durable store answer atomically without consulting other stores?
- Which cross-store workflows need shared correlation or helper APIs to compose runtime sessions, validation runs, benchmarks, and journals?
- What additional `log-api` or tooling gaps block a full durable workflow today?
- Which current instructions should become canonical policy once the design is settled?

## Starting anchors
- existing completed ticket: `d3349747-b2f2-4dd4-b73c-dc016fec80d6` (`[log-api] Add runtime log session model and cross-store links`)
- active/root-store tickets: `73b2cd22-942b-4205-86e5-333df2373211` (`[memory-api] Shared tracing and log-api runtime diagnostics`), `2e41c96d-fe9f-4cf2-b941-6f0d452f237c` (`[memory-api] Create domain instrumentation and journaling coverage map`)
- architectural precedent: `84673399-75e6-4f36-8a17-4c666001e530` (`[observability] Resolve logging, journaling, and replay architecture boundaries`)
- repo anchor surfaced this session: `context-stack/context-api/src/log_parser.rs`

## Deliverables
- instrumentation policy by operation type and layer
- atomic knowledge ownership matrix for logs, journals, executions, sessions, and summaries
- cross-store workflow/query map showing how those atomic stores compose
- dependency map for any missing `log-api` features or migrations
- follow-up specs and implementation tickets for the concrete gaps

## Validation expectations
The draft policy now exists in this ticket. The next session should validate or implement against it: keep the blocker list current, wire the layered interoperability contract through `db9bad13-ae43-4300-8037-7165c0e9a7b0`, and avoid reopening pre-policy discovery unless the durable artifacts conflict with code reality.

## Atomic knowledge ownership matrix (2026-07-10)

| Artifact class | Atomic owner | Facts that must be queryable without consulting other stores | Cross-store workflow relationship | Current anchor |
| --- | --- | --- | --- | --- |
| Validation specs and executions | `test-api` | pass/fail outcome, duration, throughput, command identity, run grouping, validation provenance, and compliance links recorded on the execution itself | joins to runtime logs or journals when a run emitted external artifacts | `memory-api/crates/test-api/src/lib.rs`, `memory-api/crates/test-api/src/store_index.rs` |
| Benchmark numeric results and budget status | `test-api` | per-operation metrics, budget status, run grouping, benchmark provenance, and compliance links | may reference runtime logs, profiler outputs, or session lineage when performance evidence needs them | `memory-api/crates/test-api/src/benchmark.rs` |
| Runtime log sessions and log captures | `log-api` | session metadata, capture ids, locators, lifecycle status, and searchable log-specific indexing | queried together with test or journal records through shared run or session correlation, not by moving ownership into `test-api` | ticket `d3349747-b2f2-4dd4-b73c-dc016fec80d6` |
| Deterministic operation journals and resume or rollback ledgers | journal contract or domain journal store | authoritative mutation history, replay-safe inputs and outputs, rollback lineage, and operation-specific journal status | linked to tests or logs for diagnosis, but not replaced by them | tracker `73b2cd22-942b-4205-86e5-333df2373211`, planned journal work `6c859ac3` |
| Runtime sessions and transcript-style domain artifacts | owning session or domain store | session identity, lifecycle, captured artifact manifest, and domain session semantics | may aggregate links to tests, logs, and journals while leaving those stores authoritative for their own facts | existing session and runtime-session work |
| Human-readable summaries and closure notes | ticket, spec, or doc surfaces | narrative conclusions, reviewer guidance, and handoff status | summarizes evidence from the durable stores rather than owning the evidence itself | root workflow-policy tracker plus downstream docs and specs |

## Cross-store workflow model

- Ownership means atomic queryability of the owned fact set. A tool should be able to ask one store for the facts that store owns without making hidden secondary reads.
- Workflow composition is a separate concern. Multi-store flows should join executions, runtime sessions, benchmark outputs, and journals through explicit correlation ids and durable links.
- Domain-specific artifacts must be explicit. Runtime sessions, benchmark bundles, profiler outputs, replay journals, and viewer-generated summaries are not interchangeable and should not be forced into one flat artifact class.

## Blocker map

- `d3349747` landed runtime log-session storage, but live indexing and active-log search remain blocked on `aa94d02e`.
- The generic journal envelope is still open under `6c859ac3`, so journal ownership is clear conceptually but not yet normalized across non-move operations.
- Consistent correlation ids across CLI, MCP, and HTTP remain blocked on `3041d7e3` and the shared tracing bootstrap work `756fed27`.
- The repo still lacks one concrete implementation slice that normalizes the shared minimum link set plus artifact-specific extensions between `test-api` executions, `log-api` runtime sessions, and journal ids.

## Concrete follow-up created

- Follow-up implementation ticket: `db9bad13-ae43-4300-8037-7165c0e9a7b0` (`[log-api][test-api][journal] Normalize artifact routing for executions, runtime sessions, and journals`).
- This ticket exists because the current design anchors describe correlation and ownership, but no concrete owner yet exists for layered compatibility rules, helper APIs, required-link validation, and runner-side evidence routing.

## Current next step

- Policy drafting is complete enough for implementation decomposition. The active remaining gap is to implement and validate the shared minimum link set plus artifact-specific extensions under `db9bad13-ae43-4300-8037-7165c0e9a7b0`.

## Focused health note (2026-07-10)

- The remaining dependency-state warning is intentional: this policy ticket stays `on-hold` while `db9bad13-ae43-4300-8037-7165c0e9a7b0` carries the active implementation slice for layered interoperability.
