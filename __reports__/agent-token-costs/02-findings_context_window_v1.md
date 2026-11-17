# Context window 250k against 500k, with M4 complete — Findings (v1)

Date: 2026-11-13

---
type: findings
topic: agent-token-costs
date: 2026-11-13
version: v1
prior-version: 01-findings_context_window_v0.md
key-metric: input-side cost of the 500k-era subagents replayed at a 250k window: -24% (prior: -20%, delta: -4 points)
decision-required: none
---

## Headline Result

metric: change in input-side cost (cache reads, cache writes and compaction summaries) when the same subagent transcripts are replayed at a 250k window instead of 500k
value: -24
unit: % (337 against 443 USD for the subagents of M2c, M3, M4 and the side sessions)
prior: -20 (round 01, before M4 finished)
direction: the case for a smaller subagent window got stronger

M4's big leaves ran their implementers to the 500k ceiling, so the 500k era now has enough long transcripts to show the cost clearly. A 250k window would have compacted those agents about 79 times instead of 10, and still cost a quarter less on input. The caveats of round 01 still hold:
- each compaction stalls an agent for about 65 s;
- after a compaction, agents re-read files more often (71.5% of file touches against 60.7%).

The replay prices neither, so the saving is an upper bound.

## Results Tables

### Compactions, full data

| Era | Kind | Compactions | Transcripts that compacted | Median trigger (k) | Total time compacting |
|---|---|---:|---:|---:|---:|
| 250k | main | 97 | 5 | 218 | 1.8 h |
| 250k | subagent | 129 | 74 | 218 | 2.3 h |
| 500k | main | 12 (round 01: 9) | 5 | 467 | 0.16 h |
| 500k | subagent | 11 (round 01: 3) | 8 | 468 | 0.28 h |

The 8 new 500k compactions are all in M4, in the implementers of opencode_settings, opencode_wiring and session_runner (two each), and of harness_out_of_core and driver_traits (one each).

### Cost per call and per agent, by era

| Era | Role | Agents | Calls | Mean context (k) | p90 context (k) | USD per call | USD per agent |
|---|---|---:|---:|---:|---:|---:|---:|
| 250k | main | 5 | 4,616 | 173 | 210 | 0.081 | 75 |
| 500k | main | 6 | 2,497 | 277 | 412 | 0.095 | 40 |
| 250k | implementer | 57 | 6,272 | 143 | 201 | 0.049 | 5.4 |
| 500k | implementer | 52 | 6,038 | 235 | 392 | 0.066 | 7.6 |
| 250k | verifier | 45 | 1,689 | 131 | 197 | 0.052 | 1.9 |
| 500k | verifier | 53 | 1,844 | 160 | 267 | 0.051 | 1.8 |

500k-era implementers now cost 35% more per call than 250k-era ones (round 01: 27%), and 41% more per agent (round 01: about the same).

### The same transcripts replayed at another window (input-side cost, USD)

| Transcripts from | Kind | Model of actual | At 250k | At 500k | At 1M |
|---|---|---:|---:|---:|---:|
| 250k era | main | 302 | 323 | 366 (+13%) | 532 (+65%) |
| 250k era | subagent | 350 | 355 | 455 (+28%) | 518 (+46%) |
| 500k era | main | 194 | **179 (-9%)** | 197 | 279 (+41%) |
| 500k era | subagent | 463 | **337 (-24%)** | 443 | 531 (+20%) |

The method is the one from round 01:
- per-call growth is kept and the real compactions are dropped;
- a compaction is triggered at 217k or 467k, and costs one read of the context plus a 6k-token summary;
- the context then restarts at the measured post-compaction size, 79k for subagents and 122k for main sessions.

### 500k-era implementers that compacted against those that didn't

| Compacted | n | USD each | Calls | Rounds |
|---|---:|---:|---:|---:|
| no | 44 | 4.9 | 80 | 4.1 |
| yes | 8 | 22.8 | 313 | 8.3 |

### Latency (calls after a tool result, full data)

| Model | Fit | Median at 100–200k | 400–500k |
|---|---|---:|---:|
| Opus 5.5 | 0.92 s + **0.20 s per 100k** + 8.9 s per 1k output | 5.3 s | 6.9 s |
| Sonnet 5.5 | 0.62 s + **0.15 s per 100k** + 6.6 s per 1k output | 4.3 s | 5.9 s |

Sonnet's context slope tripled from round 01 (0.05 to 0.15 s per 100k) once M4's long implementers were in the data. It is still small: 400k of context adds about 0.6 s to a call, as much as 90 output tokens. Time per output token stays flat across bands (8.5–9.1 s per 1k for Sonnet).

### Behaviour in the 30 calls after a compaction

| Kind | After a compaction | File touches | Share re-touching a file already read | Tool-error rate |
|---|---|---:|---:|---:|
| Subagent | yes | 7,693 | 71.5% | 4.6% |
| Subagent | no | 16,481 | 60.7% | 3.8% |
| Main | yes | 3,869 | 66.9% | 3.0% |
| Main | no | 6,183 | 63.5% | 2.8% |

## Observations

| Signal | Baseline / Expected | Observed [source] | Interpretation |
|---|---|---|---|
| Replay, 500k-era subagents | -20% at 250k (round 01) | -24% [source: replay table] | M4's long implementers are exactly where a big window costs most: each call re-reads 300–470k |
| Where the cost sits | Spread | 8 compacting 500k implementers average 22.8 USD, against 4.9 for the other 44 [source: compacted table] | Long leaves, not the window, are the root. A smaller window caps the damage, and a smaller leaf avoids it |
| Latency from context | Negligible | Now 0.15 s per 100k for Sonnet [source: latency fit] | Still minor next to output length and the ~65 s compaction stall |
| Coordinator window | Fixed at 250k since its start (round 01) | Compacted at 415k after its 12 Nov restart [source: compactions.csv] | A resumed session can take the current global setting. Round 01's "fixed at start" holds only within one process |

## Contradictions & Surprises

- The 1M replay of 500k-era subagents is now +20% over 500k (round 01: +4%). M4's transcripts are long enough that a 1M window would rarely compact them at all.
- The leaves that compacted needed twice the rounds (8.3 against 4.1). That's the same confound as round 01: long leaves both compact and attract fix rounds.

## Steering Questions

- [now] Set subagents to a 250–300k window, for example through `autoCompactWindow` in an implementer definition, and keep 500k for orchestrators? The replay supports it more strongly than in round 01, but only as an upper bound.
- [next run] Measure re-reads and verifier findings for implementers at 250k against 500k on comparable leaves. That's still the only way to price what compaction loses.
- [next run] Address the root first: size leaves below about 250 implementer calls. See [02-findings_campaign_costs_v1.md](02-findings_campaign_costs_v1.md).

## Pointers

- Data and scripts, snapshot 2026-11-13 01:22 UTC: `/private/tmp/claude-501/-Users-hacker-Documents-tmp-claude-worktrees-Candler-candler-milestone-coordination-13808c/9af68a2d-3c86-4bc4-a1de-32450da8dbe2/scratchpad/usage/v1/`. It holds `win_an2.py` without the round-01 time cutoff, `window.py`, `touches.py`, and the output `window_report_v1.txt`.
- Prior version: [01-findings_context_window_v0.md](01-findings_context_window_v0.md)
