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

## Round 02: Eliott's rulings on v1 (2026-11-23)

No new report; these rulings override v1 where they disagree.

| v1 question | Ruling |
|:--|:--|
| Scope | A **milestone of its own, before M7**, to sanitise Candler's surface. The gotchas, the chores and the HTN asks are inputs, not a backlog. For each one: is it Candler's responsibility, given why the repo exists (README: a domain-agnostic core for evaluating LLM agents, execution and evaluation behind one operation API)? What is its refactoring blast radius? Does it consolidate the triad (sustainable, scalable, efficient: `AGENTS.md`)? Stick to Candler's idioms; no melting pot of dangling features. Everything raised may still land, but the decision must be explicit. The triad applies at three positions: the development process, the codebase, and consumption. That is nine dimensions to optimise, with compromises where they conflict. They often reinforce each other: a good development cycle gives a good codebase, which gives good consumption. v1's axis (guessing removed from the agent) is the consumption position, valid but not the only one. |
| The container probe | Questioned. The container is a consequence of the experiment files and parameters, like a prepared well plate. An agent inspecting it is a bad usage pattern. If Candler can build a box that differs from what its files say, that is a Candler defect, caught by Candler's own guards and preflight (I8 already has `i8_canary_never_reaches_the_agent`). |
| MCP shape | Not decided: v1 did not explain it. It belongs to M7's planning, after the new milestone. The M7 leaf's gates (read-only tools, budgeted `run`/`evaluate`/`cancel`) are starting points, not fixed: planning sets the real shape. |
| Baseline | **Rejected.** No user-behaviour baseline: it cannot be controlled, and product studies come after the product is designed. The evidence is our own team of agents using Candler, plus engineering judgement. |
| Refusal hints | `codes.toml` would allow a `remedy` key (rows already gain `retired`, `lifted`), but hints are a rabbit hole: they go stale and wrong, and they can turn into workarounds that ask the caller to dodge Candler's bugs. Not to be added by default. 377 refusal codes (375 live; the largest sections are `internal` 85 and `schemas` 83) is a lot: their number and granularity are an input to the new milestone. |
| Docs before 1.0 | **Rejected**, and with it v1's "skills link the contract pages". Docs are the web-wide surface: they tell a visiting agent what Candler is and whether it fits its work, like a product specification. Skills and the MCP server are the interactive surfaces and must be self-contained: an agent in an isolated environment without internet must be able to use Candler. |
| F2, F4, F5 | Fold into the new milestone. |

## Status

M6b "Surface consolidation" is inserted before M7 by amendment A11 (`__roadmap__/candler-v1/…/m5b/stats/surface/m6b_surface_consolidation.md`). M6c follows it by amendment A12, renamed "Integrity" by amendment A13 (`__roadmap__/candler-v1/…/m5b/stats/surface/integrity/m6c_integrity.md`). Next: the M6c build (`__roadmap__/integrity/`).

**Deprecated (amendment A14, 2026-11-24).** This pre-study and its rulings are deprecated pending a re-evaluation of M7 after M6c Integrity, once its dogfooding pass has shown what an agent consuming Candler actually lacks. The M7 leaf is blocked until then.
