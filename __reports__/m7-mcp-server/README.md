# M7 MCP server: reports

## Round 00: pre-study (2026-11-23)

Before planning M7, a study of how well LLM agents can consume Candler today.

- `00-findings_agent_consumability_v0.md`: the briefing. It answers six questions (explain, patterns, gotchas, computable chores, speed, ease) and asks the steering questions.
- `00-inventory_docs_v0.md`: evidence: the consumer-readable documents and the CLI help.
- `00-inventory_surface_v0.md`: evidence: the operation table, hand-authored inputs, refusals.
- `00-inventory_gotchas_v0.md`: evidence: gotchas, usage patterns and chores mined from the reports and the HTN consumer.

The inventories were written by read-only subagents. Their line numbers come from semantic search and may be a few lines off.

## Round 01: HTN lane input (2026-11-23)

The HTN Phase coordinator, Candler's only consumer today, sent its lane's view at Eliott's request.

- `01-input_htn_lane_v0.md`: evidence: how HTN sessions use Candler, what they want computed, the form they prefer (condensed).
- `01-findings_agent_consumability_v1.md` (latest): the briefing, revised. It corrects v0 on current consumers, adds patterns P0, P8, P9, gotchas G18–G21, the probe and five other chores, and a section reconciling CLI, skill and MCP with the M7 gates.

## Status

The pre-study awaits Eliott's answers to the v1 steering questions. The M7 planning session (plan mode) starts after that.
