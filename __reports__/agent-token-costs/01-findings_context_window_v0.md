# Context window 250k against 500k — Findings (v0)

Date: 2026-11-12

---
type: findings
topic: agent-token-costs
date: 2026-11-12
version: v0
prior-version: none
key-metric: input-side cost of the 500k-era subagents replayed at a 250k window: -20% (prior: N/A, delta: N/A)
decision-required: none
---

## Headline Result

metric: change in input-side cost (cache reads, cache writes and compaction summaries) when the same subagent transcripts are replayed at a 250k window instead of 500k
value: -20
unit: % (199 against 249 USD for M2c, M3 and M4's subagents)
prior: N/A (first run)
direction: new

The 500k window cut compactions from 225 to 12, but made every call about 26% more expensive. Replayed on the same work, a 250k window is cheaper. That saving is an upper bound: compaction stalls an agent for about a minute and makes it re-read files, and the replay counts neither.

## Results Tables

### Which window each session actually ran with, inferred from when it compacted

| Sessions | Compacted at (median preTokens) | Window | Note |
|---|---:|---|---|
| M1, M2a, M2b | 217.7k (215–240k) | 250k | As Eliott set it |
| Coordinator, 5–11 Nov | 217k, still 205–227k on 11 Nov | 250k | It started before the change and kept 250k, even after the app updated to 2.1.321 |
| M2c, M3, M4 | 467k (322–472k) | 500k | As Eliott set it |
| Side sessions, 9–10 Nov | 364k and 470k | 500k | They started after the change, which happened between 9 Nov 10:07 and 15:20 UTC |
| M0 design (28 Oct) | 589k | larger, ~600k+ | Before either setting; left out of the comparison |

Both eras ran the same models, Opus 5.5 for main sessions and Sonnet 5.5 for subagents. Claude Code moved from 2.1.317 to 2.1.321 during M2c.

### Compactions

| Era | Kind | Compactions | Transcripts that compacted | Median duration | Total time compacting | Median summary (tokens) |
|---|---|---:|---:|---:|---:|---:|
| 250k | main | 96 | 5 | 70 s | 1.8 h | 5.7k |
| 250k | subagent | 129 | 74 | 63 s | 2.3 h | 6.0k |
| 500k | main | 9 | 5 | 63 s | 0.1 h | 5.7k |
| 500k | subagent | 3 | 3 | 62 s | 0.06 h | 6.0k |

Every compaction was automatic. Seven took under a second, apparently reusing a summary already prepared.

### Cost per call and per agent, by era

| Era | Role | Agents | Calls | Mean context (k) | p90 context (k) | USD per call | USD per agent | Compactions per agent |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 250k | main | 5 | 4,391 | 168 | 206 | 0.078 | 69 | 22.2 |
| 500k | main | 6 | 1,753 | 273 | 411 | 0.098 | 29 | 1.5 |
| 250k | implementer | 57 | 6,272 | 143 | 201 | 0.049 | 5.4 | 1.5 |
| 500k | implementer | 38 | 3,400 | 217 | 369 | 0.062 | 5.6 | 0.08 |
| 250k | verifier | 45 | 1,689 | 131 | 197 | 0.052 | 1.9 | 0.36 |
| 500k | verifier | 32 | 1,040 | 152 | 269 | 0.048 | 1.6 | 0 |

### The same transcripts replayed at another window (input-side cost, USD)

| Transcripts from | Kind | Model of actual | Replayed at 250k | Replayed at 500k | Replayed at 1M |
|---|---|---:|---:|---:|---:|
| 250k era | main | 275 | 309 | 350 (+13%) | 514 (+66%) |
| 250k era | subagent | 350 | 354 | 454 (+28%) | 517 (+46%) |
| 500k era | main | 138 | 131 (-6%) | 140 | 183 (+30%) |
| 500k era | subagent | 259 | 199 (-20%) | 249 | 259 (+4%) |

How the replay works:
- It keeps each transcript's per-call growth in tokens and drops the real compactions' falls.
- It compacts whenever the context passes the window's trigger (217k or 467k).
- A compaction costs one read of the context plus a 6k-token summary. The context then restarts at the measured post-compaction size (79k for a subagent, 122k for a main session), which is written to cache again.
- Calibration: replayed at its own window, each era's figure lands within 1–5% of the actual cost modelled the same way. 250k-era main sessions are the exception, at +12%.

### Latency

| Model | Seconds per call (fit) | Median at <100k | 100–200k | 200–300k | 300–400k | 400–500k |
|---|---|---:|---:|---:|---:|---:|
| Opus 5.5 (main) | 0.94 + **0.18 per 100k context** + 8.9 per 1k output | 3.6 s | 5.3 s | 6.1 s | 7.1 s | 6.9 s |
| Sonnet 5.5 (subagents) | 0.68 + **0.05 per 100k context** + 6.7 per 1k output | 2.9 s | 4.3 s | 5.1 s | 5.3 s | 6.0 s |

Seconds per 1k output tokens stay flat across context bands: 11.4–12.5 for Opus, 8.4–8.7 for Sonnet. The rise in the medians comes from longer outputs later in a task, not from the context.

### Behaviour in the 30 calls after a compaction, against the rest of the same transcripts

| Kind | Window | File touches | Share re-touching a file already read | Tool-error rate |
|---|---|---:|---:|---:|
| Subagent | after a compaction | 7,342 | **71.3%** | 4.6% |
| Subagent | elsewhere | 13,849 | 59.9% | 3.9% |
| Main | after a compaction | 3,780 | 66.7% | 3.0% |
| Main | elsewhere | 5,441 | 61.9% | 3.1% |

A "file touch" is a `Read` or a Bash `cat`, `sed -n`, `head` or `tail` on a file path.

### Implementers that compacted against those that didn't

| Era | Compacted | n | USD each | Calls | Rounds | USD per call |
|---|---|---:|---:|---:|---:|---:|
| 250k | no | 16 | 2.0 | 43 | 3.0 | 0.05 |
| 250k | yes | 41 | 6.7 | 136 | 7.3 | 0.05 |
| 500k | no | 35 | 4.6 | 77 | 4.2 | 0.06 |
| 500k | yes | 3 | 17.5 | 234 | 7.3 | 0.07 |

## Observations

| Signal | Baseline / Expected | Observed [source] | Interpretation |
|---|---|---|---|
| Compaction count | Fewer with a larger window | 225 → 12 [source: compactions.csv] | The 500k window almost removed compaction. Only main sessions and three very long implementers still reach it |
| Cost per call | Roughly flat | +26% for main sessions, +27% for implementers [source: era table] | Each call re-reads a larger context. Cache reads are 54% of all spend, so this compounds |
| Cost per agent | Lower at 500k (no compaction overhead) | Implementers unchanged (5.4 against 5.6 USD); main sessions not comparable | The two eras ran different leaves. Only the replay holds the work fixed |
| Replay, same work | 500k cheaper, because it skips compaction overhead | 250k is 20% cheaper for 500k-era subagents, 6% for main sessions; 500k would have cost the 250k era 13–28% more [source: replay table] | Compaction pays for itself: one 6k summary and one rewrite cost less than re-reading another 200–250k on every later call |
| Latency from context | Large | +0.05 s per 100k for Sonnet, +0.18 s for Opus; per-token speed flat [source: latency fit] | Context size barely affects latency. Output length drives it |
| Latency from compaction | Small | 63–70 s per compaction, 4.1 h in total in the 250k era against 0.17 h [source: compactions.csv] | The latency cost of a small window is the compaction stall, not slower calls |
| Behaviour after a compaction | Unchanged | Subagents re-touch already-read files 11 points more (71% against 60%); tool errors 3.9% → 4.6% [source: touches.csv] | Compaction loses working state, and agents rebuild it by re-reading. The replay doesn't price this, so the 20% is an upper bound |
| Fix rounds | Fewer at 500k? | 250k-era implementers that compacted needed 7.3 rounds against 3.0 [source: implementer table] | Confounded: long leaves both compact and need more rounds. No causal claim is possible |

## Charts & Visualizations

Input-side cost of the 500k-era subagents, replayed at each window, in USD (one block = 10 USD):

```
250k window  ████████████████████   199   41 compactions
500k window  █████████████████████████   249    2 compactions
1M window    ██████████████████████████   259    0 compactions
```

The same comparison for the 250k-era subagents:

```
250k window  ███████████████████████████████████   354   96 compactions
500k window  █████████████████████████████████████████████   454   15 compactions
1M window    ████████████████████████████████████████████████████   517    2 compactions
```

Median Sonnet response time by context band, in seconds (one block = 0.5 s):

```
<100k      ██████        2.9
100-200k   █████████     4.3
200-300k   ██████████    5.1
300-400k   ███████████   5.3
400-500k   ████████████  6.0   (output tokens also grow: 331 → 685 median)
```

## Contradictions & Surprises

- The larger window didn't make agents faster in practice. It removed about 4 hours of compaction stalls, but every call got more expensive, and per-call latency barely moved.
- The coordinator kept its 250k window after the global change and after the app updated. The window seems to be fixed when a session starts.
- M0 compacted at 589k on 28 October, before either setting. The default then was larger than 500k.

## Steering Questions

- [now] Treat the window as a per-role setting, not a global one? For subagents, `autoCompactWindow` in an agent definition, or `CLAUDE_CODE_AUTO_COMPACT_WINDOW` globally, which overrides it.
- [next run] Pilot implementers at a 250–300k window against 500k on comparable leaves, measuring cost, re-reads and verifier findings. That's the only way to price what the compaction loses.
- [next run] For orchestrators, keep a smaller window only alongside a carry file of decisions? The coordinator compacted 14 times with no lost ruling it knows of. It kept the carry file on its own initiative, though: nothing configured it. To rely on it, make it an explicit rule in orchestrator briefs, or a `PreCompact` hook.
- [later] For the MAS cost study, model cost per call as reads ∝ context × calls. Compaction turns growth that is quadratic in calls into a sawtooth, at the price of a ~6k summary, a ~65 s stall and +11 points of re-reading.

## Pointers

- Compaction records: `compactions.csv`; latency per call: `latency.csv`; file touches and tool results: `touches.csv`; summaries: `summaries.csv`
- Scripts: `window.py`, `touches.py`, `win_an2.py`; output: `window_report.txt`
- All in `/private/tmp/claude-501/-Users-hacker-Documents-tmp-claude-worktrees-Candler-candler-milestone-coordination-13808c/9af68a2d-3c86-4bc4-a1de-32450da8dbe2/scratchpad/usage/`
- Companion reports: [00-findings_campaign_costs_v0.md](00-findings_campaign_costs_v0.md), [00-findings_subagent_prefix_v0.md](00-findings_subagent_prefix_v0.md), [00-architecture_agent_system_v0.md](00-architecture_agent_system_v0.md)
