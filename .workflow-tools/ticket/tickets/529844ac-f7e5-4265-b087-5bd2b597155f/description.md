# Goal

Define and publish the cross-store correlation-id contract for observability artifacts so logs, journals, replay events, and transport spans can be joined deterministically.

## Scope

- Canonical identifiers: `operation_id`, `run_id`, `journal_id`, `session_id`.
- Propagation points: CLI/MCP/HTTP transport spans, ticket/move journal records, log session metadata, benchmark evidence records.
- Mapping to replay boundaries: identifiers are stable metadata, but profiling-only timing values remain outside deterministic replay payloads.

## Deliverables

1. Contract table for id semantics, producer, consumer, cardinality, and lifecycle.
2. Required-field matrix by subsystem (`memory-api`, `log-api`, `context-stack`, viewers).
3. Failure/absence behavior (missing id handling, fallback, and validation expectations).

## Implementation evidence

Published in owning observability spec:

- `.spec/specs/aa769a27-2721-4b9d-880c-5c4e2f8136a7/body.md`
  - Section: `Cross-store correlation-id contract`
  - Includes canonical id table (`operation_id`, `run_id`, `journal_id`, `session_id`)
  - Includes required-field matrix by subsystem/surface
  - Includes explicit failure/absence handling rules and replay-boundary constraints

Traceability links added in the same spec to:

- `.ticket/tickets/529844ac-f7e5-4265-b087-5bd2b597155f`
- `.ticket/tickets/ff6637f5-01f6-46c3-b727-e1a19ee0f202`

## Validation linkage to ff6637f5 checklist outputs

This ticket now provides contract evidence for these `ff6637f5` checklist items:

- Standardized run metadata includes required ids: covered by the canonical id contract and required-field matrix.
- Evidence links resolve across ticket/spec/journal artifacts: covered by added traceability links in `aa769a27` and id-to-surface mapping.
- Replay payload remains deterministic and excludes profiling-only timing fields while retaining correlation identifiers: covered by failure/absence handling and replay-boundary rule.

## Remaining follow-on work (outside this ticket)

- Runtime emission and query-path enforcement is implemented by downstream profiling/log/journal child tickets under `ff6637f5` and related lower-crate work.

## Depends-on context

Parent tracker: `84673399`.
Cross-tracker evidence target: `ff6637f5` validation checklist.