# opencode_driver: what the OpenCode V2 driver needed that the traits could not say

Leaf of `m4-session-runner`, branch `m4/opencode_driver`, rebased on the milestone tip 233df68. `candler-harness-opencode` implements `HarnessDriver`, a live reader, an export reader and a skill tree for OpenCode 2.2.7, passes the shared fixture suite on the nine recorded runs, and ships `assets/docker/base-opencode/Dockerfile`. No file of `candler-harness`, `candler-harness-claude-code`, `candler-core` or `candler-schemas` is touched; `candler` has the one array the coordinator asked for. Census is off. No real model, no RiVault.

## 1. Commits

| sha | subject |
|:--|:--|
| cb1cd21 | `test(adapter-opencode): state the driver from the recorded runs` (the leaf's subject is 75 characters: F15) |
| 8d077a5 | `feat(adapter-opencode): drive opencode v2 through the shared traits` |
| 8188354 | `build(adapter-opencode): add the opencode base image recipe` |
| bfbc0ef | `build(cli): embed the opencode base image context` (the coordinator extended OWNED by `ASSETS` in `crates/candler/src/commands/assets.rs` and one `[internal]` row of `docs/contract/codes.toml`) |

The report commit, `reports(adapter-opencode): record the opencode_driver leaf`, follows. Step 1 carries a stand-in driver and skill tree so each commit builds; Step 2 replaces them.

## 2. Gates

| Gate | Result |
|:--|:--|
| K1 `-E 'test(/^conformance_/)'` | PASS: 1 test run, 1 passed |
| O1 `-E 'test(/^opencode_/)'` | PASS: 18 passed, 2 skipped (the Docker test and the `conformance_` test) |
| `opencode_capabilities_` | PASS: 1 passed |
| Dockerfile pins version and checksum | PASS (`2.2.7` from PROVENANCE, `sha256` in the file) |
| `-p` STD: fmt, clippy `-D warnings` all targets, `cargo doc -D warnings`, nextest | PASS: 19 run, 19 passed, 1 skipped (the `#[ignore]` Docker test) |
| Docker test, once, offline, hardened, base image | PASS: `opencode_real_binary_is_interrupted_by_the_stop_script` (10.4 s) |
| LINUX `cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings` | PASS (Finished) |
| DIGEST | PASS: numstat 0, `guard_cell_digest` 9, core digest binaries 45 |
| SPAWN, before the runs | PASS: `guard_signal` 9 of 9 |
| SUBJ | PASS: `cog check 233df68..HEAD` no errored commits; longest subject 67 |
| OWNED | PASS: `git diff --name-only 233df68..HEAD` lists the crate, `assets/docker/base-opencode/`, `Cargo.lock`, and the two paths the coordinator added to OWNED (`crates/candler/src/commands/assets.rs`, `docs/contract/codes.toml`); `candler-core`, `candler-schemas`, `candler-harness`, `candler-harness-claude-code` untouched |
| No `confirm_*` | PASS |
| Candler guards after the two coordinator edits: `guard_cli_surface` (incl. `u3_the_embedded_assets_equal_the_tree`), `guard_refusal_codes`, `contract_pages` | PASS: 18 passed (`OpenCodeError` is one whole-enum `[internal]` row; the generated pages did not change) |
| `rust-conventions-reviewer` | no rule violation; seven comment and path edits applied (EXPLICIT annotations, one self-path) |

## 3. Guards

Oracles never come from the driver's code: the hand-written `expected.toml` and `exit.toml` of each fixture, R01 §Ends by hand, the fixtures read with `jq`, or what the real binary printed. Red-first, honestly: at the stand-in the suite failed at `gateway: the driver names no gateway variables` and 17 of 19 tests were red; two passed (the exit classification, which the stand-in already answered `unknown`, and an MCP refusal that passed because no file was written; the latter got a positive control). Both were bitten afterwards.

| Id | Guards | Oracle | Bite and the failing message seen |
|:--|:--|:--|:--|
| `conformance_opencode_recorded_runs` (27 cases: each of nine runs with its export, live only, and export alone, plus export-wins) | R01 §Harness drivers, §OpenCode v2: sessions from the export, end from the export's outcome over the stream's error event, collect inside the isolation | `expected.toml`, `exit.toml`, R01 §Ends | interrupted read as completed: `interrupt_api_with_export sessions: expected [... Unknown], the reader finished with [... Completed]`; provider refusal read as agent error: `refused_key_with_export ... expected GatewayError ... AgentError { reason: None }`; live end beating the export: `export_wins_over_the_live_end classify_end: expected Unknown, the driver classifies Completed`; live reader ignoring `error`: `interrupt_api_with_export observed: expected end Some(Aborted), the reader observed None` |
| `opencode_command_forces_standalone_json` | `run --standalone --format json --auto -m candler/<model> -- <prompt>` | `opencode_facts` row 2; the real binary rejected `run ... "--hello x"` and took it after `--` | `--standalone` dropped and `--` dropped: `assertion left == right failed` with the argv shown |
| `opencode_command_refuses_values_it_cannot_honour` | effort, turn and cost caps, tool lists are refused, never dropped | `run --help`; F2 | effort entry removed: `model.effort: expected a refusal, got Ok([...])` |
| `opencode_prepare_is_the_config_the_binary_accepted` | the home-rooted `opencode.json`: provider by `{env:}` names, MCP in the V2 shape | `tests/fixtures/config/opencode.json`, accepted by `opencode debug config` and `opencode run` in the image (`debug_config.out`) | `/v1` dropped: object mismatch; written to the workspace: `left: Workspace right: Home` |
| `opencode_codemode_is_read_from_the_cell_value` | `codemode` per server, absent written as on | `opencode_facts` row 17; V2 shape from the binary | hardcoded on: `left: [true,true,true] right: [true,true,false]`; absent written off: `left: [false,true,false]` |
| `opencode_mcp_refuses_a_shape_it_cannot_translate` | unknown server shapes refused, with a control | shapes the binary printed | unknown type guessed as local: `assertion failed: prepared_config(...).is_err()` |
| `opencode_state_paths_are_the_recorded_layout` | four XDG dirs and `OPENCODE_DB` | `opencode debug paths` in the image | database moved: tuple lists differ |
| `opencode_stop_interrupts_inside_the_container_then_terms` | pre-stop is an `Exec` with `{session_id}` as its own argument, refused for an unsafe id, then TERM | facts rows 9, 10 | `Command` instead of `Exec`: `assertion failed: matches!(action, Action::Exec(_))` |
| `opencode_real_binary_is_interrupted_by_the_stop_script` (ignored, Docker) | the script finds the `serve` child by its whole argv (a decoy with the same words is not it), interrupts, run exits 1 with `aborted`, export says `interrupted`, readers agree | the real v2.2.7 binary | substring match: `panicked ... stop_rc` assertion (36 s, stop exit 4) |
| `opencode_collect_exports_the_root_session_inside` | one `Exec` export for the first plain root id, after a dash id and a child id | facts rows 4, 6, F6 | dash ids admitted: `left: [... "-sanitize"] right: [... "ses_e32c17b4fffe2bjBVM2E3xAaLZ"]` |
| `opencode_live_reader_reports_id_and_last_word` | no events, the id from the first line (or the error event), minimal sessions, none without an id | recorded stdout | via the conformance bite above |
| `opencode_export_is_the_trace` | user and assistant messages, call and result paired by id, per-message usage, session total from `info` | `completed_o1/export.json` by hand | total dropped: event list differs |
| `opencode_export_keeps_errors` | an interrupted tool is an error result; a failure before any step keeps its error as a notice | `interrupt_api`, `unknown_model_local` exports | tool error as ok: `left [is_error false] right [is_error true]` |
| `opencode_export_counts_cache_cost_and_reasoning` | cache, cost, reasoning kept; unlisted tool is origin `other` | a document in the recorded shape with the real-run numbers of `opencode_facts` | cache read dropped: `left Number(0) right 4704` |
| `opencode_export_refuses_what_is_not_an_export` | refuses non-exports, unknown outcome is `unknown` | hand-written documents | not bitten |
| `opencode_classifies_an_exit_with_no_trace_as_unknown` | an exit with no trace is never completed | facts row 15 | exit 0 as completed: `exit 0 left: Completed right: Unknown` |
| `opencode_version_is_parsed_from_the_probe` | `opencode v2.2.7` | the binary's output | fixed version: `left 0.0.0 right 2.2.7` |
| `opencode_capabilities_declare_no_session_and_outside_execution` | no session, census, budget; execution outside the child | the leaf, F1, F2 | session on: `assertion failed: !caps.session && ...` |
| `opencode_gateway_names_are_the_ones_the_config_refers_to` | the two names are those the config refers to | the config fixture | token renamed: `assertion failed: config.contains(...)` |
| `opencode_skills_land_in_the_project_skill_directory` | `.opencode/skills/<name>/`, exec bit kept, no sibling skill | `git` (`cat-file`) on a fixture repo; facts row 16 | written to `.claude/skills`: path lists differ |

## 4. Probe answers (Docker, offline, base image)

Image built from the new recipe with `docker build -q`; every container `--rm`, `--network none`, `--read-only`, `--cap-drop ALL`, no-new-privileges, pids 512, 4 g, uid 10001, tmpfs `/tmp` noexec and `/home/agent`.

| Question | Evidence | Decision |
|:--|:--|:--|
| Does inline `{env:X}/v1` concatenate in V2? | `opencode debug config` printed `baseURL: http://gateway:8080/r/tag/run/key/v1` for `{env:CANDLER_GATEWAY_URL}/v1`; secrets `***` | closes driver_traits F2: no path suffix needed in `GatewayEnv` |
| Which config shape? | V1 `provider`/`mcp.<name>` are normalised to V2 `providers.<id>{package:"aisdk:<npm>", settings, models}` and `mcp.servers.<name>`; the V2 form loads as written | the driver writes the V2 form |
| Can a prompt start with `-`? | `run ... "--hello x"` printed usage and `Unrecognized flag`; after `--` it ran | `--` before the prompt |
| Server child argv, port, password | `/usr/local/bin/opencode serve --stdio --port 0`; `OPENCODE_PASSWORD` in its environ; one listening loopback socket | the stop script matches the whole argv by basename, reads both from `/proc`, then `opencode api ... session.interrupt` answered `{"interrupted":true}`; the run ended exit 1, `aborted`; export `interrupted` |
| Export under isolation | `opencode session export --standalone <id>` printed pretty JSON | the reader takes any JSON whitespace |

## 5. Every `classify_exit` class

| Class | Driver says | Backed by |
|:--|:--|:--|
| exit 0, `succeeded` | completed | fixture `completed_o1` |
| 143 or 130, `interrupted` | unknown (the core's stop, wall cap or kill explains it) | fixtures `sigterm_mid_tool`, `sigint_mid_tool`, `unreachable_dns` |
| exit 1, `aborted`, `interrupted` | unknown | fixture `interrupt_api`, and the real binary in the Docker test |
| `provider.auth`, `provider.invalid-request`, `provider.transport` | gateway error (R01 §Ends: a provider status, or connection refused) | fixtures `refused_key`, `unknown_model_remote`, `unreachable_refused` |
| `provider.no-route` | agent error, reason `provider_no_route` | fixture `unknown_model_local` |
| Any other `provider.*` type (429, 5xx) | gateway error, by the prefix | documentation or guess only |
| Any other error type, step cap, out of memory, permission denial, sub-agent | agent error with a sanitised reason; the cap and the rest have no class | none |

## 6. Where the driver worked around the traits

| # | Finding | Flag |
|:--|:--|:--|
| F1 | **`permission_mode` and `model.endpoint` are required of every cell** by the core's surface (`experiment/surface.rs`), and both are Claude-shaped. Fix round 1: the driver accepts only `permission_mode = bypassPermissions`, the mode `--auto` implements, and refuses the rest. `model.endpoint` is still unread. | **Request**: the core's surface |
| F2 | **Refused cell values** (behaviour of this new driver, not a core change): `model.effort`, `budget.per_run.turns`, `budget.per_run.max_cost_usd`, `tools.allowed`, `tools.disallowed` return `DriverError::InvalidValue`. `capabilities.budgets` is false: a turn cap is `agents.<id>.steps`, but the exit and end of a capped run were never recorded, so the driver cannot name the cap's end. A recorded capped run would let it declare `turns`. | **Behaviour change, report**; coordinator ruling |
| F3 | **The harness version reaches no reader.** Neither the NDJSON nor the export names the binary's version, and `offline_reader(stream)` takes none, so every session's `harness_version` is the constant `VALIDATED` (2.2.7). Fix round 1 makes the constant true: the driver accepts exactly 2.2.7 (`SUPPORTED` is `=2.2.7`, `parse_version` refuses any other). `Confidence` has no level below `Likely`, so `detect` can only be made stricter, not less sure. | **Request**: a trait item to hand the probed version to a reader, and an undated confidence level |
| F4 | `EndedBy::Interrupted` means "the supervisor was lost", so a stop on request has no honest value: an interrupt and a signal both end the session as `unknown`, with the cause only in the raw export. The core maps `interrupted` and `killed` to `unknown` anyway. | **Note**; schema wording |
| F5 | `provider.no-route` (no request left) is an agent error here, as R01 §Ends says of an error with no provider status; the core's zero-token rule will still make the run `invalid(gateway)` and retry it once. | **Note** |
| F6 | The provider package is fixed to `@ai-sdk/anthropic`, the one the fixtures cover. The real-model runs of `opencode_facts` needed `@ai-sdk/openai-compatible`; the cell has no place for the protocol (`model.endpoint` is required but unread). | **Request**: where a cell says the provider protocol |
| F7 | Gateway names are `CANDLER_GATEWAY_URL` and `CANDLER_GATEWAY_TOKEN`, chosen by the driver and reserved. The core still sets `ANTHROPIC_*` until `harness_out_of_core` reads `gateway()`. The label key the image carries is `candler.cc_version`, the one the core reads (the name is Claude Code's). | **Request** to L2/L3 |
| F8 | The leaf did not say `--`: a prompt that starts with `-` is an unknown flag without it. | **Spec defect** (leaf text) |
| F9 | `render_prompt`, the cell helpers and the tar reader of the skill archive are copies of `candler-harness-claude-code`'s (about 250 lines); a driver crate cannot use a sibling's, and `candler-harness` is not mine. | **Request**: move them to `candler-harness` |
| F10 | The skill source is assumed to be `.claude/skills/<name>` in the treatment's repository (the layout the archive command already uses); the destination `.opencode/skills/<name>` is taken from `opencode_facts` row 16. The binary has no offline command that lists skills, so the destination is not re-verified here. | **Note** |
| F11 | A code-mode `execute` call was never recorded: its `wrapped_calls` are empty, and a direct MCP tool (`<server>_<tool>`) is origin `other` because the server name cannot be split from the tool name. Code mode, a treatment factor, is invisible in the trace until a run with an MCP server is recorded. | **Request**: one fixture with a code-mode call |
| F12 | The stop needs a `/proc` script and `python3` (both in the base image). A stop before the first stdout line has no session id, so `Action::resolve` gives `None` and the core falls back to the signal. The script exits 3, 4 or 5 when it finds no server, no password or no port, which the core must treat as "interrupt did not happen". | **Note**, matches driver_traits F6 |
| F13 | Collect exports one root session (the first plain id) under the stream `export`; a second root id would collide on the stream name. | **Note** |
| F14 | The live reader's `finish` returns minimal sessions (the end from the stream's last word) rather than an error, because the core today turns a live-reader error into `invalid(reader_error)`. | **Note** for `opencode_wiring` |
| F15 | The leaf's Step 1 subject is 75 characters, over SUBJ's 72; shortened. | **Spec defect** |
| F16 | Cargo: no new external crate. `jiff` is already a workspace dependency; `Cargo.lock` gained only this crate's dependency list. | **Note** |

## 7. Docker bookkeeping

Before: 12 images, 3 containers, 5 networks, recorded sorted. Built `3b573420f739` (Step 3, used for the probes and the Docker test) and `be10a014bca2` (the Step 3 consistency build re-run at the end); both removed by exact id, and `docker images -a -q` equals the record. Containers: every one was `--rm`; `docker ps -aq` equals the record. Networks: none created (`--network none`); the built-in `bridge` has another id than at the start, as `opencode_facts` F9 found. No prune, no kill, no stop, Docker left up. The first build finished in seconds, consistent with the BuildKit cache holding the approved arm64 archive; I did not time it, so a refetch of that one approved file on a cache miss cannot be ruled out. No x64 file was fetched or built: its checksum is the digest `gh api repos/anomalyco/opencode/releases/tags/v2.2.7` publishes for `opencode-linux-x64.tar.gz` (`89a6cee2...2ca7`).

The target directory `/private/tmp/claude-501/m4-target-opencode_driver` and the scratch directory `/private/tmp/claude-501/m4-scratch-opencode_driver` are removed before this report is returned.

## 8. Fix round 1

The branch was rebuilt with fixups autosquashed on 233df68; the five commits are the ones of section 1, and the report is last. Every new guard was run red against the committed code first (8 of the 23 tests failed before the fixes), then green, then bitten with the file restored by `git checkout` (messages below were seen).

| # | Fix | Guard (id) | Bite and the failing message |
|:--|:--|:--|:--|
| B1 | `root_end` prefers the root session whose reader is the export reader, in whatever order the sessions come, and falls back to the live reader's only when the export gave none. The suite swaps the exported sessions in itself, so its cases could not hold both orders; the guard hands the driver `[live, export]` and `[export, live]` | `opencode_export_wins_whatever_the_order`: live `completed_o1` with the `interrupt_api` export (`unknown`), live `interrupt_api` with the `completed_o1` export (`completed`), live `completed_o1` with the `refused_key` export (`gateway_error`), both orders; and the live-only fallback | first root wins: `live completed_o1, export interrupt_api  left: Completed  right: Unknown` |
| B2 | Only `permission_mode = bypassPermissions` builds a command; every other value is `InvalidValue` naming the mode. The conformance and guard cells use it | `opencode_command_refuses_values_it_cannot_honour` (`acceptEdits`, and the detail names `bypassPermissions`) | check off: `permission_mode: expected a refusal, got Ok([... "--auto" ...])` |
| M1 | A key outside the shape's set is refused, naming `mcp.<server>.<key>`: stdio takes `type command args env codemode`, remote takes `type url headers codemode`. Refused: `timeout`, `enabled`, `cwd`, `tools`, `headers` on stdio, `args` and `env` on remote. The control accepts every allowed key | `opencode_mcp_refuses_a_shape_it_cannot_translate` | each check off: `assertion failed: prepared_config(&[("mcp", servers)]).is_err()` |
| M3 | `info.parentID` sets `parent_session`, and the id becomes `<parent>/<id>`, as the schema requires of a child. A lone child cannot be the root of a trace | `opencode_export_of_a_child_session_is_not_a_root` | parent ignored: `left: None` (the `parent_session`) |
| F3/L4 | `parse_version` takes exactly `opencode vX.Y.Z` and refuses any version but the validated 2.2.7 with `DriverError::VersionOutput` (no variant fits better: `InvalidValue` names a cell path). `SUPPORTED` is `=2.2.7`. `detect` for the live stream needs a known event kind with a session id, and for the export an `info` with an id and a known outcome (a cut sample is read as text for the pair). `Confidence` has no level below `Likely`, and no stream carries a version, so `Likely` is the floor | `opencode_version_is_parsed_from_the_probe`, `opencode_detect_recognises_only_what_it_reads` | token search: version test fails on `see opencode v2.2.7`; other versions accepted: same test on `v2.2.8`; any event type: `live.detect(... "mystery" ...).is_none()` fails |
| F3 | The Dockerfile says the key `candler.cc_version` is historical and holds the harness version | none (a comment) | none |
| L1 | The stop script requires the matched `serve` process's parent to be `opencode run`, reads `/proc/net/tcp6` as well as `tcp` (`[::1]` for the IPv6 loopback), and keeps `{session_id}` as its own argument. Its predicate `is_server(argv, parent_argv)` is a function: the offline guard runs it under `python3 -I` on listed command lines (real child, relative path, decoy, `sh -c`, `timeout`, wrong parent, no parent); it skips when `python3` is absent. The real-binary Docker test passes with the new script | `opencode_stop_script_matches_only_the_server_child`, and the ignored Docker test | parent not required: `left: [true, true, false, false, false, true, true, true]`; substring match: `left: [true, true, true, false, true, false, false, false]` |
| L3 | `capabilities.live_events` is false: the live reader emits no event | `opencode_capabilities_declare_no_session_and_outside_execution` | on: `the live reader emits no event` |

### Gates after the round
| Gate | Result |
|:--|:--|
| `-p` STD: fmt, clippy `-D warnings` (`candler-harness-opencode`, `candler`, all targets), `cargo doc -D warnings`, nextest | PASS: 23 passed, 1 skipped (the Docker test) |
| Docker test against the rebuilt image | PASS (7.0 s) |
| LINUX | PASS |
| SUBJ, OWNED, DIGEST numstat | PASS |
| SPAWN `guard_signal`, `guard_cell_digest`, core digest binaries, `guard_cli_surface`, `guard_refusal_codes`, `contract_pages` | PASS: 9, 9, 45 and 18 passed |

The Docker rebuild for this round took 3.7 s (a pure BuildKit cache hit), which settles the open point of section 7: no file was downloaded. The image `6a4e980d5c64` was removed by exact id; images and containers equal the record taken before. The target and scratch directories are removed before this report is returned.
