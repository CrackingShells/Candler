# Findings: M6.5 instructions and hooks, confirmed live from the trace

| Field | Value |
|---|---|
| Date | 2026-11-25 |
| Model | RiVault/Reasoning-Small, through the gateway (`model.token_secret = RIVAULT_LLM_INFERENCE`, by name only) |
| Candler | `bfdfa010` (branch `feat/m6.5-agent-config`, run record: `eval_infra.dirty = false`, `harness_config_check = clean`) |
| Harness versions | Claude Code 2.1.311 (trace `claude_code_version`); OpenCode: run record carries `harness_version` 2.2.7 |
| Shape | 2 `candler run` attempts, one per harness; 2 arms each (A, B, factor `instructions.workspace`); 1 replicate; prompt "Say hello in one sentence." |
| Gate | G13: G4 (global text and own arm's text, not the other arm's) and G11 (session-start marker), read from the first `context_start` of each stored trace |

## Result

| Harness | Arm | G4 | G11 | Global text | Workspace text | Marker |
|---|---|---|---|---|---|---|
| Claude Code | A | yes | yes | `first_message` | `first_message` | `system` |
| Claude Code | B | yes | yes | `first_message` | `first_message` | `system` |
| OpenCode | A | yes | yes | `system` | `system` | `system` |
| OpenCode | B | yes | yes | `system` | `system` | `system` |

In all four runs the other arm's text is absent from `context_start`. G13 passes on both harnesses.

## Excerpts (verbatim, from `context_start`)

Claude Code, arm A, `first_message`:
```
...GLOBAL_TEXT_CC_71a3\n\nContents of /work/CLAUDE.md (project instructions...
...checked into the codebase):\n\nWORKSPACE_TEXT_A_5be2\n</system-reminder>...
```
Claude Code, arm A, `system`: `SessionStart:startup hook success: SESSION_START_MARKER_CC_c40d`
Claude Code, arm B, `first_message`: `...checked into the codebase):\n\nWORKSPACE_TEXT_B_5be2\n</system-reminder>...`

OpenCode, arm A, `system`:
```
...opencode/AGENTS.md\nGLOBAL_TEXT_OC_71a3\n\nInstructions from: /work/AGENTS.md\nWORKSPACE_TEXT_A_5be2\n\nToday's date...
...SESSION_START_MARKER_OC_c40d   (the last line of the system text)
```
OpenCode, arm B, `system`: `...Instructions from: /work/AGENTS.md\nWORKSPACE_TEXT_B_5be2\n\nToday's date...`

## Deviations from the deterministic guard

- No blocking hook: the Claude Code `PreToolUse` script and the OpenCode `tool.execute.before` throw were left out, so G5's blocked-hook marker was not exercised live. The session-start hooks and all texts are the guard's values.
- Real model and a trivial prompt, not the fake model and the shell prompt; `wall_s = 300`; no tool call was asked for.
- The guard's two experiments (one per harness) became two files, `claude-code.toml` and `opencode.toml`, run one after the other. They differ only in the hook block and the harness name.
- The traces were read as the guard reads them: first event of kind `context_start` in `trace.jsonl`, `system` and `first_message` joined.
- Claude Code puts both instruction texts in the first message, as the probe (round 03) measured, and the hook output in the system text, where the probe's bare `--print` run had it in the first message; G11 accepts either. OpenCode puts all three in the system text.
- The two experiment files are kept beside this report, in [06-live/](06-live/).

## Not covered

No tool-call turn, no replicate beyond one per arm, and no check of the model's behaviour under the instructions: only what each session was given.
