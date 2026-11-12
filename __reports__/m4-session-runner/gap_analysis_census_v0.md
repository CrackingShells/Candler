# Gap Analysis: m4-session-runner - No tool census for OpenCode

## Problem Statement
The `init_tools` gate compares the agent's tool inventory between the arms of a stratum, and an unrecorded inventory never counts as equal. Claude Code reports its inventory in its init event. OpenCode V2 reports it nowhere the driver can read, so without a new source every OpenCode stratum would be refused on `init_tools`.

## Evidence
From `leaves/opencode_facts_v0.md` (OpenCode v2.2.7, fake-model runs plus three RiVault runs):

| Source | Lists the tools? |
|:--|:--|
| NDJSON events (`step_start`, `text`, `tool_use`, `step_finish`, `retry`, `error`) | no |
| `session export` (`info`, `messages`) | no |
| `debug agents`, `GET /api/agent` | permission rules only |
| The server's OpenAPI (170 operations), `/api/skill`, `/api/mcp`, `/api/command` | no tools endpoint |
| The model request on the wire (`tools` array) | yes |

Each session also sends one tool-less title request.

## Root Cause
Plan decision D9 assumed a harness exposes its inventory through an init event or a command. OpenCode does neither.

## Impact Assessment
- Scope: the census source for harnesses without an init event. Claude Code keeps its init-event census.
- Dependencies: `opencode_wiring` (L3) records and collects the source; the census in `candler-core` reads it; G3 and M6's paired statistics need recorded censuses.
- Risk: a census taken from the wrong request, or an empty set taken as a census, would make OpenCode strata compare on a false inventory.

## Options considered
| Option | Verdict |
|:--|:--|
| A: the driver declares the census from the config it writes | rejected: a declared census is not evidence |
| B: the gateway sidecar records the tool names of each model request as a raw stream | chosen: the only observed source, and harness-neutral |
| C: `capabilities.census = false` | rejected: it refuses every OpenCode stratum, or exempts OpenCode from the gate |

## Decision
Option B. The coordination session backed it. Eliott approved the amendment on 2026-11-12, relayed by the coordination session. Conditions:
1. Names only: never schemas, descriptions or message content. No secret reaches the store.
2. `init_tools` is the sorted set of tool names in the session's first model request that carries a non-empty `tools` array. Tool-less requests are skipped but kept in the raw stream.
3. The record says the census was observed by the gateway. A declared or unrecorded census never counts as equal to an observed one.
4. A session with no request carrying tools leaves the census unrecorded and refused, never an empty set.
5. Nothing moves for Claude Code: no stored digest and no existing record, proven by GOLDEN and DIGEST.
6. Fake-model tests only. No RiVault runs beyond those already approved.
