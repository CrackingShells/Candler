# Coordinating M5b: what the money paid for (v0)

Date: 2026-11-17. Source: the M5b milestone coordinator, from the transcripts of the three
coordinator sessions and their 33 subagents. It follows the M4 report
(`__reports__/m4-session-runner/coordination_knowledge_transfer_v0.md`) and is written for Eliott and
for any future coordinator that dispatches implementers and verifiers.

## Executive summary

- **What shipped.** M5b built in two levels:
  - the per-cell egress proxy, with `isolation.egress_unsupported` lifted;
  - the D9 ground-truth refusal and the I8 e2e;
  - A1's single inspect per destroy, and a destroy for containers that were created but never ran;
  - the `gateway.upstream_ca` machine-config key;
  - two security holes the tests found and closed: a proxy filter that checked only the first
    resolved address, and an agent that reached the Docker host on Linux.

  The final read-only verifier found every gate except the live smoke clean. CI's container job
  passed 21/21.
- **What it cost.** About $111 to build, against a $52 cap: 2.1× over plan. Planning cost $10 more. Fix level after the live smoke added $15.46 of that.
- **Where it went.** Coordinators cost $71 and the agents doing the work cost $51. **Coordination
  was 58 % of the bill.**
- **Did the frugality rules work?** For the agents they bound, yes:
  - an implementer round fell from $4.55 to $1.40;
  - its average context fell from 171k to 111k;
  - its calls fell from 84 to 29.

  But L1 needed 17 implementer rounds where L0 needed 3. And the rules did not change the
  coordinators, who ignored the read rule and woke on every message.

## Where the money went

Measured at list rates from the transcripts (`m5b-spend.py`, and `categorize.py` for the categories).
Each API message is counted once. A message's cost is split evenly over the tool calls it made.
Context means the input, cache-write and cache-read tokens of one call.

| Session | Role | Agents | Tool calls | Avg context | Cost |
|:--|:--|--:|--:|--:|--:|
| L0 | coordinator (Opus) | 1 | 120 | 148k | $10.71 |
| L0 | implementers (Sonnet) | 3 | 251 | 171k | $13.66 |
| L1 | coordinator (Opus) | 1 | 222 | 155k | $23.28 |
| L1 | implementers and fix rounds (Sonnet) | 17 | 501 | 111k | $23.78 |
| L1 | verifiers (Sonnet) | 5 | 80 | 92k | $3.11 |
| milestone | coordinator (Opus), planning included | 1 | 248 | 162k | $27.64 |
| milestone | roadmap author, live smoke, final verifier | 3 | 67 | — | $3.56 |
| milestone | fix level after the live smoke: coordinator share, 6 fix agents, smoke re-run | 7 | 182 | — | $15.46 |
| **all** | | **38** | | | **≈ $122** |

The plan was $52 to build, plus $15 to plan. The actual figures:

| Level | Actual | Plan |
|:--|--:|--:|
| L0 | $24.4 | $17.0 |
| L1, including one round after its close | $50.2 | $23.5 |
| The milestone coordinator's build share, live smoke included | $21 | $11.5 (close and contingency) |

### What each kind of call cost

Share of each group's cost, by the tool its calls used. Each row also gives the number of calls.

| Category | L0 implementers | L1 implementers | L1 verifiers | L1 coordinator | Milestone coordinator |
|:--|--:|--:|--:|--:|--:|
| build and test (cargo, nextest) | 31 % (69) | 18 % (85) | — | 4 % (11) | 1 % (3) |
| Docker | 15 % (37) | 16 % (81) | 22 % (14) | 9 % (25) | 3 % (9) |
| **read code: colgrep** | — | 17 % (119) | 34 % (37) | 3 % (8) | 2 % (5) |
| read code: shell (sed, cat, grep on source) | 14 % (42) | 17 % (101) | 11 % (10) | **26 % (62)** | 17 % (52) |
| read code: git grep, show, diff, log | 13 % (38) | 4 % (17) | 15 % (8) | 7 % (17) | 7 % (15) |
| read: Read tool | 4 % (4) | 4 % (10) | — | — | 2 % (2) |
| read: logs and outputs | 3 % (11) | 10 % (26) | — | 5 % (16) | — |
| wait and poll (Monitor, sleep loops) | 11 % (35) | 2 % (10) | — | 2 % (6) | — |
| dispatch and message | — | — | — | 13 % (34) | 7 % (24) |
| reply with no tool call | 2 % | 3 % | — | 15 % | 17 % |
| app (plan list, wrap-up, sessions) | — | — | — | 2 % (5) | 17 % (44) |
| git, commits, roadmap tooling | 1 % (3) | 1 % (9) | — | 6 % (17) | 9 % (22) |

## The frugality experiment

Eliott set these rules after L0:
- read code only through colgrep, and trust the result;
- target every other read, and widen it one step at a time;
- think the change through once, then write it;
- no polling;
- a hard cap on each agent's calls.

L1 ran under them.

| Per implementer round | L0 | L1 | Change |
|:--|--:|--:|--:|
| Tool calls | 84 | 29 | −65 % |
| Average context | 171k | 111k | −35 % |
| Cost | $4.55 | $1.40 | −69 % |
| Read calls (all kinds) | 32 | 16 | −50 % |
| Cost of reads | $1.53 | $0.71 | −54 % |
| Wait and poll calls | 12 | 0.6 | −95 % |
| Build and test calls | 23 | 5 | −78 % |

**What the numbers support:**
- **Polling stopped.** Thirty-five Monitor and sleep calls at L0 became ten across seventeen rounds.
- **Rounds became short and cheap.** That follows from fewer calls and from a context that never
  grew past about 140k.
- **colgrep was adopted, but not exclusively.** Implementers made 119 colgrep calls, and still made
  101 shell reads of code plus 17 git reads. Reads made up a *larger* share of calls at L1 (54 %, up
  from 38 % at L0) on a much smaller total. The rule cut the cost of each read through smaller
  contexts, not by replacing every read. One part of the measurement is still missing: we have not
  checked whether the colgrep calls carried the scoping arguments the rule asks for.
- **Verifiers are the cheapest agents.** They are colgrep-first and read-only. The final verifier
  checked eight gates in 7 calls for $0.48.

**What the numbers do not support.** That the rules cut the milestone's cost. L1 cost twice its
plan:
- **ground_truth_e2e took ten agent rounds** (about $12). Five went to one gateway 502 error, first
  through a misread base-image cache, then through the supervisor's cleared environment, which
  dropped `XDG_CONFIG_HOME`. The real cause was found one round at a time, each round starting cold
  at 70–100k context.
- **The L1 coordinator cost $23.** That is as much as its seventeen implementer rounds together.

## Pain points and root causes

| Pain | Root cause |
|:--|:--|
| Coordinators were 58 % of the bill | Each wake costs a full-context call chain. L1's coordinator woke 58 times (24 agent completions, 12 session messages, 22 other notices), with about 4.3 API calls per wake at 155k: about $0.40 a wake. The milestone coordinator woke 70 times on 43 session messages. A one-line progress note costs as much as a merge. |
| Coordinators read code by shell | Both coordinator briefs bound them to the colgrep rule. The L1 coordinator still made 62 shell reads of code, its largest category at 26 %, to check claims and to resolve conflicts. A coordinator's reads cost twice an implementer's, because its context is 1.4× larger and it runs on Opus. |
| Coordinators had no call cap | Implementers were capped and stayed under their caps. The coordinators were not capped, and made 222 and 248 calls. |
| One hard bug took five rounds | Fresh fix agents are cheap per round, but each one re-learns the problem. For a diagnosis, where the next step depends on what the last one found, the fresh-per-round rule costs more than one agent that keeps its context. |
| The app's own tools are not free | The milestone coordinator spent 17 % of its cost on plan and wrap-up calls: 44 calls, each re-reading a 160k context. |

## Next-cycle changes

**For coordinators** (Eliott will study coordinator cost separately; these are interim measures):
1. **Cap the coordinator** like the agents it dispatches. Put the cap in its brief, and have its
   parent check it from the transcript at each report.
2. **Fewer wakes.** Ask agents for one final report, never progress notes. Batch completions: let
   two or three leaves finish before integrating. Report to the parent at the level close only.
   Every message a coordinator receives costs a full-context call.
3. **Coordinators follow the read rule too.** Check a claim with one colgrep `expand`, or delegate
   the check to a read-only verifier. A verifier costs $0.50–$0.90; the L1 coordinator's 62 shell
   reads cost $6.10.
4. **Diagnosis keeps its agent.** A fix round with a known fix goes to a fresh agent. A bug whose
   cause is unknown stays with one agent, with a higher cap, until it is found.
5. **Restart the coordinator at about 120k.** Hand off through the carry notes. Its average context
   was 148–162k in all three sessions.

**For implementer briefs** (keep):
- colgrep first, with scoping arguments;
- targeted reads, widened one step at a time;
- no polling;
- hard call caps.

All four held, and together they cut the cost of each round by two thirds.

**To measure next time:**
- the share of colgrep calls that set `paths`, `include` and `limit`;
- reads per merged line;
- each coordinator's wakes and its cost per wake.

`categorize.py` can report all three.

## Artifacts to preserve

- `m5b-spend.py`, in the coordination notes folder: the spend per session tree.
- `categorize.py`: the per-category, per-role breakdown used above (copied next to `m5b-spend.py`).
- `m5b-l1-coordinator-carry.md`: the per-agent spend table and the record of every round.
- `m5b-final-verifier.md`: the G1–G9 table.

## Open questions

- How much of a coordinator's wake cost is the platform's fixed context (system prompt, tool and
  skill lists: 62–80k before any work), and how much is history it could shed by restarting?
- Would a Sonnet coordinator, under a rule to escalate design questions, cost half as much and still
  catch what the Opus coordinators caught?
- Should a level's integration be done by a fresh, short-lived agent per merge rather than by a
  coordinator that holds the whole level in its context?

## Fix level after the live smoke

Fix level is everything after the first live smoke: the six fix agents, the smoke re-run, and the
coordinator's cost above its $27.64 at the smoke. Measured with `categorize.py` from the transcripts.

| Role | Agent | Tool calls | Average context | Cost |
|:--|:--|--:|--:|--:|
| MCP render (F2) and F1 fix | a6a9742d | 36 | 103k | $1.10 |
| F2/F1 fix round: gates and core check | a40a6b16 | 23 | 88k | $0.71 |
| Drop or fix `Surface.mcp`, find the F1 refusal | ae0a0c66 | 11 | 87k | $0.45 |
| MCP guards through production code | a5274601 | 29 | 109k | $1.25 |
| Finish the MCP guards, real bites | ae6f21bd | 25 | 113k | $1.08 |
| F3: njs binary-safe egress filter | a21a626c | 35 | 112k | $1.35 |
| **Six fix agents** | | **159** | | **$5.94** |
| B5 smoke re-run (23 calls, 86k) plus $0.15 of RiVault model cost | a039003e | 23 | 86k | $0.93 |
| Coordinator (Opus), $36.23 now minus $27.64 | | | 164k whole session | $8.59 |
| **Fix level total** | | | | **$15.46** |
| Close agent | | | | measured at merge |

The coordinator was 56 % of fix level. Its $8.59 is $0.79 above the $7.80 the close brief quoted,
so the brief's figure was taken before the last session messages. The six fix agents averaged $0.99 each.

Categories of tool call, for the coordinator's whole session (the script does not split by phase), as
calls and cost:

| Category | Calls | Cost |
|:--|--:|--:|
| reply with no tool call | 0 | $6.07 |
| app: plan, wrap-up, sessions | 60 | $6.02 |
| read code or docs through the shell | 55 | $4.81 |
| coordinate: dispatch and message | 38 | $3.54 |
| git, commit, roadmap tooling | 27 | $3.02 |
| read: git grep, show, diff, log | 20 | $2.27 |
| write: Edit and Write | 32 | $1.64 |
| edit by script | 13 | $1.60 |
| Docker | 14 | $1.52 |
| tool and skill loading | 17 | $1.33 |
| CI (gh) | 11 | $1.14 |
| read: colgrep | 6 | $0.61 |

The frugality rule on reading was ignored again: shell reads cost 8 times what colgrep reads did.
