# Goal
Research and codify the repository policy for tracing instrumentation, runtime log capture, and managing generated logs plus executions through `log-api`.

## Why this work exists
Tracing, logs, journals, and execution evidence now span `context-*`, `memory-api`, `test-api`, `log-api`, and viewer surfaces. Existing tickets establish parts of the model, but contributors still lack one focused workflow that explains what to instrument, where logs live, how they are indexed, and how validation/runtime executions link back to the generated artifacts.

## Research questions
- What spans, events, result summaries, and correlation ids are the minimum required instrumentation for domain operations?
- Which generated artifacts belong in `log-api`, which belong in `test-api`, and which remain transient files or viewer-layer outputs?
- How should runtime sessions, validation runs, benchmark logs, and operation journals relate across stores?
- What additional `log-api` or tooling gaps block a full durable workflow today?
- Which current instructions should become canonical policy once the design is settled?

## Starting anchors
- existing completed ticket: `d3349747-b2f2-4dd4-b73c-dc016fec80d6` (`[log-api] Add runtime log session model and cross-store links`)
- active/root-store tickets: `73b2cd22-942b-4205-86e5-333df2373211` (`[memory-api] Shared tracing and log-api runtime diagnostics`), `2e41c96d-fe9f-4cf2-b941-6f0d452f237c` (`[memory-api] Create domain instrumentation and journaling coverage map`)
- architectural precedent: `84673399-75e6-4f36-8a17-4c666001e530` (`[observability] Resolve logging, journaling, and replay architecture boundaries`)
- repo anchor surfaced this session: `context-stack/context-api/src/log_parser.rs`

## Deliverables
- instrumentation policy by operation type and layer
- artifact ownership matrix for logs, journals, executions, and summaries
- dependency map for any missing `log-api` features or migrations
- follow-up specs and implementation tickets for the concrete gaps

## Validation expectations
The next session should finish with a draft policy outline tied to real crates/tickets plus an explicit blocker list for anything `log-api` cannot yet support.
