# driver_traits: what the driver traits now carry, and what OpenCode's facts did to them

Leaf L1 of `m4-session-runner`, branch `m4/driver_traits` (base 601ab6c). The traits gained every item D9 lists, each with a default that is not a Claude Code answer; the Claude Code driver implements them; one fixture suite runs on Claude Code and on three fakes, one of which takes the OpenCode facts one by one. No gateway recording was built, and `crates/candler-core`, `crates/candler` are untouched.

## 1. Commits

| sha | subject |
|:--|:--|
| 27d557d | `feat(runner): let a driver supply gateway, state, census and session` |
| 68461be | `test(runner): share one fixture suite across drivers` |
| 8635919 | `feat(adapter-claude-code): add session mode and the new driver facts` |

The leaf's first and third subjects are 80 and 74 characters; SUBJ allows 72 (spec defect, F11). The report commit follows as `reports(runner): record the driver_traits leaf`.

## 2. Gates

| Gate | Result |
|:--|:--|
| K1, both commands | PASS: 10 tests (`candler-harness`), 5 `conformance_*` tests (`candler-harness-claude-code`) |
| Headless command line unchanged | PASS: `guard_claude_code_driver.rs` identical to a26fe53; `binary(guard_claude_code_driver)` 2 passed |
| Reader id unchanged | PASS: no `READER` line in `git diff a26fe53 -- .../src/lib.rs` |
| Workspace with core untouched | PASS: `git diff --quiet 601ab6c -- crates/candler-core crates/candler`; `cargo nextest run --workspace` 451 passed, 19 skipped |
| STD (`-p` forms) | PASS: fmt, clippy `-D warnings` (all targets, feature `conformance`), `cargo doc -D warnings`, 29 tests |
| LINUX (before fix round 1; the workspace form passes now, see the section below) | `-p candler-harness -p candler-harness-claude-code` PASS. The workspace form FAILS in `candler-harness-opencode/src/lib.rs` (doc_markdown on "OpenCode", two lines of the `opencode_facts` stub); not owned, F11 |
| DIGEST | PASS: numstat 0, `guard_cell_digest` 9, core digest binaries 39 |
| SUBJ | PASS after shortening two subjects (`cog check`: no errored commits; all at most 68) |
| OWNED | PASS: every path is under the two crates; `Cargo.lock` untouched |
| SPAWN | PASS before the test runs (`guard_signal`) |
| No `confirm_*` left | PASS |
| `rust-conventions-reviewer` | 7 annotation-placement findings, all fixed; no rule violation of substance |

## 3. Guards (K1)

Each `Guards:` line is in the test's doc. "Red" below means the failing message I saw.

| Id | Guards | Oracle | Bite and failing message |
|:--|:--|:--|:--|
| `conformance_lines_shape` | A Lines reader with an init event passes: sessions, inventory labelled `Harness`, end, collect | hand-written literals (R01 §Ends) | the Lines-only bite below leaves it green, as it must |
| `conformance_document_shape` | A Document reader, no live reader, no init event passes: export fed whole, end from the export | hand-written literals (R01 §Ends, §OpenCode v2) | **Bite 1** |
| `conformance_opencode_facts_shape` | A Document-framed fake with `{env:}`-style names, XDG and `OPENCODE_DB` state paths, a census read from a collected record, an in-container `Exec` pre-stop with `{session_id}`, exit 1 meaning three endings and an authoritative export passes; an empty census record stays unrecorded | the streams recorded by `opencode_facts` (`interrupt_api`, `refused_key`, `completed_o1`) and the report's twelve tool names | **Bite 1**; also an unsafe session id written into the script turns it red: `stop: a session id that could change a script is written into the action` |
| `conformance_claude_code_fixtures` | Claude Code on three recorded streams and sessions c1, c2: sessions, ends, inventory of the first init line, session spec, tracker states after each record | the fixtures' `expected.toml`, and the raw lines read with `serde_json` | reader reports no init: `census (case api_error): expected inventory Some(InitReport { tools: [...Task, Bash...` ; turn end taken as not ended: `tracker: after record 8 expected PendingState { turns: 1, turn_ended: true ...} the tracker reports ... turn_ended: false`; a token in the session argv: `session: a secret value is in the argument vector` |
| `conformance_refuses_session_capability_without_spec` | A session capability without a session spec is refused; the same driver without it passes | the leaf's capability rule | **Bite 2**: with the check removed the test fails (`the suite passed a broken driver`); with it: `the session capability is declared and the driver returns no session spec` |
| `conformance_refuses_census_capability_without_a_source` | A census capability with neither a census stream nor an init report is refused | same | check removed: `the suite passed a broken driver` |
| `conformance_refuses_an_empty_inventory_for_an_empty_record` | A record with no request read as an empty inventory is refused (unrecorded, never an empty set) | the leaf's census rule | comparison skipped for an expected `None`: `the suite passed a broken driver` |
| `conformance_refuses_outside_execution_without_a_pre_stop` | `execution_outside_child` needs a pre-stop | the leaf's rule | check removed: red |
| `conformance_refuses_a_secret_in_argv_and_an_owned_name` | A canary in argv, or a variable the core owns set by a command, is refused | hand-written canary, the driver's reserved names | check removed: red |
| `conformance_requires_the_driver_to_name_its_gateway` | The trait supplies no gateway names | the leaf's "no Claude Code default" | **Claude default in the trait** (`gateway()` returning the `ANTHROPIC_*` names): `the suite passed a driver with no gateway names` |
| `conformance_session_surface_on_a_fake` | A session spec with an `Exec` close, and a tracker over hand-written records, pass; an unsafe id is refused | hand-written records and states | not separately bitten |
| `conformance_claude_session_spec` | The session command is the recorded one, the first message is one stream-json user line, close is end of input, hooks are `Stop` and `SessionEnd` ending in `\|\| true; exit 0` | `command` of `sessions/PROVENANCE.toml` | hook command without `\|\| true`: red (assert on the command's tail) |
| `conformance_claude_hook_command_observes_without_deciding` | The hook command appends one `{written_ns, payload}` line, prints nothing and exits 0 even when the log directory is missing; the tracker reads the line (task, wake-up and cron) | hand-written payload; `sh` as second engine | tracker ignoring the Stop payload: `assertion left == right failed`; hook without `\|\| true`: `assertion failed: output.status.success()` |
| `conformance_claude_tracker_without_a_stop_line` | A turn ended by `error_max_turns` with no Stop keeps the latest `background_tasks_changed` pending; a notification reopens the turn | hand-written lines (session_facts F1) | `background_tasks_changed` ignored: `assertion left == right failed` |
| `conformance_claude_skills_layout` | The skill tree is `.claude/skills/<name>` through the trait | `git archive`, the layout path | not bitten (thin delegation to the guarded functions of `guard_claude_code_driver`) |

**The two bites the leaf names**
1. A suite check that only a Lines reader with an init event can pass (`reader.framing() != Lines || observed.init.is_none()` fails): `conformance_document_shape` turned red with `Failure { case: Some("document_error_result"), check: "bite", detail: "needs a Lines reader with an init event" }`, `conformance_opencode_facts_shape` red on `exit1_interrupted_through_the_api`, `conformance_lines_shape` stayed green. Restored with `git checkout`.
2. A session capability declared without a session spec: refused, see the row above.

Red-first, honestly: I wrote the suite and the fake cases together, so the harness crate's cases were never red before the code; they were red through the bites above and three early failures that were errors in my test data. The Claude Code cases were red before Step 3 (`gateway: the driver names no gateway variables`, the first check that fails), then green once implemented.

## 4. How OpenCode's facts fit each new item

| OpenCode fact | Trait item | Claude Code default doing the work? |
|:--|:--|:--|
| provider through `{env:VAR}` in `opencode.json` | `gateway() -> Option<GatewayEnv>`: names only; the driver's `prepare` writes a configuration that refers to them | No: `None` is the default, and the suite refuses a driver that names none. F2 |
| `HOME`, `XDG_*`, `OPENCODE_DB` | `state_paths()`: `Dir`/`File` with a home-relative subpath; the suite checks unique, valid names disjoint from the gateway's | No: default empty, Claude Code keeps empty |
| no init event, no tool listing | `census_stream()` names a collected record; its offline reader reports `Observed.init`; `InitReport.source` is `Harness` or `Gateway`; a record with nothing leaves `init` `None` | No: default `None` means the live reader's init; `Capabilities.census` stays for a harness with neither |
| skills read from `$HOME/.claude/skills` and others | `candler_harness::skills::SkillTree` is a trait each driver crate implements; nothing in the traits names a layout | No (HOME per run is the core's, state path or isolation) |
| interrupt needs session id, port and password from `/proc` | `Action::Exec` runs inside the isolation; the command finds port and password; `{session_id}` is filled from `Observed::root_session_id`, refused for an id with characters outside `[A-Za-z0-9._-]` | No |
| exit 1 is three endings | `classify_end(exit, sessions, observed)` with `Observed.end` (`Finished`, `Error{kind}`, `Aborted`); default calls `classify_exit` | No: Claude Code leaves `Observed.end` `None` |
| export authoritative | the offline reader's sessions win in `classify_end` (the fake shows it); no trait item needed | n/a |

## 5. Findings

| # | Finding | Flag |
|:--|:--|:--|
| F1 | **Census source.** An inventory is labelled `CensusSource::Harness` (init event, Claude Code) or `Gateway` (collected record, OpenCode); labels differ so two inventories never compare equal, and `None` is "unrecorded". `InitReport.mcp_servers` is `Option`: a gateway record sees tool names, not servers, so OpenCode's servers are unknown, not an empty list. The core's census (`runner/evidence/census.rs`) requires both lists and treats a missing one as malformed, so L2 must record `census.tools` and `census.init_tools` without `census.mcp_servers` for that source. No recording was built. | **Request** to the coordinator: L2 census mapping; D9's census rule stays pending Eliott |
| F2 | `gateway()` returns `Option<GatewayEnv>`, not `GatewayEnv`: a bare return needs a default, and any default is Claude Code's. Until L2 the core still hard-codes the `ANTHROPIC_*` names. Also: the core's run base URL ends at `/run/<key>`, while `opencode_facts` row 7 puts `/v1` at the end of OpenCode's URL. Its `prepare` must write `{env:NAME}/v1`; the facts show whole-value substitution only, so inline concatenation is unproven. If it fails, `GatewayEnv` needs a path suffix. | **Deviation** from the leaf's signature; **request**: O1 must confirm inline `{env:}` |
| F3 | `session` is `session(cell, workspace) -> Result<Option<SessionSpec>, DriverError>` and `SessionSpec` carries the session `command`: the argv differs from headless (`--input-format stream-json`, `--settings`), so input and close alone cannot run a session. Added `HarnessDriver::session_tracker()`, because the pending parse is stateful (latest Stop wins, else latest task list). Hook paths are relative to the agent's home, not the workspace. `--settings` takes the absolute `/home/agent/.candler/settings.json`, which duplicates the core's `CONTAINER_HOME`. | **Deviation** and shared-name additions; **request**: a `{home}` placeholder if the core's home ever moves |
| F4 | `classify_end` was added next to `classify_exit`, which stays: the core's test fakes implement it, and changing its signature breaks them. L2 or L3 must make the core call `classify_end` and pass the live reader's `observed()`. | **Deviation**; core call is a request |
| F5 | The suite's rule is the leaf's: `execution_outside_child` implies a pre-stop. It does not forbid a host-side `Http` pre-stop for a server inside the container (the first fake uses one). That the pre-stop must run in the isolation is W1's bite, not a trait guarantee. | Note |
| F6 | A root session id with a character outside `[A-Za-z0-9._-]`, or a child id (`/`), is refused by `with_session_id`; the core must fall back to the signal. The same holds when no id has been observed yet: the id is on the first stdout line, so a stop before it has nothing to interrupt. | **Request** to L3 |
| F7 | The recorded sessions c1 and c2 hold no pending wake-up or cron. The Stop payload's `session_crons` mapping (`recurring:false` is a `WakeUp`) is covered by a hand-written payload only. After a turn with no Stop line (`error_max_turns`), wake-ups and crons are not in the stream, so the tracker keeps those of the last Stop, which can be stale. `PendingKind::Monitor` is never produced: Stop and the task-list event do not tell a monitor from a job. | **Request**: `session_runner` decides how to read this; a live run with a cron would close the gap |
| F8 | `Capabilities` of Claude Code now says `census` and `session`. `census` was off before although the core computes one; nothing in `candler-core`, `candler` reads the flag. `session` is what D1's refusal will read. | Behaviour of a flag only; no digest, argv or id moved |
| F9 | The hook command stamps the line with `date +%s%N`, which GNU `date` (the image) supports. The hook test runs it with the host's `sh` and `date`; it passed on this macOS host. | Note |
| F10 | "A cwd" in Step 2's list of command invariants is read as: a working directory, when set, stays inside the workspace. No driver sets one. | Interpretation |
| F11 | **Spec defects.** The leaf's subjects for Step 1 and Step 3 are 80 and 74 characters, over SUBJ's 72: shortened. The workspace LINUX gate fails on `crates/candler-harness-opencode/src/lib.rs` (the `opencode_facts` stub), where clippy wants backticks around OpenCode in two doc lines; not an owned path. | **Spec defect**; **request**: fix in `opencode_driver` |
| F12 | `conformance_shapes` has `required-features = ["conformance"]`, so a plain `cargo check --all-targets -p candler-harness` builds. K1 passes the feature, and a workspace or `-p` run that includes the Claude Code crate enables it through its dev-dependency. `cargo nextest run -p candler-harness` alone skips that binary. | Note |
| F13 | The Claude Code reader reports `Observed.init` only when the first init line lists tool names; `mcp_servers` absent gives `None`, where the core's census would call the line malformed and record nothing. For every init line of every fixture the two agree. H3 (L2) compares them. | Note |
| F14 | `guard_two_shapes.rs` keeps its three tests; its fakes moved to `tests/support/mod.rs` and gained gateway names, which the suite requires. Nothing it asserts changed. | Note |

## 6. Cleanup (first round)

The target dir `/private/tmp/claude-501/m4-target-driver_traits` and the scratch dir `/private/tmp/claude-501/m4-scratch-driver_traits` are removed before this report is returned. No Docker, no model, no download was used.

## 7. Fix round 1

Branch rebased on the milestone tip 66224ea; the three code commits are now 27d557d, 68461be and 8635919, and the report commit follows them. Every fix has a guard and a bite, run against the committed code with the file restored from a copy afterwards. Honest note on order: I added the types first and wrote each guard against them, so a guard was never red before its type existed; each was red under its bite (below), and the new suite checks each have a negative test that fails when the check is removed.

### Gates after the round
| Gate | Result |
|:--|:--|
| K1, both commands | PASS (17 and 8 `conformance_*` tests) |
| Headless command line, reader id | PASS (test file identical to a26fe53, 2 passed; no `READER` line in the diff) |
| Core untouched | PASS (`git diff --quiet 66224ea -- crates/candler-core crates/candler`); `cargo nextest run --workspace` 461 passed, 19 skipped |
| STD (`-p` forms) | PASS: fmt, clippy, doc, 39 tests |
| LINUX, workspace form | PASS |
| DIGEST | PASS |
| SUBJ | PASS (`cog check 66224ea..HEAD`, all subjects at most 72) |
| OWNED | PASS: only the two crates and this report; `Cargo.lock` untouched |
| SPAWN, `confirm_*` | PASS |

### Fixes
| # | Fix | Guard (all `Guards:`/`Oracle:` in the test doc) | Bite and failing message |
|:--|:--|:--|:--|
| M1 | `PendingState.known`, false until a readable source is seen and after any unreadable list or entry, which keeps the last readable list; an id-less item is unreadable, on the Stop path and the stream path; documented on the type that the core must not close while it is false | `conformance_claude_unreadable_pending_work_is_never_nothing` (a result with no Stop and no task list; a malformed entry, an empty id, a renamed entry key, a renamed list key, a list that is a string; a Stop with an id-less item, an empty cron id, a missing list; recovery by a readable list). The recorded c1 and c2 states carry `known` too | unreadable list read as nothing: `assertion left == right failed: {...background_tasks_changed","tasks":[{"task_id":"b8"},{"run_id":"r"}]}`; empty id admitted: same with `"task_id":""`; `known` always true: red in this test and in `conformance_claude_code_fixtures` (`after record 0 expected ... known: false`) |
| M2 | `PreparedFile` has a root, `FileRoot::{Workspace, Home}`, default Workspace; `PreparedFile::in_home`; `HookSpec.files` are home-rooted. The `OpenCode`-shaped fake's `prepare` writes `.xdg/config/opencode/opencode.json` in the home, with `{env:<base var>}/v1` and `{env:<token var>}` taken from its own `gateway()`. The suite refuses a hook file not rooted at the home, and any written file whose `{env:NAME}` is not declared (gateway names, reserved names, state-path variables) | `conformance_refuses_a_config_naming_an_undeclared_variable`; the home-root rule through `conformance_claude_session_spec` and `conformance_claude_code_fixtures` | undeclared reference accepted: `the suite passed a broken driver`; Claude hook files written to the workspace: `session: hook file .candler/settings.json is not rooted at the agent's home` |
| M3 | `CollectSpec::Exec`, in the isolation, inherits the run's `state_paths` environment (documented on the variant). The fake collects with `opencode session export --standalone <id>` through it. The suite refuses a host `Command` collect for a driver that declares state paths | `conformance_refuses_a_host_collect_for_a_driver_with_state`; the collect expectation of the fake's cases | check removed: `the suite passed a broken driver` |
| M4 | The Document-shaped fake reads the recorded `completed_o1/export.json`, `interrupt_api/export.json` and `refused_key/export.json` (the real `{info, messages}` shape). Ids, outcomes, exit statuses and the terminal error type are read from each case's `expected.toml` and `exit.toml`. One case combines the recorded `completed_o1` stdout (live end finished) with the `interrupt_api` export (interrupted): the export wins. The suite's `Case` has an export that classifies the run | `conformance_opencode_facts_shape` | export ignored: `export_wins_over_the_live_end: expected Interrupted, the driver classifies Completed`; export outcome read as unknown: `export_completed_o1: expected [... Completed], the reader finished with [... Unknown]` |
| L1 | Claude Code's three names pinned, and no state path | `conformance_claude_gateway_names_are_the_cores_names` (oracle: `run.rs` at a26fe53, by hand) | a typo in the token name turns it red |
| L2 | `InitReport::new` refuses an empty tool list from the gateway and collected sources (`InitReportError::NoTools`); the harness's own empty init list stays valid. The fake's census reader takes the first request that carries a non-empty `tools` array | `conformance_inventory_sources_are_labelled_and_never_empty`; cases `census_skips_a_tool_less_first_request`, `census_record_of_tool_less_requests`; `conformance_refuses_an_empty_inventory_for_an_empty_record` now breaks the reader into a phantom inventory | refusal removed: `assertion left == right failed`; first request taken whole: `census_skips_a_tool_less_first_request: expected inventory Some(InitReport {...tools [edit, glob...` |
| L3 | `CensusSource::Collected`; a declared census with a census stream needs an offline reader for it; the suite takes `Gateway` or `Collected` for the stream's reader and `Harness` for a live one | `conformance_refuses_a_census_stream_with_no_reader`, and the label test above | check removed: the test fails on its message assertion |
| L4 | `Action::resolve(Option<&SessionId>) -> Option<Action>`: the action itself when it needs no id, the filled action for a plain id, `None` for an unknown or refused id; the suite uses it for the pre-stop and the close. The fake's script finds the server by scanning `/proc/<pid>/cmdline` for `opencode serve `; the suite refuses `pgrep -f` and `pkill -f` in a command | `conformance_action_resolve_falls_back_when_the_id_is_unknown`; `conformance_refuses_a_stop_script_that_matches_itself` | resolve returning the unfilled action: `assertion failed: filled.as_ref().is_some_and(...)`; `pgrep -f` accepted: `the suite passed a broken driver` |
| L5 | `state_paths` refuses a duplicate or nested subpath; the fake lists all four XDG directories and puts the database file in its own directory (`.xdg/db/oc.db`, not inside the data directory); the session command keeps every element of the headless argv but the prompt | `conformance_refuses_nested_and_duplicate_state_paths` (nested file, duplicate, and siblings `data` and `database` accepted); `conformance_claude_session_command_keeps_the_headless_arguments` (a cell with effort, turn and cost caps) | nesting check removed: `the suite passed a broken driver`; `--verbose` dropped from the session command: `the session command lost --verbose` |

### Changes to the findings of section 5
- F1: `InitReport.source` now has three labels (`Harness`, `Gateway`, `Collected`), and a gateway or collected inventory is never empty.
- F2: the fake now writes `{env:<base var>}/v1` as the ruling says; V2's behaviour with an inline suffix is `opencode_driver`'s to confirm.
- F3: `HookSpec.files` and `PreparedFile` are rooted explicitly; `--settings` still carries the absolute `/home/agent/...` (unchanged).
- F4: switching the core to `classify_end` stays with `harness_out_of_core` (coordinator's ruling).
- F6: `Action::resolve` is the core's tool for the fallback.
- F7: unchanged for wake-ups and crons after a turn with no Stop line, but the state now says so: `known` is false until a list was read.
- F11: the workspace LINUX gate passes on the tip.
- Superseded by round 2 (R2): `OPENCODE_DB` is back at `.xdg/data/oc.db` in the fake, inside the data directory as the recorded `debug paths` run has it; a file inside a listed directory is allowed.


## 8. Fix round 2

The three code commits are now 6ae2247, 0c93f88 and 7d5b2ef (each fixed up in place and autosquashed on 66224ea); this report follows them. Each bite below was injected into the committed code, seen red, and the file restored from a backup copy (`cmp` clean). As in round 1, the checks were written against the types and then bitten; the Claude Code R1 tests were red against the old tracker before the fix.

### Gates after the round
| Gate | Result |
|:--|:--|
| K1, both commands | PASS (17 and 8 `conformance_*` tests) |
| Headless command line, reader id | PASS (test file identical to a26fe53, 3 passed; no `READER` line in the diff) |
| Core untouched | PASS (`git diff --quiet 66224ea -- crates/candler-core crates/candler`); `cargo nextest run --workspace` 461 passed, 19 skipped |
| STD (`-p` forms) | PASS: fmt, clippy `-D warnings` (all targets, all features), `cargo doc -D warnings`, 39 tests |
| LINUX, workspace form | PASS: `cargo clippy --workspace --all-targets -- -D warnings` |
| DIGEST | PASS: no digest file changed, `guard_cell_digest` 9 |
| SUBJ | PASS (`cog check 66224ea..HEAD`, longest subject 68) |
| OWNED | PASS: only the two crates and this report; `Cargo.lock` untouched |
| Per-commit build | PASS: `cargo check --all-targets --all-features` at each of the three code commits |

### Fixes
| # | Fix | Guard | Bite and failing result |
|:--|:--|:--|:--|
| R1 | The tracker keeps `tasks_known` and `timers_known`; `known` is their conjunction. `timers_known` is set only from a Stop payload whose `session_crons` and `background_tasks` both read; an unreadable list clears its own flag. A non-JSON or truncated line that mentions `background_tasks_changed` clears `tasks_known`. The `PendingState` doc states the consequence: an error turn with no Stop keeps `known` false, so the session runs to its cap | `conformance_claude_unreadable_pending_work_is_never_nothing`: (a) a Stop with `session_crons:[{"x":1}]`, then a readable `tasks:[]`, stays unknown; (b) an assistant line, an empty task list, a result, no Stop ever: unknown; (c) a cut line, and a text line containing the name, clear the flag and keep the last list; a plain non-JSON line changes nothing. The c1 checkpoint and the no-Stop test now expect `known=false` | `known` as a disjunction: red in this test; tasks line no longer clearing the flag: red (case c); an unreadable Stop list no longer clearing `timers_known`: red (case a) |
| R2 | `state_paths` refuses a duplicate subpath and a `File` that is another path's parent; a `File` inside a `Dir`, and siblings such as `data` and `database`, pass. The rule is in the `state_paths` doc. The fake's `OPENCODE_DB` is `.xdg/data/oc.db` again | `conformance_refuses_duplicate_state_paths_and_a_file_that_is_a_parent` (duplicate; file as parent in both orders; file in dir and siblings accepted) | file-as-parent check removed: `the suite passed a broken driver`; the old rule (any nesting) restored: red on the accepted file-in-dir case |
| R3 | The pgrep and pkill lint splits the command line into commands and refuses `--full` or any single-dash cluster that contains `f` after a `pgrep` or `pkill` word (`-af`, `-fl`, `-9f`, `pkill -9 -f`); `-x`, `-l` and `echo -f pgrep` pass. `InitReport::new` refuses an empty tool name (`InitReportError::EmptyToolName`, any source) | `conformance_refuses_a_stop_script_that_matches_itself`; `conformance_inventory_sources_are_labelled_and_never_empty` | cluster test disabled: `the suite passed a broken driver`; empty-name check disabled: `assertion left == right failed` |
| R4 | The `CollectSpec::Exec` doc says the command inherits the whole run environment, including the gateway token variable, and its output is archived raw, so it must not print its environment | doc only | none |

### Notes
- Opencode's recorded `debug paths` layout is accepted again, so `opencode_driver` may keep the database in the data directory.
- The conservative effect of R1 on the core: a Claude Code session whose turn ended in an error with no Stop line stays `known == false` and runs to its cap rather than closing early. That is the intended direction (never close on unread pending work).

The target dir `/private/tmp/claude-501/m4-target-driver_traits` and the scratch dirs are removed before this report is returned.
