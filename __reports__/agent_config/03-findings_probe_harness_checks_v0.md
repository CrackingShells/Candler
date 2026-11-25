---
type: findings
topic: agent_config
date: 2026-11-25
version: v0
prior-version: none
key-metric: questions answered by measurement: 7 of 7 (prior: N/A, delta: N/A)
decision-required: confirm
---

# Harness checks for instructions and hooks — Findings (v0)

Date: 2026-11-25

**Session-start hooks: both harnesses have one that reaches the model, so the plan does not return to Eliott for that reason.** It does return on two points that change the v2 wording of G4 and `context_start` (the first two rows of the Contradictions section): on Claude Code the instruction texts land in the first message, never in the system text, and on OpenCode the gateway's first request is a title request that carries neither instructions nor hook output.

## Headline Result

metric: probe questions answered by measurement (a)–(g)
value: 7
unit: questions of 7
prior: N/A (first run)
direction: new

## Verdict per question

| Q | Answer | What L2 / L3 does with it |
|---|---|---|
| a | `claude doctor` runs with no TTY and no network, exit **0** in every case, valid or not. Errors and warnings in project settings (bad JSON, unknown hook event) print under one `Invalid settings` heading. A section of 4 install-method warnings always prints in the cell image, so a warning count is useless. | `config_check` for Claude Code is `claude doctor`. A finding is the presence of an `Invalid settings` heading. Ignore the exit code and the `N warnings found` block. |
| b | `InstructionsLoaded` fires under `--print --setting-sources project,local` and carries `file_path`, `memory_type` (`Project` or `User`), `load_reason` (`session_start`), `cwd`, `session_id`, `transcript_path`. | Census reads it. It works with the hook set in the home `--settings` file. |
| c | `opencode debug config` and `opencode plugin list` need no server and no network, exit 0 always. `debug config` shows a rejected `opencode.json` only by the document being absent from the list, with no message. `plugin list` prints `No plugins found` even with a valid local plugin installed. | Neither is a usable check. OpenCode's `config_check` is the standalone start below. |
| d | Yes. With `XDG_CONFIG_HOME=/home/agent/.xdg/config`, `…/opencode/AGENTS.md` and `…/opencode/plugins/good.ts` both load (`opencode debug paths`: `config /home/agent/.xdg/config/opencode`; request shows `Instructions from: /home/agent/.xdg/config/opencode/AGENTS.md`; log `loading plugin`). | Install OpenCode's global file and plugins there, as v2 says. |
| e | With `user` and a home holding only `~/.claude/CLAUDE.md` (Candler's `--settings` file is passed by flag, not read as `user`), exactly that one file loads, as `memory_type: User`. If the home also holds `~/.claude/settings.json` or `~/.claude/rules/*.md`, `user` loads both (the decoy hook fired and the rule text reached the model). A parent-directory `CLAUDE.md` also loads, but as `Project`, with no `user` needed. | **Claude Code global file: the run's home, `~/.claude/CLAUDE.md`, with `user` added to the setting sources.** See Placement below. |
| f | Yes on both. A single file bind-mounted read-only at `/work/CLAUDE.md` (Claude Code) or `/work/AGENTS.md` (OpenCode) is read. Order on both: global first, workspace second. | The read-only workspace mount of v2 works on both harnesses. |
| g | Claude Code: `SessionStart` fires with `--print`; its output is in the **first message**. OpenCode: a local plugin `session.hook("context", …)` adds a system block; it is in the **system text**, after the instructions. | L3's hook is `SessionStart` on Claude Code and a `context` plugin hook on OpenCode. Both reach the model. |

## Evidence per question

Each row is one run. `home_g` holds only `~/.claude/CLAUDE.md` (`GLOBAL_CC_MARKER_G1`) and an observation `--settings` file `obs.json` (an `InstructionsLoaded` writer and a `SessionStart` echo). `home_g2` is `home_g` plus decoy `~/.claude/settings.json` (a `SessionStart` hook echoing `DECOY_USER_SETTINGS_HOOK_D1`) and `~/.claude/rules/decoy.md`. `oc_*` homes hold `$XDG_CONFIG_HOME/opencode/{opencode.json, AGENTS.md, plugins/good.ts}`; `oc_errcfg` has a bad-JSON `opencode.json`, `oc_errplug` adds `bad.ts` with no default export, `oc_warn` adds an unknown key and an unknown provider setting. "Recorder" is a Python HTTP server on the `--internal` network `m6_5-probe-net` that writes each request body and answers 400, so claude exits 1 and no model runs. All containers ran as user `agent`, `--rm`, label `m6_5-probe`. CC image `candler-base:src-83c18228eddb`; OC image `candler-base:src-b062c666af0a`.

| Id | Q | Command (what matters) | Exit | Key output line |
|---|---|---|---|---|
| E1 | a | CC, `--network none`, `home_g`, cwd `/work` = `w_valid` / `w_err` / `w_warn`: `claude doctor </dev/null` | 0, 0, 0 | valid: no `Invalid settings` (count 0); err: `/work/.claude/settings.json: Invalid or malformed JSON`; warn: `hooks.NoSuchEvent: Unknown hook event "NoSuchEvent" was ignored. Valid events: …`; all three: `4 warnings found` |
| E2 | b | CC, recorder net, `ANTHROPIC_BASE_URL=http://m6_5-probe-rec:8080`, dummy key, `home_g`, `/work` = `w_valid` (CLAUDE.md `WORKSPACE_MARKER_W1`, project hooks): `claude --print 'say hi' --model claude-sonnet-4-5 --output-format stream-json --verbose --setting-sources project,local --settings /home/agent/obs.json` | 1 | payload `{"hook_event_name":"InstructionsLoaded","file_path":"/work/CLAUDE.md","memory_type":"Project","load_reason":"session_start","cwd":"/work",…}`; message 0 holds `WORKSPACE_MARKER_W1`, `SESSIONSTART_HOOK_MARKER_S1`, `SESSIONSTART_OBS_MARKER_S2`; `GLOBAL_CC_MARKER_G1` absent; `system` holds none |
| E3 | e | as E2 but `--setting-sources user,project,local`, `home_g2`, `/work` = empty dir | not recorded | files loaded: `/home/agent/.claude/CLAUDE.md User`, `/home/agent/.claude/rules/decoy.md User`; message 0 holds `GLOBAL_CC_MARKER_G1`, `DECOY_USER_RULE_D2`, `DECOY_USER_SETTINGS_HOOK_D1` |
| E4 | e | as E3 with `home_g` | not recorded | one file loaded: `/home/agent/.claude/CLAUDE.md User` |
| E5 | e | as E2 but `project,local`, `home_g2`, cwd `/p/work` (no CLAUDE.md), `/p` mounted with `/p/CLAUDE.md` = `PARENT_MARKER_P1` | not recorded | one file loaded: `/p/CLAUDE.md Project`; global and decoys absent |
| E6 | f | as E2 with `user,project,local`, `home_g`; `/work` = dir with an empty placeholder `CLAUDE.md`, over it `-v ro_CLAUDE.md:/work/CLAUDE.md:ro` | 1 | loaded: `/home/agent/.claude/CLAUDE.md User`, `/work/CLAUDE.md Project`; message 0 block: `Contents of /home/agent/.claude/CLAUDE.md …G1` then `Contents of /work/CLAUDE.md …W1`; the `SessionStart:startup hook success: SESSIONSTART_OBS_MARKER_S2` block is a separate block ahead of it. A first try without the placeholder file failed in Docker: `mountpoint … is outside of rootfs` (the host directory needs the file to exist) |
| E7 | f | as E2 with `project,local`, `home_g`, cwd `/p/work`, `/p` mounted, `/p/work/CLAUDE.md` = ro bind of `ro_CLAUDE.md` over a placeholder | 1 | loaded: `/p/CLAUDE.md Project`, `/p/work/CLAUDE.md Project`; message 0: parent text, then workspace text |
| E8 | c | OC, `--network none`, `/work` = `oc_work`, `XDG_*` under `/home/agent/.xdg`: `opencode debug config` and `opencode plugin list` on `oc_valid`, `oc_errcfg`, `oc_errplug`, `oc_warn`; second pass added `--print-logs --log-level debug` | 0 on all 8 (second pass; first pass printed none) | valid: JSON list with `{"type":"document","path":"…/opencode.json"}`; `oc_errcfg`: only `{"type":"directory",…}`, no document and nothing on stderr; `plugin list`: `No plugins found` on all four, including `oc_valid` |
| E9 | d, f, g | OC, recorder net, `CANDLER_GATEWAY_URL=http://m6_5-probe-rec:8080`, `oc_valid`, `/work` = `oc_work` with `ro_AGENTS.md` ro-bound over `AGENTS.md`: `timeout 90 opencode run --standalone --format json --print-logs --log-level debug -m candler/claude-sonnet-4-5 'say hi'` | 1 | stdout `provider.invalid-request … "recorder: refused"`; log `loading plugin … plugins/good.ts`; request 1 system text: `Instructions from: /home/agent/.xdg/config/opencode/AGENTS.md` (G2), `Instructions from: /work/AGENTS.md` (W1), then `PLUGIN_CONTEXT_MARKER_S3` as the last system block; request 0 has 0 tools and none of these |
| E10 | c, config_check | OC, `--network none`, `oc_work`, each `oc_*` home: `timeout 60 opencode run --standalone --print-logs -m probe/none 'x' </dev/null` | 1 on all four | all four: `ERROR … Failed to drain Session … ModelUnavailableError: Model unavailable: probe/none`; `oc_errplug`: `WARN … failed to load plugin target=…/bad.ts … Plugin must export a default definition with an id and an effect or setup function`; `oc_errcfg`: `WARN … configuration normalization diagnostic … kind=invalid action="rejected malformed JSON or JSONC document"`; `oc_valid`, `oc_warn`: no `WARN` |
| E11 | c | as E10 with `-m candler/claude-sonnet-4-5 --format json --log-level debug`, on `oc_errplug`, `oc_errcfg`, `oc_warn` | 124, 1, 124 | `oc_errplug`, `oc_warn`: `retry … provider.transport … Can't reach m6_5-probe-rec` until the timeout; `oc_errcfg`: `Model unavailable: candler/claude-sonnet-4-5` |
| E12 | d | OC, `--network none`, `oc_valid`: `opencode debug paths` | not recorded | `config     /home/agent/.xdg/config/opencode` |

## Results Tables

### Where each text lands (recorded first agent request, gateway recorder answering 400)

| Harness | Global file | Workspace file | Hook output | Order in the request |
|---|---|---|---|---|
| Claude Code 2.1.311 | first message, in one `<system-reminder>` block headed `Contents of /home/agent/.claude/CLAUDE.md (user's private global instructions …)` | same block, `Contents of /work/CLAUDE.md (project instructions …)` | its own `<system-reminder>` block, `SessionStart:startup hook success: SESSIONSTART_OBS_MARKER_S2`, earlier in the message than the instruction block | hook block, then global, then workspace. The `system` text holds none of the three (searched for each marker: not found). |
| OpenCode 2.2.7 | system text, `Instructions from: /home/agent/.xdg/config/opencode/AGENTS.md` | system text, `Instructions from: /work/AGENTS.md`, directly after the global | system text, a separate trailing block `PLUGIN_CONTEXT_MARKER_S3` | global, workspace, then the date and environment, then the hook block. The first message is the prompt only. |

### The three installs, run through each check (network `none`, image of the harness)

| Install | Claude Code: `claude doctor` | OpenCode: `opencode run --standalone --print-logs -m probe/none x` |
|---|---|---|
| Valid | exit 0; no `Invalid settings` heading | exit 1; stderr has no `WARN` and only the expected `ERROR … Failed to drain Session … ModelUnavailableError` |
| Error | `.claude/settings.json` bad JSON: exit 0; `Invalid settings` / `/work/.claude/settings.json: Invalid or malformed JSON`. OpenCode: `opencode.json` bad JSON: `WARN … configuration normalization diagnostic … kind=invalid action="rejected malformed JSON or JSONC document"`; plugin with no default export: `WARN … failed to load plugin target=…/bad.ts … Plugin must export a default definition with an id and an effect or setup function` | as the left column |
| Warning | hook on an unknown event: exit 0; `Invalid settings` / `hooks.NoSuchEvent: Unknown hook event "NoSuchEvent" was ignored. Valid events: …` | An unknown config key and an unknown provider setting produce no `WARN`; `debug config` shows the provider setting kept. OpenCode has no warning class for these. |

### Requests the gateway saw (OpenCode)

| seq | Tools | System text | Holds the markers? |
|---|---|---|---|
| 0 | 0 | 1,806 chars (title generator) | no |
| 1 | 12 | 9,098 chars | global, workspace and plugin markers |

Claude Code sent six requests: the first three with 12 tools, the rest with 26. All six carry the same first message, so any of them holds the markers.

## Observations

| Signal | Baseline / Expected | Observed | Interpretation |
|---|---|---|---|
| Claude Code instructions in `system` | v2 G4: `context_start.system` holds both texts | markers absent from `system`, present in message 0 `[source: E2, E6]` | G4 must read `system` or `first_message`, per harness. |
| OpenCode seq 0 | v2: the gateway's first request is the agent's context | seq 0 is a title request with no tools `[source: E9]` | The event must come from the first request that carries the tools. |
| `claude doctor` exit code | non-zero on a finding | 0 on a malformed file and on an unknown event `[source: E1]` | `read_config_check` parses output. |
| `user` source scope | loads only `CLAUDE.md` | also `settings.json` hooks and `rules/*.md` of the home `[source: E3 vs E4]` | Safe only because Candler's home holds nothing else there. |
| OpenCode config rejection | an error | exit 0, document silently missing from `debug config` `[source: E8]` | `debug config` cannot gate. |
| Global file via parent directory | scope `User` | `Project` `[source: E5, E7]` | The census would label the global file as project scope. |

## Placement and checks L2 uses

### Claude Code's global file: the run's home

**L2 installs it at `~/.claude/CLAUDE.md` in the run's home and adds `user` to `--setting-sources` (`user,project,local`).** Why:

- The v2 rule is met: with Candler's home, `user` loads only that file (E4). Candler's observation hooks go in by `--settings`, not through `user`.
- The census scope stays truthful: the harness reports it `User`. Through the parent directory it reports `Project`, indistinguishable from the workspace file.
- Read order is unchanged either way: global first, workspace second.

The cost to state on the capability page: `user` loads whatever else sits in `~/.claude/` (settings hooks, rules). Candler's home must stay free of anything else there, and a consumer who plants such files through a future home mechanism would reach the agent. Not probed: `~/.claude/skills`, `~/.claude/agents`, `~/.claude.json` under `user`.

### `config_check` per harness

| Harness | Command (network `none`, no TTY) | An error or warning is | Ignore |
|---|---|---|---|
| Claude Code | `claude doctor`, cwd the workspace | a line `Invalid settings` and the bullets under it (covers a rejected file and a skipped entry alike) | exit code (always 0); the `N warnings found` block; `Remote Control`, `Managed settings`, `Organization policy` lines |
| OpenCode | `opencode run --standalone --print-logs -m probe/none x`, with a throwaway `XDG_DATA_HOME` | any stderr line with `level=WARN`. Known limit: an unknown config key or provider setting gives none. | exit code (always 1); the one `ERROR … Failed to drain Session … ModelUnavailableError` line; `INFO` lines. `opencode debug config` and `opencode plugin list` are not used. |

Measured for the OpenCode command (E10): network `none`, exit 1 with `ModelUnavailableError`, and a `loading plugin` log line for each local plugin. The report records no gateway request for it. With the real model name on network `none` (E11) the run did not end: exit 124 after retrying for 90 s, so the bogus model name is what makes it end. It is the only measured way to see a failed local plugin. Known limit of the check: an unknown config key or provider setting produces no `WARN` (E10), so those pass. Not probed: a future OpenCode version that rejects the model name before loading plugins.

### Session-start hook per harness (L3)

| Harness | Hook | Where its output lands in the first agent request |
|---|---|---|
| Claude Code | `SessionStart` (works under `--print`) | first message, own `<system-reminder>` block, before the instructions block |
| OpenCode | plugin `session.hook("context", e => e.system.push({type:"text", text}))` | system text, last block, after the instructions and environment |

## Contradictions & Surprises

- **G4 as written fails on Claude Code.** Neither instruction text is in `system`. Read both from `system` and `first_message`, per harness.
- **`context_start` as written records the wrong request on OpenCode.** Seq 0 is a title request. The event needs "the first request with tools" (or the first request of the agent loop). The ADR sentence "one event per session, from the first request only" changes accordingly.
- `claude doctor` never fails by exit code, and OpenCode's two named checks cannot see a bad plugin. Only output parsing and a standalone start work.
- OpenCode's plugin import `@opencode/plugin` resolved for a local file plugin without installing a package.

## Steering Questions

- [now] Does Eliott accept the amended proof wording: instruction texts and the Claude Code hook marker read from `system` or `first_message` per harness, and `context_start` taken from the first request that carries the agent's tools? `trace_context` and `e2e_guard` follow it.
- [now] Does Eliott accept the home placement with `user` for the Claude Code global file, given that `user` also loads other files of the home?
- [next run] Should L2 probe `~/.claude/skills`, `agents` and `~/.claude.json` under `user` before it merges the placement?

## Pointers

- [02-architecture_agent_config_v2.md](02-architecture_agent_config_v2.md): §Where each value lands, §Live trace proof, gates G4, G11, G12
- [probe_harness_checks.md](../../__roadmap__/m6_5-agent-config/probe_harness_checks.md): the leaf
- Images: `candler-base:src-83c18228eddb` (Claude Code 2.1.311), `candler-base:src-b062c666af0a` (OpenCode v2.2.7)
- Raw runs (removed at the close): `$HOME/m6_5-probe-scratch/runs/`
