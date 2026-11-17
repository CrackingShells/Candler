# Agent token costs of the Candler campaign — Findings (v0)

Date: 2026-11-12

---
type: findings
topic: agent-token-costs
date: 2026-11-12
version: v0
prior-version: none
key-metric: API-equivalent cost of the Candler campaign: 1,346 USD (prior: N/A, delta: N/A)
decision-required: confirm
---

## Headline Result

metric: API-equivalent cost of every Candler session and subagent, 2026-10-28 to 2026-11-12 00:30 UTC
value: 1,346
unit: USD at list price (21,313 API calls, 12 sessions, 275 subagents)
prior: N/A (first run)
direction: new

**How to read the dollars.** They price each call's tokens at Anthropic's list prices:
- Opus 5.5: 4 USD/MTok input, 20 output, 0.20 cache read.
- Sonnet 5.5: 2 USD/MTok input, 10 output, 0.20 cache read.
- Cache writes: 1.25 × input for the 5-minute TTL, 2 × input for the 1-hour TTL.

The subscription's weekly quota may weight these differently. The shares are what to read: they don't depend on the price list.

## Results Tables

### Where the money went, by token category

| Category | MTok | USD | Share |
|---|---:|---:|---:|
| Cache reads | 3,607 | 722 | 54% |
| Output (with thinking) | 20.4 | 262 | 19% |
| Cache writes, 5 min (subagents) | 73 | 187 | 14% |
| Cache writes, 1 h (main sessions) | 22.3 | 178 | 13% |
| Uncached input | 0.05 | 0.1 | 0% |
| **Total** | | **1,346** | 100% |

### By session

| Session | USD | Share | Subagent share | Subagents |
|---|---:|---:|---:|---:|
| M2b execution parity | 292 | 22% | 65% | 60 |
| M3 lean CLI | 261 | 19% | 76% | 32 |
| M1 contracts | 207 | 15% | 66% | 31 |
| M2a evaluation parity | 191 | 14% | 52% | 34 |
| M2c wire alignment | 116 | 9% | 58% | 32 |
| Coordinator | 94 | 7% | 15% | 5 |
| M0 design (main checkout, 28 Oct) | 82 | 6% | 61% | 12 |
| M4 session runner (in progress) | 34 | 2% | 25% | 8 |
| Side: v1 review gaps | 30 | 2% | 47% | 12 |
| Side: E-tests (9 Nov) | 23 | 2% | 49% | 13 |
| Side: conformance docs | 17 | 1% | 14% | 2 |

### By role, all sessions

| Role | USD | Share | Agents | USD per agent (median, p90) | Calls per agent (median, p90) |
|---|---:|---:|---:|---|---|
| Main sessions (orchestrators) | 548 | 41% | 12 | — | — |
| Implementers | 555 | 41% | 96 | 4.2, 12.0 | 89, 193 |
| Verifiers | 152 | 11% | 81 | 1.1, 3.8 | 27, 65 |
| Drafters | 33 | 2% | 12 | 2.6, 5.4 | 40, 69 |
| Probes | 24 | 2% | 12 | 1.5, 3.5 | 37, 75 |
| Explorers | 16 | 1% | 16 | 0.8, 1.7 | 19, 36 |
| Planners | 13 | 1% | 13 | 0.8, 1.4 | 11, 29 |

Roles come from each subagent's description. A description starting "Implement" is an implementer. Otherwise the type is set by its first keyword: verify, review, check, draft, smoke and so on.

### Implementers and verifiers by milestone

| Milestone | Role | n | USD each | Rounds | Mean context (k) | Peak context (k) | Calls |
|---|---|---:|---:|---:|---:|---:|---:|
| M1 | implementer | 12 | 8.4 | 10.8 | 141 | 209 | 163 |
| M2a | implementer | 16 | 4.7 | 6.4 | 140 | 204 | 97 |
| M2b | implementer | 29 | 4.5 | 7.1 | 138 | 191 | 96 |
| M2c | implementer | 14 | 3.3 | 4.6 | 150 | 204 | 63 |
| M3 | implementer | 15 | 9.5 | 4.7 | 227 | 340 | 132 |
| M1 | verifier | 11 | 2.6 | 5.1 | 120 | 166 | 47 |
| M2b | verifier | 22 | 1.4 | 3.2 | 114 | 153 | 30 |
| M3 | verifier | 7 | 4.6 | 2.7 | 168 | 240 | 84 |

"Rounds" counts the user turns in a subagent's transcript: the dispatch plus each fix round or follow-up sent to it.

### Main sessions (orchestrators)

| Session | Calls | USD | USD per call | Mean context (k) | p90 context (k) | 1-h writes (MTok) | Output (MTok) |
|---|---:|---:|---:|---:|---:|---:|---:|
| M2b | 1,327 | 102 | 0.08 | 169 | 206 | 4.30 | 1.09 |
| M2a | 1,145 | 91 | 0.08 | 168 | 207 | 4.16 | 0.99 |
| Coordinator | 1,045+ | 79+ | 0.08 | 163 | 205 | 4.10 | 0.62 |
| M1 | 848 | 71 | 0.08 | 172 | 207 | 3.28 | 0.81 |
| M3 | 593 | 60 | 0.10 | 290 | 412 | 1.76 | 0.59 |
| M2c | 496 | 49 | 0.10 | 259 | 414 | 1.67 | 0.49 |
| M0 design | 291 | 32 | 0.11 | 313 | 532 | 1.00 | 0.29 |

### Since PR #17 opened (2026-11-11 15:03 UTC), the window behind "18 % of the week"

| Session | Main USD | Subagent USD | Total |
|---|---:|---:|---:|
| M3 (L1 to L3: 14 implementers, 8 verifiers) | 38 | 172 | 210 |
| Coordinator | 6 | 0 | 6 |
| M4 (planning, first leaves) | 2 | 0.3 | 2 |
| **Total** | 46 | 172 | **218** |

### Cache rewrites: calls that wrote more than 20k tokens after a session's first call

| Kind | Gap before the call | Count | Write USD |
|---|---|---:|---:|
| Main | under 5 min | 94 | 46.5 |
| Main | over 1 h | 36 | 32.8 |
| Subagent | 5–60 min | 133 | 47.7 |
| Subagent | under 5 min | 137 | 14.0 |

## Observations

| Signal | Baseline / Expected | Observed [source] | Interpretation |
|---|---|---|---|
| Coordination overhead | "Supervising myself would be cheaper" | The coordinator is 7% overall and 3% since #17 [source: calls.csv, ms=Coordinator] | Direct supervision would save little. The milestone sessions and their subagents would run anyway. |
| What drove the 18% week | Coordination cost | M3's implementation wave is 210 of the 218 USD since #17 [source: since-#17 table] | The spike is M3's work: 14 implementers and 8 verifiers in about 8 hours. |
| Implementer cost per leaf | 3.3–4.7 USD (M2a to M2c) | 9.5 USD in M3, 8.4 in M1 [source: implementer table] | M3: longer contexts (227k mean) and more calls (132). M1: the most fix rounds (10.8). |
| Context ceiling | ~200k, the plateau seen in M1 to M2c | M3 implementers peak at 340k on average and 465k at most [source: peak context] | M3's subagents ran on the 1M window without compacting at 200k. |
| Cost of context above 200k | Large | Reading the tokens above 200k cost 46 USD in subagents and 37 in main sessions, 6% in all [source: excess.py] | Capping context is a small lever. The number of calls matters more than their size. |
| Fixed prompt at every subagent call | Small | 61k tokens per general-purpose call, 14.9k calls: about 180 USD of reads plus 34 USD of writes at spawn [source: 00-findings_subagent_prefix_v0] | The largest single avoidable line. See the prefix report. |
| Cache expiry inside subagents | Rare | 133 rewrites after 5–60 min gaps, 48 USD [source: rewrite table] | Subagents idle past the 5-min TTL while they wait on long builds and tests. |
| Main-session output | Small next to reads | 109 USD (8%) [source: role × category] | Opus thinking in milestone sessions running at high effort. |

## Charts & Visualizations

Share of cost by role (one bar = 2%):

```
Main sessions  ████████████████████  41%  548 USD
Implementers   ████████████████████  41%  555 USD
Verifiers      █████▌                11%  152 USD
Drafters etc.  ███▌                   7%   91 USD
```

Implementer cost per leaf by milestone, in USD (one bar = 0.5 USD):

```
M1   █████████████████     8.4   (10.8 rounds)
M2a  █████████▍            4.7   ( 6.4 rounds)
M2b  █████████             4.5   ( 7.1 rounds)
M2c  ██████▋               3.3   ( 4.6 rounds)
M3   ███████████████████   9.5   ( 4.7 rounds, 227k mean context)
```

Cost by day, in USD (one bar = 10 USD; █ = main sessions, ░ = subagents):

```
10-28  ██░░░░░             72
10-29  █                    9
11-05  ████████░░░░░░░░░░░░░░    218
11-06  ████████████░░░░░░░░░░░░░░░░░░░░░    328
11-09  ██████████████░░░░░░░░░░░      246
11-10  ██████░░░░░░░░             139
11-11  ███████████░░░░░░░░░░░░░░░░░░░░░░    335
```

## Contradictions & Surprises

- Opus and Sonnet charge the same for a cache read, 0.20 USD/MTok. Moving orchestrators to Sonnet would only save on output and writes, about 140 USD over the campaign, not on the 54% that reads take.
- Uncached input is zero. Every call hit the cache, so the waste is in *what* gets re-read, not in cache misses.
- M3 ran fewer fix rounds than M1 (4.7 against 10.8) and still cost more per leaf: its leaves and contexts were bigger.

## Steering Questions

- [now] Pilot a restricted implementer definition (tools allowlist, no browser or artifact tools, no lists) on M4's next leaves? Expected saving: about 50k tokens per subagent call. See the architecture report.
- [now] Disable, for Candler, the MCP servers and plugins its agents never use (HPC, ArangoDB, Wolfram, calendar, Svelte, the GitHub connector)? That shrinks the lists in every session, main sessions included.
- [next run] Cap fix rounds at two by batching every verifier finding into one message? A third finding then starts a fresh implementer with a focused brief.
- [next run] Size leaves at drafting time to stay under about 100 implementer calls?
- [later] Lower the milestone sessions' effort during relay-only phases, and keep high effort for design rulings?

## Pointers

- Data: `/private/tmp/claude-501/-Users-hacker-Documents-tmp-claude-worktrees-Candler-candler-milestone-coordination-13808c/9af68a2d-3c86-4bc4-a1de-32450da8dbe2/scratchpad/usage/calls.csv` (one row per API call, deduplicated by message id)
- Scripts: `aggregate.py`, `analyse.py`, `extra.py`, `excess.py`, `prefix.py` in the same folder
- Transcripts: `~/.claude/projects/*Candler*/<session>.jsonl` and `<session>/subagents/agent-*.{jsonl,meta.json,prefix.json}`
- Prices: the `claude-api` skill, `shared/models.md` and `shared/prompt-caching.md`
- Companion reports: [00-findings_subagent_prefix_v0.md](00-findings_subagent_prefix_v0.md), [00-architecture_agent_system_v0.md](00-architecture_agent_system_v0.md)
