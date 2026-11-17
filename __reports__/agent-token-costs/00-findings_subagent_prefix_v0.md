# What every subagent starts with — Findings (v0)

Date: 2026-11-12

---
type: findings
topic: agent-token-costs
date: 2026-11-12
version: v0
prior-version: none
key-metric: fixed prompt of a general-purpose subagent: 61,279 tokens (prior: N/A, delta: N/A)
decision-required: confirm
---

## Headline Result

metric: tokens in a general-purpose subagent's first call, with a 20-word brief
value: 61,279
unit: tokens (Sonnet 5.5 tokenizer)
prior: N/A (first run; the campaign median over 144 implementers is 61,860)
direction: new

About three quarters of it is tool definitions and two name lists that an implementer never uses. The prompt is re-read on every call: across the campaign's 14.9k subagent calls, that comes to about 180 USD of cache reads, 13% of all spend.

## Results Tables

### Composition of the 61.3k prompt (general-purpose, 42 tools)

| Part | Tokens | Share | How it was measured | Does an implementer need it? |
|---|---:|---:|---|---|
| Deferred tool names (421 names) | 10,690 | 17% | **Measured**: a probe printed the list; the context grew by this much | Mostly no. It needs the 9 colgrep names |
| Skill listing (89 skills) | 10,450 | 17% | **Measured**: the same probe, in two halves (5,320 + 5,155, minus call overhead) | Mostly no. It needs `rust-conventions`, `waiting-on-processes` and `designing-tests` |
| `Agent` and `Artifact` tool definitions | ~13,000 | 21% | Estimate: general-purpose minus Explore (15.9k), minus Edit, Write, CLAUDE.md and hooks | No |
| 32 app and plugin tools (browser ×17, iOS simulator, visualize ×2, terminal, docs connector ×3, session and turn tools) and `SearchPlugins` | ~17,000 | 28% | Estimate: Explore's 45.4k minus both lists, the system prompt and the core tools | No |
| Core tools (`Bash`, `Read`, `Edit`, `Write`, `Skill`, `ToolSearch`, `SubagentHandback`) | ~5,000 | 8% | Estimate from description lengths | Yes |
| System prompt, environment, date and reminders | ~3,000 | 5% | The web-fetch agent's floor is 4.3k with one tool and no lists | Yes |
| `CLAUDE.md`, `AGENTS.md` and the two hook policies | ~2,000 | 3% | Characters × 0.3 | Yes |
| Brief (a real median brief adds ~1.6k) | ~100 | 0% | — | Yes |
| **Total** | **61,279** | 100% | Measured | **~10–15k needed** |

### First-call size by agent type (campaign transcripts plus the two probes)

| Agent type | Tools | Lists present | First call, median (tokens) | n |
|---|---:|---|---:|---:|
| general-purpose | 42 | names + skills | 61,860 | 144 |
| spec-verifier | 42 | names + skills | 64,031 | 21 |
| rust-conventions reviewer | 42 | names + skills | 62,019 | 21 |
| Explore / Plan | 37–38 | names + skills | 45,452 | 34 |
| claude-code-guide | 5 | none | 45,873 (its own docs prompt) | 1 |
| web-fetch | 1 | none | 4,329 | 1 |
| Probe: general-purpose, 20-word brief | 42 | names + skills | 61,279 | 1 |
| Probe: Explore, 1-line brief | 38 | names + skills | 45,431 | 1 |

### Where the 421 deferred names come from

| Source | Names | Used by Candler agents? |
|---|---:|---|
| ArangoDB | 46 | No |
| GitHub connector | 45 | Rarely (`gh` is used instead) |
| octopus-dev, banyan, dgx1 and rikyu HPC servers | 116 | No |
| git-cmds | 28 | No (git through Bash) |
| macOS Calendar and Reminders | 28 | No |
| Desktop session management, sidebar, host, view and window | ~50 | Orchestrators only |
| Wolfram toolkit | 15 | No |
| colgrep | 9 | **Yes** (the hook denies Grep) |
| Others (Drive, Notes, arXiv, Svelte, scheduled tasks…) | ~84 | No |

## Observations

| Signal | Baseline / Expected | Observed [source] | Interpretation |
|---|---|---|---|
| Prompt size | A few thousand tokens of instructions | 61.3k, of which ~44k is tools and lists an implementer doesn't use [source: probes a2d2…, aa42…] | Roughly 70% of the fixed prompt is avoidable for implementers and verifiers |
| Tokens per character | ~0.25–0.3 for prose | Deferred names 0.63; skill listing 0.35 [source: probe deltas] | Identifiers with server hashes tokenize poorly. The 17k-character name list costs as much as the 30k-character skill listing |
| Lists without the tools that read them | Lists always present | claude-code-guide (5 tools) and web-fetch (1 tool) carry neither list [source: prefix.csv] | A restricted tool set seems to drop both lists. Verify on a pilot agent |
| Cross-agent cache sharing | Each spawn writes its whole prompt | The probe's first call wrote 24.7k and read 36.6k already cached by other agents | The shared head (system prompt and tool definitions) is cheap to spawn. The cost is the re-read on every later call |
| Cost over the campaign | — | 14.9k calls × 61k × 0.20 USD/MTok ≈ 182 USD [source: calls.csv] | Cutting the prompt to ~15k would have saved ~140 USD, about 10% of all spend |

## Charts & Visualizations

The 61.3k prompt, one block = 1k tokens (█ = needed by an implementer, ░ = not needed):

```
Agent + Artifact tools     ░░░░░░░░░░░░░            ~13k
App and plugin tools (33)  ░░░░░░░░░░░░░░░░░        ~17k
Deferred tool names        ░░░░░░░░░░░              10.7k  measured
Skill listing              ░░░░░░░░░░               10.4k  measured
Core tools                 █████                     ~5k
System + reminders         ███                       ~3k
CLAUDE.md, AGENTS, hooks   ██                        ~2k
```

## Contradictions & Surprises

- The shortest list in characters is the most expensive per character: hashed server prefixes such as `mcp__30f29341-…__` make each name cost ~25 tokens.
- Explore and Plan agents carry the same two lists as general-purpose agents, although they can't use most of the tools named.
- Headless `claude -p` can't sign in from inside the desktop app ("OAuth session expired"). The measurement therefore used two small subagent probes, costing about 0.30 USD.

## Steering Questions

- [now] Disable the MCP servers and plugins Candler never uses, project-wide? That cuts the name list and the skill listing for every session, main sessions included.
- [now] Define `implementer` and `verifier` agents with an explicit tool allowlist, and measure their first call against the 61.3k baseline before adopting them.
- [next run] Check whether skills can be preloaded into an agent definition, so that `Skill` and its 10k listing can go.
- [later] Measure the main-session prompt the same way. Its post-compaction floor is 100–140k tokens, and it carries more tools.

## Pointers

- Probe transcripts: `~/.claude/projects/-Users-hacker-Documents-tmp-claude-worktrees-Candler-candler-milestone-coordination-13808c/9af68a2d-3c86-4bc4-a1de-32450da8dbe2/subagents/agent-a2d2c81e762e59a21.jsonl` and `agent-aa42a368503ed476c.jsonl`
- Per-agent split of the visible prompt parts: `/private/tmp/claude-501/-Users-hacker-Documents-tmp-claude-worktrees-Candler-candler-milestone-coordination-13808c/9af68a2d-3c86-4bc4-a1de-32450da8dbe2/scratchpad/usage/prefix.csv` (script `prefix.py`)
- The two lists as probed: `probe_deferred.txt` and `probe_skills.txt` in the same folder
- Context and cache settings: code.claude.com/docs `env-vars`, `sub-agents`, `model-config` and `prompt-caching`
- Companion reports: [00-findings_campaign_costs_v0.md](00-findings_campaign_costs_v0.md), [00-architecture_agent_system_v0.md](00-architecture_agent_system_v0.md)
