# M6.5 closing review against its gates (v0)

**Verdict: the milestone holds. All 13 gates hold on the tip, none fails; no defect blocks the merge. Three small items are in scope to fix (F1 to F3), the rest are debt or notes.** Read-only review of `528cfef3` against base `8d82f63b`. No test or build was run; the 767/767 pass and the Docker guards were taken as given. "Absent on base" was read from the diff, not by building the base.

## Gates

| Gate | Holds | Evidence on tip | Absent on base |
|---|---|---|---|
| G1 | yes | `guard_agent_config_e2e` (`guard_cli_e2e/agent_config_e2e.rs:277`, `resolve()` at :187, `record["instructions"]`, `["hooks"]` at :334); `inspect_shows_the_mcp_server...` sibling at `guard_inspect_mcp_config.rs:124` | `instructions`, `hooks` not in `CONFIGURATION_PATHS` (`resolve/paths.rs` diff): `unknown_path` |
| G2 | yes | `two_levels_of_instructions_or_hooks_plan_two_cells` (`core/tests/it/guard_agent_config.rs:97`), counts cells and checks a level keeps its digest | same paths unknown |
| G3 | yes | rows at `docs/contract/harness-capabilities.md:51-53,62`, both harnesses Mapped (`claude_code driver.rs` `answer`/`fact`, `opencode driver.rs`); contract_pages guard | no rows, no `AgentSetting::Instructions/Hooks`, no `DriverFact::ConfigCheck` (`spec.rs` diff) |
| G4 | yes | e2e :344-355 (global and own arm present, other arm absent, in `system` or `first_message`); live: report 06 table | no `context_start` kind (`trace.rs` `KnownKind`) |
| G5 | yes | e2e :399-407: the fake model's recorded answer to `toolu_fake_hook` is an error carrying `BLOCKED` on both harnesses, never `hook-not-blocked`. Not exercised live (report 06, deviations) | no `h1` script in `fakemodel.py`, no hook install |
| G6 | yes, on the message | `guard_harness_check_refuses` (:454): invalid payload gives `<harness> rejects it: ` plus the quoted harness text and 0 sealed runs; valid payload seals. Core: `harness_finding_refuses_the_job_before_any_run` (`guard_config_check.rs:365`). Code not read back (O11, filed) | no `check_config` (`orchestrate/config.rs` is new) |
| G7 | yes | e2e :365-377 (exact two entries, scope `user`/`project`, path suffix); `the_census_names_the_instruction_files_the_hook_log_shows` (CC driver guard :757), fixture `census_instructions.jsonl` (OpenCode) | no `census.instructions` (`census.rs` diff) |
| G8 | yes | `misused_hooks_and_instructions_are_refused_by_name` (`guard_agent_config.rs:143`): five cases, code and pointer asserted, incl. `..` path | `unknown_path` |
| G9 | yes, weaker than worded | `workspace_file_is_mounted_read_only_and_is_no_output` (`guard_config_check.rs:537`): mount mode `ReadOnly`, not in `tree.json`, not `input_leak`; e2e :379-397 on real runs. No test makes an agent write to the file, so "a write fails" rests on the Docker `ro` flag (F4) | `Plan` had no `workspace_files` |
| G10 | yes | `planning_refuses_an_input_at_the_workspace_instruction_file` (`guard_agent_config.rs:223`: case, `./`, directory below, near-misses `docs/CLAUDE.md`, `CLAUDE.mdx`, no text); CLI: `guard_instruction_collision.rs:20`. Claude Code only; the `AGENTS.md` side is not exercised | no `InstructionsCollide`, no `input_targets` |
| G11 | yes | e2e :360 (marker in `system` or `first_message`); live: marker in `system` on both harnesses | no event |
| G12 | yes | `trace_context_first_event_is_the_first_request_with_tools_on_both_harnesses` (`guard_trace_context_start.rs:178`: seq 0, equals the gateway line of the lowest tooled seq, ends in `go`); `..._a_trace_stored_without_the_event_still_reads` (:201, round-trips bytes); reader test :227 | no `ContextStart`, no `context` log line (`gw.js` diff) |
| G13 | yes | `06-findings_live_confirmation_v0.md`: 4 runs, both arms, both harnesses, excerpts quoted, `harness_config_check = clean`, `dirty = false` | no event |

No gate passes for the wrong reason in a way that hides a failure. The oracles are the literal texts the experiment fixes, the gateway's own stored line, and the harness output recorded in fixtures (`PROVENANCE.toml`). The one oracle copied from the implementation is G12's comparison with the gateway line (the gate asks for exactly that).

## Findings, ranked

**Defects**
- F1 (in scope, small). `experiment.instructions_collide` always points at `/fixed/instructions.workspace` (`runner/plan.rs`, `refuse_instruction_collision`, `pointer_of(["fixed","instructions.workspace"])`). Architecture v2 says "or the factor's path". Scenario: `instructions.workspace` is a factor (the intended use). The refusal names a pointer that does not exist in the file; the consumer is sent to the wrong place. The guard uses a fixed value, so it cannot see it.
- F2 (in scope, small). OpenCode's `read_config_check` refuses only `level=WARN` lines (`opencode driver.rs`, after `is_drain`). Eliott's ruling is errors and warnings. The narrowing is deliberate (environment `ERROR` lines on valid cells, round 05), but no fixture shows a plugin that fails in `setup()` or a bad hook name, so whether a consumer-file fault ever logs at `ERROR` is unmeasured. A plugin that throws in `setup` and logs `ERROR` would run unchecked. Fix: record that case in a fixture, or state the limit on the capability page.
- F3 (in scope, small). `HARNESS_IDS` (`candler-harness/src/spec.rs`) is a second list of the shipped harnesses beside `crates/candler/src/drivers.rs` `BUILTIN`. A third driver in `BUILTIN` has `hooks.<id>` and `instructions.global.<id>` refused as "names no registered harness" until someone edits the other list. Derive one from the other, or add a guard that they agree.

**Debt (in scope, small)**
- F4. G9's "a write to it fails" has no test that writes. Add a step to the fake model's `h1` script, or state in the gate that the proof is the mount flag.
- F5. `__reports__/agent_config/06-findings_live_confirmation_v0.md` says "G1's blocked-hook marker" (should be G5). Its `06-live/*.toml` carry `source = /Users/hacker/Documents/tmp/claude-worktrees/Candler/m6_5-scratch/live/treatment`, a scratch path that is deleted, so the two experiments cannot be rerun as kept.
- F6. Stale prose: `__design__/architecture.md:682` (Prepare row: "instructions written into the workspace"; now the global file goes to the home and the workspace file is a read-only mount). Architecture v2 and `installed/proved/e2e_guard.md` name the live report `04-findings_live_confirmation_v0.md`; it is `06-...`. `__reports__/agent_config/README.md` Status first paragraph still says "Level 1 is dispatched".
- F7. `context_start` is placed on the root session only (`gather.rs` `with_context_start`), and the gateway keeps one record per run key. Gate wording, leaf `trace_context` ("every session") and invariant 8 say every session. A Claude Code subagent session has none. Reword the three, or file as an observation.

**Notes (out of scope, observations)**
- N1. `input_leak` is checked against `cell.inputs` only (`exec/mod.rs`, `input_leak(&evidence, &cell.inputs)`), while the tree is built with the mounted files. An agent that copies the workspace instruction file into its outputs is neither collected under that name nor sealed; the architecture's risk row expected `input_leak`. Low impact, since the text is the arm's own treatment, but the risk row is wrong.
- N2. OpenCode's census reads `Instructions from: <path>` at line start from the system text, unescaped (documented in `loaded`): an instruction text holding such a line adds a census entry. The consumer writes that text, so only a self-inflicted false entry.
- N3. The gateway log line is 15,500 bytes (CC) and 8,943 (OpenCode) with the fake model; a real system text is larger and Docker splits log lines at 16 KiB. The live run read it whole, so it holds, but the size on the trace page is the fake model's.
- N4. The sidecar image tag now follows the sources (`sidecar_tag`), so each `gw.js` change leaves an older `candler-gateway:src-<digits>` image behind. Same as the base image today; no cleanup exists.

## Consumer breakage checked

- A refusal blaming the wrong side: a check that printed no report is `jobs.config_check_failed` (environment); a finding is `experiment.harness_rejected_input`. Non-report output on Claude Code still goes out as `Rejected` (O in round 05, filed).
- A value not in the digest: all four values are cell values, so the digest and `inspect` cover them (G2, G1). The check verdict is provenance, not the digest; correct.
- A file collected that should not be: none found. `.claude/settings.json`, `.claude/hooks/*`, home files and the root file are all absent from outputs in the e2e guard (:379).
- A cell a valid experiment cannot express: an arm with no workspace text next to an arm with text cannot omit the key in a factor level; the workaround is an empty string, which mounts an empty root file. Not tested; note for the consumer page.
