# Agent token costs of the Candler campaign, with M4 complete — Findings (v1)

Date: 2026-11-13

---
type: findings
topic: agent-token-costs
date: 2026-11-13
version: v1
prior-version: 00-findings_campaign_costs_v0.md
key-metric: API-equivalent cost of the Candler campaign: 1,678 USD (prior: 1,346, delta: +332)
decision-required: none
---

## Headline Result

metric: API-equivalent cost of every Candler session and subagent, 2026-10-28 to 2026-11-13 01:22 UTC
value: 1,678
unit: USD at list price (25,894 API calls, 12 sessions, 279 subagents)
prior: 1,346 (round 00, cut at 2026-11-12 00:30 UTC)
direction: +332 USD, of which M4 is 299 and the coordinator 31

M4 is the most expensive milestone: 332 USD, 20% of the campaign. Most of it went to its L1–L3 implementation wave on the night of 11 to 12 November, 244 USD in about 11 hours. Its leaves after the knowledge-transfer rules were written cost about a quarter as much per leaf, but those were also smaller leaves. The second night, which ran the PR cycle, cost 34 USD.

Prices are unchanged from round 00:
- Opus 5.5: 4 USD/MTok input, 20 output, 0.20 cache read.
- Sonnet 5.5: 2, 10 and 0.20.
- Cache writes: 1.25× input for 5 minutes, 2× input for 1 hour.

The shares don't depend on the price list.

**Correction to round 00.** The role classifier filed some descriptions under the wrong role:
- "L5 live_confirmation implementer" went to verifiers, because the description contains "confirm";
- "F1 opencode_probe_home implementer" went to probes;
- "Verify …" descriptions that also mentioned "probe" went to probes.

A description that names "implementer", or starts with Implement, Fix round or Build, is now an implementer. One that starts with Verify, Review, Grit review, Rust conventions or Spec-verify is a verifier. This moved 15 USD between roles across the whole campaign, and every table below uses the corrected roles.

## Results Tables

### Where the money went, by token category

| Category | MTok | USD | Share | Round 00 share |
|---|---:|---:|---:|---:|
| Cache reads | 4,699 | 940 | 56% | 54% |
| Output (with thinking) | 24.0 | 303 | 18% | 19% |
| Cache writes, 5 min (subagents) | 89 | 227 | 14% | 14% |
| Cache writes, 1 h (main sessions) | 25.9 | 208 | 12% | 13% |
| Uncached input | 0.1 | 0.1 | 0% | 0% |
| **Total** | | **1,678** | 100% | |

### By session

| Session | USD | Share | Subagent share | Agents | Added since round 00 |
|---|---:|---:|---:|---:|---:|
| **M4 session runner (complete)** | **332** | **20%** | 75% | 48 | 299 |
| M2b execution parity | 292 | 17% | 65% | 67 | 0 |
| M3 lean CLI | 263 | 16% | 76% | 33 | 1 |
| M1 contracts | 207 | 12% | 66% | 31 | 0 |
| M2a evaluation parity | 191 | 11% | 52% | 34 | 0 |
| Coordinator | 125 | 7% | 11% | 7 | 31 |
| M2c wire alignment | 116 | 7% | 58% | 32 | 0 |
| M0 design | 82 | 5% | 61% | 12 | 0 |
| Side sessions (3) | 71 | 4% | 40% | 27 | 0 |

### By role, all sessions

| Role | USD | Share | Agents | USD per agent (mean, median, p90) |
|---|---:|---:|---:|---|
| Implementers | 748 | 45% | 118 | 6.3, 4.4, 13.5 |
| Main sessions (orchestrators) | 645 | 38% | 12 | 54 per session |
| Verifiers | 198 | 12% | 110 | 1.8, 1.0, 4.1 |
| Drafters, probes, explorers, planners, other | 87 | 5% | 51 | — |

### Implementers and verifiers by milestone

| Milestone | Role | n | USD each (mean / median) | Rounds | Mean context (k) | Peak context (k) | Calls |
|---|---|---:|---|---:|---:|---:|---:|
| M1 | implementer | 12 | 8.4 / 7.0 | 8.3 | 141 | 209 | 163 |
| M2a | implementer | 16 | 4.7 / 4.1 | 5.5 | 140 | 204 | 97 |
| M2b | implementer | 30 | 4.4 / 3.6 | 5.5 | 136 | 189 | 93 |
| M2c | implementer | 14 | 3.3 / 2.4 | 4.6 | 150 | 204 | 63 |
| M3 | implementer | 15 | 9.6 / 10.0 | 4.5 | 228 | 340 | 133 |
| **M4** | implementer | 20 | **10.1 / 5.4** | 4.8 | 192 | 290 | 152 |
| M1 | verifier | 13 | 2.3 / 1.3 | 3.9 | 116 | 160 | 40 |
| M2b | verifier | 21 | 1.5 / 1.2 | 3.0 | 115 | 154 | 30 |
| M3 | verifier | 8 | 4.3 / 3.7 | 2.9 | 159 | 226 | 78 |
| **M4** | verifier | 23 | **1.7 / 0.5** | 2.5 | 116 | 151 | 32 |

"Rounds" counts the user turns in a subagent's transcript: the dispatch plus each fix round or follow-up sent to it. The round-00 rounds counted compaction summaries as turns; this table doesn't, so M1 drops from 10.8 to 8.3.

### M4 by leaf (USD)

| Phase | Leaf | Implementer | Verifier | Total | Agents |
|---|---|---:|---:|---:|---:|
| L0 | session_facts (probe) | 7.4 | — | 7.4 | 1 |
| L0 | imported_strata | 2.2 | 0.8 | 3.0 | 3 |
| L0 | type_merges | 0.8 | — | 0.8 | 1 |
| L0 | opencode_facts (probe) | 4.6 | — | 4.6 | 1 |
| L1 | driver_traits | 12.8 | 4.2 | 17.0 | 3 |
| L2 | harness_out_of_core (one fix round, 12.2) | 26.0 | 5.0 | 31.0 | 3 |
| L2 | opencode_driver | 13.5 | 2.2 | 15.7 | 3 |
| L2 | **opencode_settings** | **41.4** | 7.6 | **49.0** | 4 |
| L3 | **session_runner** | **30.0** | 8.5 | **38.6** | 3 |
| L3 | **opencode_wiring** | **33.2** | 4.1 | **37.3** | 2 |
| L4 | session_controls | 12.0 | 1.6 | 13.6 | 3 |
| L4 | contract_docs | 5.4 | 1.1 | 6.6 | 2 |
| L4 | design_record | 2.7 | 1.5 | 4.2 | 2 |
| L4 | harness_guard | 0.6 | 0.5 | 1.0 | 2 |
| L5 | digest_invariance | 2.0 | 1.0 | 3.0 | 2 |
| L5 | live_confirmation (RiVault spend not included) | 5.3 | — | 5.3 | 1 |
| L5 fix | env_tag_base (F2) | 3.2 | 0.3 | 3.5 | 2 |
| L5 fix | opencode_probe_home (F1) | 2.2 | 0.3 | 2.5 | 2 |
| — | planning: surveys, plan attack, docs fetch | — | — | 5.9 | 7 |
| — | M4 main session | — | — | 82.6 | 1 |

Five implementers compacted, each one or two times at about 468k tokens:
- opencode_settings, opencode_wiring, session_runner, harness_out_of_core and driver_traits;
- together they are 52% of M4's subagent spend.

### M4 by phase (USD)

| Phase | Main | Subagents | Total |
|---|---:|---:|---:|
| Plan (11 Nov) | 14.0 | 4.7 | 18.7 |
| L0–L3 (overnight 11–12 Nov) | 38.6 | 205.4 | **243.9** |
| L4 | 10.9 | 25.3 | 36.2 |
| L5, live runs, F1 and F2 | 12.1 | 14.3 | 26.4 |
| PR, CI fix, close-out | 7.1 | 0 | 7.1 |

### Main sessions (orchestrators)

| Session | Calls | USD | USD per call | Mean context (k) | p90 context (k) | 1-h writes (MTok) | Output (MTok) |
|---|---:|---:|---:|---:|---:|---:|---:|
| Coordinator | 1,296 | 111 | 0.09 | 182 | 230 | 6.08 | 0.81 |
| M2b | 1,327 | 102 | 0.08 | 169 | 206 | 4.30 | 1.09 |
| M2a | 1,145 | 91 | 0.08 | 168 | 207 | 4.16 | 0.99 |
| **M4** | 877 | 83 | 0.09 | 281 | 413 | 2.40 | 0.73 |
| M1 | 848 | 71 | 0.08 | 172 | 207 | 3.28 | 0.81 |
| M3 | 635 | 64 | 0.10 | 297 | 421 | 1.82 | 0.62 |
| M2c | 496 | 49 | 0.10 | 259 | 414 | 1.67 | 0.49 |
| M0 design | 291 | 32 | 0.11 | 313 | 532 | 1.00 | 0.29 |

### The second overnight (2026-11-12 16:09 to 2026-11-13 01:22 UTC): the M4 PR cycle

| Session | Main USD | Subagent USD | Total |
|---|---:|---:|---:|
| M4: live runs, F1 and F2, PR, CI fix, close-out | 16.3 | 10.0 | 26.3 |
| Coordinator: rulings, PR review, CI | 6.5 | 0.9 | 7.4 |
| **Total** | 22.8 | 10.9 | **33.7** |

The four RiVault runs are not in these figures. They cost 0.19 USD by Claude Code's own count, on the institute's endpoint.

### The coordinator by window

| Window | Main USD | Subagent USD | Mean context (k) |
|---|---:|---:|---:|
| Before PR #17 (5–11 Nov) | 73.1 | 13.4 | 163 |
| #17 to the round-00 cutoff | 7.3 | 0.0 | 180 |
| Cost reports and the day of 12 Nov | 23.8 | 0 | 327 |
| Second overnight (M4 PR cycle) | 6.5 | 0.9 | 175 |

### Cost against code landed

| Milestone | USD | Lines added | Rust lines added (no fixtures) | of which tests | USD per 1k Rust lines |
|---|---:|---:|---:|---:|---:|
| M1 | 207 | 65,656 | 32,829 | 12,793 | 6.3 |
| M2a | 191 | 24,084 | 10,625 | 4,514 | 17.9 |
| M2b | 292 | 36,528 | 29,228 | 10,972 | 10.0 |
| M2c | 116 | 11,078 | 7,706 | 5,528 | 15.1 |
| M3 | 263 | 48,941 | 27,786 | 15,811 | 9.5 |
| M4 | 332 | 30,185 | 21,490 | 12,215 | 15.5 |

Lines come from each milestone PR's merge, or from main to PR #20's head for M4. This is a crude measure: M2a and M2c were mostly reshaping existing code, and M1 imported a large corpus.

## Observations

| Signal | Baseline / Expected | Observed [source] | Interpretation |
|---|---|---|---|
| M4's total | About M3's 263 | 332, the campaign's highest [source: by-session table] | Two crates of new surface (driver traits, the OpenCode driver) plus session mode, the largest scope since M2b |
| Where M4 spent | Spread over 18 leaves | Four L2–L3 leaves are 163 USD, 49% of M4 [source: per-leaf table] | Big leaves are where cost concentrates. Each ran 400–580 calls and compacted at 468k |
| Leaves after the knowledge transfer | Same per-leaf cost | L4–L5 leaves average 5.0 USD (8 leaves), against 20.4 for L0–L3 (10 leaves) [source: per-leaf table] | Consistent with the M4 rules: at most two fix rounds, re-checks sized to the diff. It is confounded with leaf size, since the later leaves were smaller, so no causal claim is possible |
| Verifier cost | 1.5–4.3 USD each (M1–M3) | 0.5 USD median for M4, 2.5 rounds [source: implementer/verifier table] | M4's verifiers checked a narrow diff and reported once. Their total is 20% of implementer spend, against 23% in M3 |
| Fix rounds | 2–3 per leaf | One counted fix round, harness_out_of_core at 12.2 USD [source: per-leaf table] | The two-round cap held. The fix round cost as much as the first implementation of that leaf |
| Overnight PR cycle | Comparable to the build | 34 USD for nine hours: 2 fixes, CI and a PR [source: overnight table] | Review and ruling are cheap. The money is in implementation waves |
| Coordinator share | 7% | Still 7%, 125 USD [source: by-session table] | Unchanged. Its 24 USD on 12 Nov were the cost reports themselves, at 327k mean context |
| Cache reads | 54% | 56% [source: category table] | The 500k-window era keeps raising the read share. See the context-window report v1 |

## Charts & Visualizations

Cost by milestone, in USD (one bar = 10 USD):

```
M4     █████████████████████████████████  332
M2b    █████████████████████████████      292
M3     ██████████████████████████         263
M1     █████████████████████              207
M2a    ███████████████████                191
Coord  ████████████▌                      125
M2c    ███████████▌                       116
M0     ████████                            82
Side   ███████                             71
```

M4 per-leaf total, in USD (one bar = 2 USD):

```
opencode_settings     ████████████████████████▌  49.0
session_runner        ███████████████████▎       38.6
opencode_wiring       ██████████████████▋        37.3
harness_out_of_core   ███████████████▌           31.0
driver_traits         ████████▌                  17.0
opencode_driver       ███████▉                   15.7
session_controls      ██████▊                    13.6
session_facts         ███▋                        7.4
contract_docs         ███▎                        6.6
the other 9 leaves    ██████████▏                20.3
```

Cost by day, in USD (one bar = 10 USD):

```
10-28  ███████▎   72
10-29  ▉           9
11-05  █████████████████████▊   218
11-06  ████████████████████████████████▊   328
11-09  ████████████████████████▌   246
11-10  █████████████▉   139
11-11  █████████████████████████████████▍   335
11-12  ████████████████████████████████▎   323
11-13  ▊   8 (to 01:22 UTC)
```

## Contradictions & Surprises

- M4's median implementer (5.4 USD) is cheaper than M3's (10.0), while its mean is higher (10.1 against 9.6). Four big leaves carry the mean, and the rest were small.
- The last day cost as much as the M3 build day (323 USD on 12 Nov against 335 on 11 Nov), but 244 of it went to M4's first overnight wave, which started late on the 11th.
- The coordinator's window grew after its restart on 12 Nov. It compacted at 415k, where before it compacted at about 217k. Round 00 said its window was fixed at session start; a resumed session can apparently take the current setting.

## Steering Questions

- [now] Size leaves so that none needs more than about 250 implementer calls? M4's four leaves over 400 calls cost 163 USD and all compacted. Splitting them at drafting time is the cheapest lever this data shows.
- [next run] Keep the M4 rules (two fix rounds at most, one verifier report, re-checks sized to the diff) as written into every brief? The L4–L5 figures support them, with the confound noted above.
- [later] For the MAS cost study, track USD per leaf and calls per leaf as the unit metrics, not USD per line. Lines added vary with fixtures and reshaping too much to compare milestones.

## Pointers

- Data, snapshot 2026-11-13 01:22 UTC: `/private/tmp/claude-501/-Users-hacker-Documents-tmp-claude-worktrees-Candler-candler-milestone-coordination-13808c/9af68a2d-3c86-4bc4-a1de-32450da8dbe2/scratchpad/usage/v1/calls.csv`
- Scripts: `analyse_v1.py` (corrected roles, M4 leaves and phases, windows) and `tbl.py` (per-milestone role tables), in the same folder; output in `report_v1.txt`
- Line counts: `git diff --numstat` on each milestone PR's merge commit (M1 9386e99, M2a 656be71, M2b 049a9ab, M2c 9c18ed3, M3 1c7018d), and main to 4328c53 for M4
- Prior version: [00-findings_campaign_costs_v0.md](00-findings_campaign_costs_v0.md). Companion: [02-findings_context_window_v1.md](02-findings_context_window_v1.md)
