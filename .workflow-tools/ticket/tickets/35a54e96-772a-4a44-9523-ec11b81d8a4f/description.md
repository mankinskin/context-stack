## Objective
Add compile-fail coverage for `context-stack/context-trace-macros`.

## Context
Audit `tmp/test-coverage-audit/01b-coverage-verification.md`: context-trace-macros has 659 LOC, zero tests, and is the only genuinely zero-test crate.

## Acceptance criteria
- Cover supported and rejected macro invocations with compile-fail tests.
- Preserve diagnostic expectations at meaningful boundaries.

## Out of scope
- Macro feature redesign.