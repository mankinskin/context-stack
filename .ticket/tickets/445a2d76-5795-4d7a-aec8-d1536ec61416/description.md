## Summary

Implement "context stack price awareness" so that an agent automatically switches into an **orchestrator mode** based on the cost of its own underlying model. In orchestrator mode the agent delegates most work — tasks, tool-call batches, and file read/edit — to cheaper sub-agents via `runSubagent`, preserving expensive-model context and reducing token spend while keeping quality for high-value reasoning.

Source: transformed from the design transcript at [transcripts/25-07-2026_context-price-awareness/input.clean.md](../../transcripts/25-07-2026_context-price-awareness/input.clean.md).

## Motivation

Context costs are too high because we lean on expensive models for their output quality and decision-making. We want to keep expensive models for the work that matters most:

- Strategic decisions.
- Developing new code and planning code changes.
- Planning tool calls.

At the same time, we want to protect the context window of large/expensive models to keep costs low by pushing routine execution down to cheaper sub-agents.

## Scope / Deliverables

1. **Model → cost mapping.** Machine-readable mapping from models to per-token cost, sourced from [tools/model-prices/model_prices.json](../../tools/model-prices/model_prices.json) (synced by [tools/model-prices/sync_model_prices.py](../../tools/model-prices/sync_model_prices.py)). Per-1M-token USD fields: `input_mtok`, `output_mtok`, `cache_read_mtok`, `cache_write_mtok`, plus `context_window`, `deprecated`.
2. **AGENTS.md orchestrator instruction.** Extended the "Model cost awareness & routing" guidance with an explicit, enforceable rule keyed to `output_mtok > X`.
3. **Guidance surface.** The AGENTS.md Token-Efficient Output section now carries the orchestrator-threshold rule referencing the mapping.

## Resolved Decisions (2026-07-25)

- **Driving field:** `output_mtok` (`out$/M`, USD per 1M output tokens). Not blended/input-based.
- **Threshold `X`:** `15` USD per 1M output tokens (= `1500` credits/1M at 100 credits = $1). Fires when `output_mtok > X`.
- **Effect:** Opus-tier (25–75) and o3 (40) orchestrate; Sonnet (15), GPT-5 (10), Gemini Pro (10), Haiku, Flash, mini execute directly.
- **Mapping location:** shared `model_prices.json`; AGENTS.md references it, no per-tool hardcoding.

## Acceptance Criteria

- [x] Documented model→cost mapping derivable from `model_prices.json` and referenced by the guidance.
- [x] AGENTS.md contains an explicit orchestrator-mode rule keyed to threshold `X` on a named price field (`output_mtok`).
- [x] Guidance states which work stays on the expensive model vs. is delegated.
- [x] Threshold `X` and the driving field recorded as concrete values (ticket + transcript).

## Implementation Notes

- AGENTS.md "Model cost awareness & routing" bullet extended with two new bullets: the enforceable threshold rule and the model→cost mapping source-of-truth.
- Transcript Open Questions updated with a Resolved Decisions block.

## Verification Note (2026-07-27)

Verified 2026-07-27: all 4 ACs met in current tree; orchestrator-threshold rule migrated from AGENTS.md into .agents/instructions/orchestration/orchestrator-delegation.instructions.md (reachable via AGENTS.md delegation pointer). Closed on verification, no code/doc change required.

Evidence:
- AC1 (model→cost mapping referenced): tools/model-prices/model_prices.json has output_mtok; referenced at .agents/instructions/orchestration/orchestrator-delegation.instructions.md (Source of truth section).
- AC2 (orchestrator rule keyed to output_mtok > X): orchestrator-delegation.instructions.md L8 "output_mtok strictly exceeds threshold X = 15"; L13 names output_mtok as driving field.
- AC3 (keep-vs-delegate work split): orchestrator-delegation.instructions.md L39-L58 lists keep-on-expensive vs delegate work; L35 tier table.
- AC4 (concrete X + field recorded): X = 15 and output_mtok stated; tools/model-prices/cost_gate.py DEFAULT_THRESHOLD_X = 15.0 enforces in code.