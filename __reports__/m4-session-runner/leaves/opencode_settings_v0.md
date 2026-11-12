# `opencode_settings` leaf report

Branch `m4/opencode_settings`, cut from `feat/m4-session-runner` at `03900ae`.

## Commits

| sha | subject |
|---|---|
| `990318d` | `test(adapter-opencode): record effort and step-cap on the fake model` |
| `435bb8b` | `test(runner): fold m4's driver tests into table-driven guards` |
| `427c414` | `feat(adapter-opencode): map the settings opencode can honour` |

(A fourth commit, this report, follows: `reports(adapter-opencode): record the opencode_settings leaf`.)

## Equivalence table

One row per cell setting the Claude Code driver and R01 give meaning to today. "Mapped" means the OpenCode 2.2.7 construct named reproduces that meaning; "Refused" means `DriverError::Unsupported` (`experiment.setting_unsupported`), naming the path.

| Setting | Candler's meaning | OpenCode 2.2.7 construct | Mapped / Refused | Why |
|---|---|---|---|---|
| `model.effort` (`low`/`medium`/`high`/`xhigh`/`max`) | Claude Code's `--effort <level>` | A config-declared `models.<model>.variants` entry on the model (`packages/schema/src/config/provider.ts::ModelConfig.variants`, v2.2.7), used verbatim, bypassing the models.dev catalog lookup that a custom `candler` provider cannot reach anyway | **Mapped** | The driver writes `variants: [{id: level, settings: {thinking: {type: "adaptive", display: "summarized"}, effort: level}}]`, the same shape `anthropicMessages()` resolves a catalogued Claude model's effort to (`packages/core/src/variant.ts:325-339`). Proven on Candler's wire: `tests/fixtures/streams/oc_effort_variant/wire.jsonl` shows every request of that session carrying `thinking: {type: "adaptive", display: "summarized"}`; `effort` itself is consumed by OpenCode and never reaches the wire (`wire_effort_field_present` false in the same file; also asserted by `opencode_wire_effort_variant_reaches_the_request_body`). A level outside the five names is `InvalidValue`, not a refusal (OpenCode has no 6th level to ask for). |
| `budget.per_run.turns` | Claude Code's `--max-turns` | none equal: `agents.<id>.steps` makes the last request tool-less with an injected prompt and ends the run `succeeded` | **Refused** (fix round 1, B5) | See Fix round 1. The old `Mapped` row and its wire guard are gone; `oc_step_cap` stays as the evidence for the refusal. |
| `budget.per_run.max_cost_usd` | Claude Code's per-run dollar cap | none found | **Refused** | No cost-cap mechanism exists in the v2.2.7 schema or source (confirmed by the facts probe and re-confirmed here; nothing in `schema/src/config/*.ts` names a cost limit). |
| `tools.allowed` / `tools.disallowed` | Claude Code's tool allow/deny lists | `permissions` rows `{action, resource: "*", effect}`, last match wins, on a baseline of `{*, *, allow}` | **Mapped, per name** (rewritten in fix round 1, B2, M1, L1, L2) | See the Fix round 1 tool table: `Write` is `edit`, MCP names are normalised, deny rows come last, a lone `Edit`/`Write` in the deny list is refused. The old text said an unmatched action asks; it does not (baseline allow). |
| `permission_mode: bypassPermissions` | Claude Code's `--dangerously-skip-permissions`: nothing is asked | `--auto`: auto-replies every **pending `ask`** with "once" | **Mapped** | `denied()` (an explicit `deny` match) is checked **before** any `ask` is even published (`packages/core/src/permission.ts:176-178, 262-269`), using only the agent's own and the session's rules — `--auto` cannot reopen it. Since the driver adds no `deny` rule for this mode, nothing is blocked: `--auto` alone reproduces "nothing is asked." |
| `permission_mode: plan` | Claude Code's plan mode: reads and read-only shell | none equal | **Refused** (fix round 1, B3) | The `--agent plan` mapping was not read-only (a global rule follows the agent's rules, and the `subagent` child can edit). See Fix round 1. |
| `permission_mode: default` / `acceptEdits` | Claude Code headless: unapproved calls are denied | No `--auto` also rejects unmatched asks (`oc_permission_ask`) | **Refused** (fix round 1, M2) | The earlier reason (no rule list can reproduce the two tiers) was false as stated; the true reasons are in Fix round 1. |

## Answers to the coordinator's three questions

1. **Does a deny rule bind under `--auto`?** Yes, unconditionally. `Permission.denied()` is checked first, using only the agent's and the session's own rules (never the "always"/saved rules), *before* any `ask` event is even published (`packages/core/src/permission.ts:176-178, 262-269`). `--auto` (`packages/cli/src/run/noninteractive.ts:97-112`) only auto-replies a *pending* `ask` request with `"once"` — it has no path that touches a `deny` match at all. The fact stands; the `plan` mapping that used it is withdrawn.

2. **Superseded by fix round 1 (answer was wrong).** See the Fix round 1 section.

3. **Superseded by fix round 1 (answer was wrong).** See the Fix round 1 section.

## Test counts, before -> after

| File | Before | After |
|---|---|---|
| `crates/candler-harness-opencode/tests/guard_opencode_driver.rs` | 28 | 15 (14 real + `#[ignore]`'d `opencode_real_binary_is_interrupted_by_the_stop_script`, unchanged; `opencode_stop_interrupts_inside_the_container_then_terms` dropped per the brief — its script-content checks were purely textual, and its resolve/session-id assertions are already covered generically by the shared conformance suite's `check_action`/`check_stop`) |
| `crates/candler-harness/tests/conformance_shapes.rs` | 17 | 5 (`conformance_lines_shape`, `conformance_document_shape`, `conformance_opencode_facts_shape`, `conformance_session_surface_on_a_fake` unchanged; the 13 "refuses a broken driver" checks folded into one `conformance_refuses_a_broken_driver`) |
| `crates/candler-harness-claude-code/tests/conformance.rs` | 8 | 7 (fixtures, session spec, hook command, skills layout, gateway names, session command unchanged; the tracker test and the pending-work test folded into one `conformance_claude_tracker_and_pending_work`) |

Step 2 (`435bb8b`) made none of these changes behavioural: all three files, both old and new shape, were run and green against the **original** (Step 1) driver code before Step 3 began.

`guard_opencode_driver.rs` also gained two new tests in Step 3: `opencode_wire_effort_variant_reaches_the_request_body` and `opencode_wire_step_cap_makes_exactly_n_tool_bearing_requests_and_none_after` (the `opencode_wire_*` O1 gate), reading the fixtures' own committed `wire.jsonl` rather than relying on source citation alone.

## One recorded bite per merged guard

Each bite: a one-line source change, the test run that went red with its message, then the exact revert and a green re-run. All seven are already reverted; `git diff` shows none of them.

1. **`opencode_command_line_maps_or_refuses_every_setting`** (merges the command-line mapped/refused rows) — changed `("Bash", "shell")` to `("Bash", "shell2")` in `prepare.rs`'s `TOOL_ACTIONS`. Red: `assertion left == right failed ... left: ... "shell2" ... right: ... "shell"`.
2. **`opencode_prepare_writes_the_provider_mcp_codemode_and_gateway_names`** — flipped `server()`'s default `codemode` from `None => true` to `None => false`. Red: the written config's `codemode` for a server with no explicit flag no longer matched the expected `true`.
3. **`opencode_export_reader_covers_trace_errors_cache_cost_and_child_sessions`** — swapped the `cache("read")`/`cache("write")` argument order in `export.rs`'s `usage_of`. Red: `assertion left == right failed, left: Number(11), right: 4704` (the cache token counts came out transposed).
4. **`opencode_end_is_the_export_whatever_the_order_or_unknown_with_no_trace`** — inverted the reader-match condition (`==` to `!=`) in `driver.rs`'s `root_end`. Red: `live completed_o1, export interrupt_api ... left: Completed, right: Unknown`.
5. **`opencode_stop_terms_after_the_interrupt_and_matches_only_the_server_child`** — set `client = True` in `stop.py`'s `is_server`, disabling the parent-is-the-client check. Red: the predicate table came back `[true, true, false, false, false, true, true, true]` against an expected `[true, true, false, false, false, false, false, false]` — three decoys (an `sh -c` mention, a `timeout` wrapper, a server whose parent is not the client) now wrongly matched.
6. **`conformance_refuses_a_broken_driver`** (shared `candler-harness/src/conformance.rs`) — made `check_names` return `Ok(())` instead of refusing a driver with no gateway names. Red: `Error: "the suite passed a driver with no gateway names"`.
7. **`conformance_claude_tracker_and_pending_work`** (claude-code crate) — changed the `background_tasks_changed` key `"task_id"` to `"task_id_x"` in `candler-harness-claude-code/src/session.rs`. Red: `pending: []` where `pending: [PendingItem { kind: Task, id: "b9" }]` was expected.

## Gates

All run with `RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/private/tmp/claude-501/m4-target-opencode_settings` (target dir deleted after this report was written).

- **K1** (`cargo nextest run -p candler-harness --features conformance --no-tests=fail -E 'test(/^conformance_/)'` then `-p candler-harness-claude-code -p candler-harness-opencode ... `): `5 tests run: 5 passed, 5 skipped` / `8 tests run: 8 passed, 25 skipped`.
- **O1** (`cargo nextest run -p candler-harness-opencode --no-tests=fail -E 'test(/^opencode_/)'`): `14 tests run: 14 passed, 2 skipped`.
- **Wire** (`... -E 'test(/^opencode_wire_/)'`): `2 tests run: 2 passed, 14 skipped`.
- **Claude Code unchanged**: `git diff --quiet c4d1559 -- crates/candler-harness-claude-code/src crates/candler-harness-claude-code/tests/guard_claude_code_driver.rs` passed (no diff); `cargo nextest run -p candler-harness-claude-code --no-tests=fail -E 'binary(guard_claude_code_driver)'` → `3 tests run: 3 passed, 0 skipped`.
- **Core/binary/docs untouched**: `git diff --quiet c4d1559 -- crates/candler-core crates/candler docs` passed (no diff).
- **Workspace**: `cargo nextest run --workspace --no-tests=fail` → `631 tests run: 631 passed (1 slow), 22 skipped`.
- **STD** (`-p` forms for the three touched crates): `cargo fmt --check` passed; `cargo clippy --all-targets -- -D warnings` passed; `cargo nextest run --no-tests=fail` → `42 tests run: 42 passed, 1 skipped`; `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` → `Generated .../doc/candler_harness/index.html and 2 other files`.
- **LINUX** (`cargo clippy --target x86_64-unknown-linux-gnu -p candler-harness --features conformance -p candler-harness-claude-code -p candler-harness-opencode --tests -- -D warnings`): `Finished` dev profile target(s), no warnings.
- **DIGEST**: not applicable — this leaf touches no oracle vector or cell digest fixture.
- **SUBJ** (`cog check 03900ae..HEAD && git log --format=%s 03900ae..HEAD | awk 'length>72{b=1}END{exit b}'`): passed after a correction (see Findings).

## Findings

1. **Spec defect: the leaf's own Step 1 commit subject exceeded SUBJ's 72-character limit.** The leaf file's `**Commit**:` field for Step 1, `test(adapter-opencode): record effort and step-cap runs on the fake model`, is 73 characters. I had committed it verbatim in an earlier phase of this dispatch. Caught by SUBJ; fixed by reconstructing the three-commit history with a shortened, meaning-preserving subject (`test(adapter-opencode): record effort and step-cap on the fake model`, 68 characters) — the fixture content, the Step 2 consolidation, and the Step 3 mapping are each still their own commit with the file sets unchanged, only the one subject line differs. No behaviour changed by this reconstruction; the full gate suite was re-run and green at every intermediate commit.
2. **`opencode_wire_*` gate required a fixture addition beyond the original Step 1 recording.** The leaf's own Success Gates list `cargo nextest run -p candler-harness-opencode --no-tests=fail -E 'test(/^opencode_wire_/)'` as a required gate, proving the effort and turn-cap mappings "on the wire" from committed evidence, not source citation alone. The streams originally recorded (`oc_effort_variant`, `oc_step_cap`) had their `stdout.jsonl`/`export.json` committed but not the fake model's own request-body records, which are the only place the wire fact lives; a non-committed `records.jsonl` (noted "kept outside the repository") couldn't back an automated test. I re-recorded both streams (Docker rebuilt from the same `base-opencode` image, same fake model script, new session ids; `oc_effort_variant` this time also used the driver's actual adaptive-thinking shape rather than a hand-invented placeholder) and committed a new, redacted `wire.jsonl` per fixture (model id, `tool_choice`, whether `tools` was offered, and for effort whether `thinking`/`effort` reached the wire — no header, no secret, no message body), then added the two `opencode_wire_*` tests that read it.
3. **Rust-conventions review (read-only subagent) findings not acted on.** Two real issues were fixed (missing `# Errors` on `render_config`; a doc-comment code span broken across a line, in `prepare.rs`'s `EFFORT_LEVELS` comment). Left open, by the reviewer's own assessment low-severity and consistent with pre-existing code in the same file: a `.as_str()`/`.to_owned()` pair in `effort_variant` whose `// BORROW:` comment sits on the following line rather than the same one (the file's pre-existing `required_text` does the same); a few `.to_owned()`/`.as_str()` calls in the test files without `// BORROW:` annotations (the reviewer noted tests are not held to this strictly elsewhere in the repo); and two optional readability refactors (an `if`/`else if` chain over a fixed set of values that could be a `match`, and a `Settings::auto` field that is `true` on every path the driver currently reaches, which a future refused-then-mapped mode could make meaningful again).
4. **Nothing left incomplete.** All three steps landed, all gates named in the leaf pass, and the fixture/Docker/build-footprint cleanup described in the implementer brief is done (two leaf-built images removed, `docker images`/`ps`/`network ls` matched back to the pre-leaf baseline; the dedicated `CARGO_TARGET_DIR` deleted).


## Fix round 1

Base: `feat/m4-session-runner` at d30f830 (the leaf's own commits rebased onto it). Commits, after the rebase: `fd28003` (declare the settings table, with its conformance check and the broken-driver rows) and `2cd91f0` (the OpenCode mapping, its guards and the re-recorded fixtures), then this report. Citations are to OpenCode v2.2.7 and to Claude Code's docs (context7 `/websites/code_claude`: permission modes, permissions and read-only commands, CLI reference `--allowedTools`, Agent SDK permissions).

### What changed in the model

- **Baseline (M1).** Every OpenCode agent starts from `{*, *, allow}` plus an ask on `.env` reads (`packages/schema/src/agent.ts:39-50`). Config `permissions` are appended after it, last match wins (`permission.ts:98`), the `evaluate()` fallback is unreachable. An `allow` row restates the baseline; a `deny` row, written last, overrides it. `--auto` only answers the `.env` ask.
- **`tools.allowed` (M1).** Claude Code's `--allowedTools` only auto-approves; unlisted tools stay available (docs, CLI reference). Under `bypassPermissions` everything is approved already, so an allow row is a no-op in both harnesses and is mapped for that mode only. It is not mapped for any other mode, since no other mode is mapped.
- **One declared table (B1).** `HarnessDriver::settings_table() -> Vec<SettingRow>` (default: empty, so the Claude Code driver and the core's fakes compile unchanged). The OpenCode driver fills it from the same constants `settings()` reads. The command guard asserts the case set equals the row set and that each case's mapped/refused matches its row's disposition.

### Equivalence table, as it stands

| Cell setting | Result | Why / construct |
|---|---|---|
| `permission_mode: bypassPermissions` | Mapped | `--auto`; always on the command line, never `--agent` |
| `plan` (B3) | Refused | Claude Code plan = reads plus read-only shell (non-configurable set, its own parser). The rule list that is surely read-only (`explore` pattern, `plugin/agent.ts:80-93`) denies shell entirely and a wildcard on command text cannot express the set. The stock `plan` agent is not read-only once a global rule follows it, and its `subagent` child can edit |
| `default` (M2) | Refused | Claude Code allows reads plus a built-in read-only shell set; OpenCode's shell rows are wildcards on parsed command text, which approximate that set without being it. (No `--auto` does reject unmatched asks; that was never the obstacle.) |
| `acceptEdits` (M2) | Refused | Adds edits and mkdir, touch, rm, rmdir, mv, cp, sed inside the working directory; OpenCode's shell resource is the command text and knows no working directory |
| any other mode | Refused | no allowed set can be written as a ruleset |
| `budget.per_run.turns` (B4, B5) | Refused | `steps` changes the last request (tool-less, injected prompt) and ends `succeeded`, not `error_max_turns` (`Cap::Turns`); `oc_step_cap` is the evidence. No `steps` or `agents` block is written. The extra tool-less request in every recording is the hidden `title` agent's (`plugin/agent.ts:102-108`) |
| `budget.per_run.max_cost_usd` | Refused | no cost cap |
| `model.effort` low, medium, high, xhigh, max (M3) | Mapped | a `variants` entry on the model, `-m <model>#<level>`; the wire carries `thinking = adaptive/summarized` and `output_config.effort = <level>` on every request (`oc_effort_<level>`) |
| `model.effort`, model that cannot take it | Refused | Claude opus, sonnet, fable or mythos of 4.6 or later only (`variant.ts:68-100`, `325-339`: 3.7 and 4.0-4.5 are budget-based, haiku is outside the recorded wire, unrecognised ids get nothing); `xhigh` only from 4.7 (4.6 gets low, medium, high, max). A date-like minor (3+ digits) is read as no minor, which refuses |
| `model.effort`, a level outside the five | Invalid value | as before |
| tools, each of Bash, Edit, Write, Read, Glob, Grep, WebFetch, WebSearch, Task, Skill, TodoWrite | Mapped | action = the permission the tool registers: shell, edit, **edit** (B2: `tool/plugin/write.ts:49`), read, glob, grep, webfetch, websearch, subagent, skill, todowrite |
| `Edit` or `Write` alone in `tools.disallowed` | Refused | one `edit` action covers both (and patch); list both |
| MCP `mcp__<server>__<tool>` (L2) | Mapped | `<server>_<tool>`, each part with characters outside `[A-Za-z0-9_-]` turned to `_` (`runtime.ts:288-293`, `mcp.ts:21-25`) |
| `NotebookEdit`, `MultiEdit`, any other name | Refused | no OpenCode action |
| both lists (L1) | Mapped | allow rows first, deny rows last, so a name in both is denied |

`<home>/.opencode/plan/*` (`plan.ts:40`) is no longer reachable since plan is refused.

### Findings closed

| Finding | Closed | Bite (injected, guard turned red) |
|---|---|---|
| B1 table | yes | row dropped from the table (turns row): case-set vs row-set assertion red; case dropped (Glob): red at the set assertion; Glob to grep, Task to task, WebFetch to fetch: red at the case |
| B2 Write | yes | `Write` back to `write`: red |
| B3 plan | yes, by refusal | plan accepted again: red (cases: plan alone, with `tools.allowed` Edit, with Task, with an MCP name) |
| B4 | yes | no `agents` block asserted for every mapped case |
| B5 turns | yes | refusal removed: red; the step-cap wire guard is deleted |
| M1 | yes | baseline stated in the table row and above; allow-row no-op documented |
| M2 | yes | reasons asserted by needle per mode |
| M3 effort | yes | `xhigh` removed from the levels: red; variant display changed: red (wire guard compares the driver's entry with each fixture's `config.json`); `xhigh` on 4.6 accepted: red |
| L1 | yes | deny rows placed first: red |
| L2 | yes | MCP not normalised: red; a name with `_` in the server now maps |
| L3 | see B3 | |
| L4 | yes | conformance bites, each red: shared state variable, state variable equal to a gateway name, invalid state variable name, address and token one variable, reserved name twice, settings row twice, refusal with no reason, mapping with no construct. `Settings::auto` and `agent` removed |

Every fix is a row in an existing guard (`opencode_command_line_maps_or_refuses_every_setting`, `opencode_wire_effort_levels_reach_the_request_body_as_output_config`, `conformance_refuses_a_broken_driver`); no new test function was added, and the two old wire guards (`..effort_variant..`, `..step_cap..`) were replaced or removed. Process note: the guard rows were written after the implementation of this round, not before; every row was then shown red by an injected fault (list above), none by running against the old code.

### Newly refused or changed relative to 03900ae and the first leaf commit

03900ae refused every one of these settings, so nothing is refused that it accepted. Relative to e711fb5 (the first mapping): `plan` (was mapped), `budget.per_run.turns` (was mapped), `model.effort` with a model that is not a Claude 4.6+ opus, sonnet, fable or mythos (was accepted for any model, e.g. `claude-x`), a lone `Edit` or `Write` in `tools.disallowed` (was accepted), `TodoWrite` is now mapped (was refused), an MCP server name containing `_` is now mapped (was refused), and `Write` now means `edit` (was `write`, which matched nothing). A name that accepted before and is refused now: none among tool names or MCP names.

### Recordings

`oc_effort_low`, `_medium`, `_high`, `_xhigh`, `_max` replace `oc_effort_variant` (fake model, production gateway sidecar, no tokens; hashes in `PROVENANCE.toml`); `oc_plan_mode_blocks_shell` is deleted (plan is refused). `oc_step_cap` and `oc_permission_ask` stay as refusal evidence. Docker: the two images built for the re-record were removed by exact id and `docker images -q` / `docker ps -aq` match the baseline taken before.

### Counts and gates

Workspace nextest: 643 passed, 22 skipped. K1 (`conformance_`): 13 passed. O1 (`opencode_`): 13 passed. `opencode_command_`: 1. `opencode_wire_`: 1. Clippy `-D warnings` (workspace, all targets) and the LINUX target, rustdoc `-D warnings` and `cargo fmt --check` clean. Core untouched against 9366622. The Claude Code crate's `src` is unchanged against 9366622; its `tests/conformance.rs` differs by the leaf's earlier commit 7d4de2b (owned path). `crates/candler-harness/tests/public_surface.rs` is not an owned path and does not name `SettingRow`/`Disposition`; the coordinator may want it to, since it is the contract the API review diffs.
