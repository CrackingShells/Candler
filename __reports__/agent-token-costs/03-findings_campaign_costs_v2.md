# Agent token costs of the Candler campaign, 13 to 17 November — Findings (v2)

Date: 2026-11-17

---
type: findings
topic: agent-token-costs
date: 2026-11-17
version: v2
prior-version: 02-findings_campaign_costs_v1.md
key-metric: API-equivalent cost of round 03's window: 274.7 USD (prior: 1,379.6 cumulative to round 02, corrected rates; delta: +274.7)
decision-required: confirm
---

## Headline Result

metric: API-equivalent cost of every Candler session and subagent from 2026-11-13 01:22 UTC to 2026-11-17 04:14 UTC
value: 274.7
unit: USD at list price (4,842 API calls, 13 sessions)
prior: 1,379.6 (rounds 00-02 re-priced with the corrected rates; round 02 published 1,678)
direction: new window; cumulative 1,654 USD

Two things need Eliott's confirmation. First, round 02's cache-read price for Sonnet 5.5 was twice the list price, which overstated rounds 00-02 by 298 USD (18%). Second, M5, M5b's planning and M5b as a whole all went over their budgets, and the window's main threads cost more than its subagents.

**Rates (one set everywhere).** From the `claude-api` skill, USD per MTok:

| Model | Input | Output | Cache write 5 min | Cache write 1 h | Cache read |
|---|---:|---:|---:|---:|---:|
| Opus 5.5 | 4 | 20 | 5 | 8 | 0.20 |
| Sonnet 5.5 | 2 | 10 | 2.5 | 4 | 0.10 |

The skill states input, output and cache read. The two cache-write columns are the standard 1.25× and 2× of input, as round 00 used them.

**Round 02's rate was partly wrong.** Its text gives Opus 5.5 cache read as 0.20, which is right. The `spend.py` script uses 0.40 for Opus, which is wrong. Round 02 priced Sonnet 5.5 cache reads at 0.20, where the list price is 0.10. I re-priced every transcript up to the round 02 cut and reproduced 1,678.2 with round 02's rates, so the method is the same. With the list rates:

| Rounds | Published | Corrected | Delta |
|---|---:|---:|---:|
| 00 (cut 2026-11-12 00:30 UTC) | 1,346 | 1,118 | -228 |
| 00-02 (cut 2026-11-13 01:22 UTC) | 1,678 | 1,380 | -298 |

`spend.py` (Opus 0.40) would give 1,996 for rounds 00-02. Its rates should not be used. The round 00-02 files are unchanged.

## Results Tables

### By group

Main = the session's own thread. Calls are main / subagent. Cache read % is the share of the group's USD.

| Group | USD | Share | Main USD | Subagent USD | Opus / Sonnet USD | Calls (main / sub) | Cache read % |
|---|---:|---:|---:|---:|---|---|---:|
| M5b (plan, level 0, level 1) | 83.3 | 30% | 49.5 | 33.8 | 49.5 / 33.8 | 723 / 1,018 | 42% |
| M5 / M5a (plan, close) | 64.0 | 23% | 31.1 | 32.9 | 31.1 / 32.9 | 363 / 907 | 53% |
| Milestone coordinator (`8776070d`) | 44.7 | 16% | 41.0 | 3.7 | 41.0 / 3.7 | 491 / 108 | 49% |
| Interlude audit (11-13, 03:27-04:45 UTC) | 44.5 | 16% | 5.4 | 39.1 | 44.0 / 0.5 | 55 / 604 | 55% |
| Driver work (trait, plan, dedup, C8) | 19.0 | 7% | 15.3 | 3.7 | 15.3 / 3.7 | 232 / 104 | 54% |
| Predecessor coordinator, outside the audit | 10.7 | 4% | 10.1 | 0.6 | 10.1 / 0.6 | 100 / 23 | 68% |
| Side chat, outside the audit | 6.6 | 2% | 6.6 | 0 | 6.6 / 0 | 37 / 0 | 32% |
| Roadmap amendments | 1.8 | 1% | 1.8 | 0 | 0 / 1.8 | 50 / 0 | 42% |
| Cost-report session (this one) | 0.1 | 0% | 0.1 | 0 | 0 / 0.1 (Haiku) | 27 / 0 | 48% |
| **Total** | **274.7** | | **160.9** | **113.8** | **197.6 / 77.0** | **2,078 / 2,764** | **49%** |

M5 has no separate level sessions in the transcripts: its implementation subagents sit under the planning session. Calls from the audit's window by the predecessor coordinator and the side chat are in the audit row, not in their own rows.

### By token category

| Category | USD | Share |
|---|---:|---:|
| Cache reads | 135.8 | 49% |
| Output (with thinking) | 56.7 | 21% |
| Cache writes, 1 h | 48.8 | 18% |
| Cache writes, 5 min | 33.4 | 12% |
| Uncached input | 0.0 | 0% |

### Against budget

| Work | Budget (USD) | Measured | Ratio |
|---|---:|---:|---:|
| M5 (plan and close) | 60 cap | 64.0 | 1.07× |
| M5 | 102 (M5a's reminder) | 64.0 | 0.63× |
| M5b planning (session `8cc176cd`) | 15 | 34.6 | 2.3× |
| M5b build (level 0 + level 1) | 52 | 48.7 | 0.94× |
| M5b total | 67 | 83.3 | 1.24× |
| Interlude audit | 50 | 44.5 | 0.89× |

The audit's own report states 44.3; the transcripts give 44.5 for every Candler call in its window.

### Long-lived main threads

| Session | Main calls | Mean context | Main USD | USD per call |
|---|---:|---:|---:|---:|
| Predecessor coordinator (`9af68a2d`) | 100 | 347k | 10.1 | 0.101 |
| M5 planning (`23dcb7cb`) | 257 | 274k | 24.9 | 0.097 |
| Coordinator (`8776070d`) | 491 | 213k | 41.0 | 0.083 |
| M5b planning (`8cc176cd`) | 356 | 163k | 26.5 | 0.074 |
| M5b level 1 (`0a3e9c89`) | 249 | 155k | 15.7 | 0.063 |
| M5b level 0 (`c198db0e`) | 118 | 148k | 7.3 | 0.062 |
| M5a close (`53e2340c`) | 106 | 171k | 6.3 | 0.059 |

## Observations

| Signal | Baseline / Expected | Observed | Interpretation |
|---|---|---|---|
| Main-thread share | 38% in round 02 (orchestrators, 645 of 1,678) | 59% (160.9 of 274.7) [source: by-group table] | The round 02 share used the overstated Sonnet reads, which fall mostly on subagents. Even so, the window is main-thread heavy |
| Cost of one call | Subagent call | Main call 0.077 USD, subagent call 0.041 [source: calls above] | A main call carries 150-350k of context, and most of it is billed as cache reads each turn |
| Coordinator overhead | Suspected main driver | Coordinator `8776070d` is 16% (44.7 USD); with its predecessor and the side chat, 62 USD or 22% [source: by-group table] | Real but not the largest. Planning sessions are as costly: M5 planning main 24.9 and M5b planning main 26.5, together more than the coordinator's 41.0 |
| Planning cost | 15 USD for M5b planning | 34.6 USD, 356 main calls to 163k mean context [source: long-lived table] | The planning session of M5b was the largest overrun, 19.6 USD of M5b's 16.3 USD total overrun (the build finished 3.3 under) |
| M5 budget | 60 USD cap | 64.0 USD, 0.63× of the 102 in M5a's reminder | The cap was missed by 4 USD; the 102 figure was not near |
| Subagent-heavy work | Audit and M5 spent in subagents | Audit 88% subagents, M5 51%, M5b 41% [source: by-group table] | The audit is the one case where parallel Opus subagents were the cost, 39.1 USD, and the budget held |
| Model mix | Sonnet for subagents | Sonnet is 28% of USD (77.0), Opus 72% [source: by-model] | Sonnet carries M5 and M5b subagents. Opus carries every main thread and the audit's subagents |
| Cache reads | 56% in round 02 | 49%, 42% in M5b [source: category table] | Lower at corrected Sonnet rates, and lower where subagents are Sonnet |

## Charts & Visualizations

Cost by group, in USD (one bar = 3 USD):

```
M5b                █████████████████████████████▎  83.3
M5 / M5a           █████████████████████▎          64.0
Coordinator        ██████████████▉                 44.7
Interlude audit    ██████████████▊                 44.5
Driver work        ██████▎                         19.0
Predecessor coord  ███▌                            10.7
Side chat          ██▏                              6.6
Roadmap amend.     ▌                                1.8
```

Main thread against subagents, in USD (one bar = 3 USD):

```
M5b               main █████████████████ 49.5   sub ███████████▎ 33.8
M5 / M5a          main ██████████▎ 31.1         sub ███████████ 32.9
Coordinator       main █████████████▋ 41.0      sub █▎ 3.7
Interlude audit   main █▊ 5.4                   sub █████████████ 39.1
Driver work       main █████ 15.3               sub █▎ 3.7
```

Cost by day, in USD (one bar = 5 USD):

```
11-13  ██████████████████▋  93.4
11-14  ▎                     1.5
11-15  ▌                     3.1
11-16  ██████████████████████████████▌  153.0
11-17  ████▊                 23.8 (to 04:14 UTC)
```

## Contradictions & Surprises

- The one correction that mattered came from the subagent side: Sonnet reads at 0.20 instead of 0.10 added 298 USD to a campaign whose subagents are mostly Sonnet. The rule "cache read is 0.1× of input" holds for Sonnet 5.5 (0.10) and Opus 5.5 (0.20 against 4 input is 0.05×, not 0.1×).
- M5b's build finished under its budget (48.7 against 52) while its planning ran 2.3× over. The plan-to-build split in the budget did not match where the work went.
- M5b's level 1 session cost 33.0 USD with its subagents, the same order as M4's big leaves (31-49 USD), at a lower main-thread context (155k).

## Steering Questions

- [now] Accept the corrected rates (Opus 5.5 read 0.20, Sonnet 5.5 read 0.10) for every later round, and treat 1,380 USD as the round 02 total? `spend.py` should be fixed before it is used again.
- [now] Cap planning sessions, since M5b's planning cost 34.6 against 15 and M5's planning session ran 24.9 in its main thread? A call ceiling per planning session would be the lever.
- [next run] Hand the coordinator's long context to a fresh session after each milestone merge? At 213k mean context a main call costs about twice a subagent call. The data supports a restart rule but not a number; it needs a per-session replay like the one in the context-window report.
- [later] Budget M5b-style milestones as plan + build with the plan share set from this data, about 40% of the total, not 22%.

## Pointers

- Script and output: [scripts/round03_measure.py](scripts/round03_measure.py), [scripts/round03_output.txt](scripts/round03_output.txt); run with `python3 -I`
- Source: the 16 `~/.claude/projects/*Candler*` directories, each main `<id>.jsonl` plus `<id>/subagents/*.jsonl`, `message.id` counted once, window by record timestamp
- Interlude cross-check: `__reports__/interlude-audit/01-results_v0.md` (44.3 USD)
- Prior version: [02-findings_campaign_costs_v1.md](02-findings_campaign_costs_v1.md). Rate source: the `claude-api` skill model table, 2026-11-03 cache
