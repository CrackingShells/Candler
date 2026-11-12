# harness_out_of_core: the core asks the driver, the binary picks it by name

Leaf of `m4-session-runner`, branch `m4/harness_out_of_core`, rebased on the milestone tip 9bdfc1c (after `opencode_driver`). `candler-core` no longer names a harness: the image version, the gateway and state variables, the census, the end classification and the skill layout come from the driver. The binary selects the driver by `request.harness` through `crates/candler/src/drivers.rs`.

## 1. Commits

| sha | subject |
|:--|:--|
| cc48980 | `test(isolation): pin what a claude code run looks like before the move` |
| 632d0bb | `refactor(isolation): read the harness version through its driver` |
| d052d96 | `refactor(isolation): take the gateway, state and census from the driver` |
| fc5fad8 | `feat(cli)!: select the harness driver by the experiment's name` (BREAKING CHANGE footer: name a registered harness) |
| 4f0c50c | `test(isolation): give the fake harness an opencode-shaped gateway` |

The report commit follows. Baselines (argv, census, comparables) were recorded by running the producers unchanged on a scratch checkout at a26fe53 and again at cf3de30; the two sets are identical.

## 2. Gates (run on the rebased tip)

| Gate | Result |
|:--|:--|
| H1 `harness_version_` + label key `candler.cc_version` | PASS (3 tests) |
| H2 `harness_env_` | PASS |
| H3 `harness_census_` | PASS |
| H4 `harness_resolution_` | PASS |
| R1 `registry_` | PASS (3 tests) |
| Core names no harness | one hit, see 4.1; `.version_probe(` present; no `"subtype"`/`"init"` under `runner/evidence` |
| Only `drivers.rs` names a driver in the binary | PASS |
| `.classify_end(` in the runner, no `.classify_exit(` in the core | PASS |
| STD workspace (`fmt --check`, clippy `-D warnings` all targets, `nextest --workspace`, `cargo doc -D warnings`) | PASS: 668 run, 668 passed, 22 skipped |
| `guard_refusal_codes::r1_*`, `guard_cli_surface::u3_*`, `binary(contract_pages)` | PASS (contract pages regenerated with `CANDLER_BLESS_CONTRACT=1`, then re-run) |
| LINUX | PASS |
| SPAWN | PASS: `guard_signal` 9 of 9 |
| DIGEST | PASS: numstat 0, `guard_cell_digest` 9, core digest binaries 45; no digest moved |
| GOLDEN | PASS: `guard_golden` 2, `guard_oracle_ac` 1 |
| SUBJ | PASS: `cog check 6564467..HEAD` and `9bdfc1c..HEAD`; longest subject at most 72 |
| Reviewer (`rust-conventions-reviewer`) | three doc and line-width findings and one nit, all applied |

## 3. Bites

| Guard | Bite | Seen |
|:--|:--|:--|
| H1 | the core's own `claude --version` argv in `ImageService::probe` | `harness_version_comes_from_the_drivers_probe_and_parse` red |
| H2a | `ANTHROPIC_*` names hardcoded | `harness_env_reserved_names_are_the_drivers` and `harness_env_second_driver_names_reach_the_argv` red |
| H2b | a `CLAUDE_CONFIG_DIR` set by the core | `harness_env_claude_code_argv_equals_cf3de30` red |
| H3a | census tools not sorted | `harness_census_equals_cf3de30_on_every_fixture` and `harness_census_without_servers_records_tools_only` red |
| H3b | `LiveHandle::observed` empty | `harness_census_live_handle_reports_the_inventory_before_finish` red |
| H4 | `harness.driver` recorded by the identity step | `harness_resolution_comparable_equals_cf3de30` red |
| R1 | unknown name falls back to Claude Code | `registry_unknown_harness_is_refused_naming_it` and `registry_ingest_selects_the_reader_by_name` red |

The leaf text lists "the core matching `system`/`init` with a reordered key" for H3; no such matching remains in the core (the grep gate on `"subtype"`/`"init"` holds), so H3a and H3b stand for it.

## 4. For the coordinator

1. **The "core names no harness" grep gate does not pass as written.** `crates/candler-core/src/isolation/refusal.rs:211` carries the code string `isolation.claude_version_unreadable`. I kept the code (it is a contract identifier with a page; renaming it is a compatibility decision for you) and the variant `ImageError::VersionUnreadable`; only the description strings changed. Renaming the code to `isolation.harness_version_unreadable` is a one-line change plus `codes.toml` and the regenerated pages; say if you want it.
2. D10: `experiment.unknown_harness` (`CliRefusal::UnknownHarness`), refused before any docker call. A `--set request.harness={"name":"x"}` is checked the same way; `--set request.harness.name=x` is refused earlier as `experiment.unknown_path`, as before. One `[internal]` row added for `candler::drivers::BuildError`.
3. `JobServices`: no field added. `jobs/runtime/session.rs`: no hunk. `jobs/runtime/mod.rs` imports the harness crate's `SkillTree`.
4. OpenCode verifier points: the image build probes with the driver's `version_probe()` and `parse_version()` (H1; the fake driver's program is not `claude`). The gateway variables are the driver's `gateway()` names; the second driver of H2 declares `CANDLER_GATEWAY_URL` and `CANDLER_GATEWAY_TOKEN`, OpenCode's pair, and no `ANTHROPIC_*` reaches its argv. The OpenCode driver is not registered here (`opencode_wiring`). The leaf does not depend on the OpenCode crate; the second driver is a fake in the test.
5. Edits outside OWNED: `isolation/container/mod.rs` (one `ContainerPlan` literal), `isolation/refusal.rs` and `runner/refusal.rs` (description strings), container tests (`guard_container_{guard,record,run}.rs`, `container_run_e2e.rs`, `env_image_e2e.rs`), `guard_run_evidence.rs`, `guard_cli_kill_live.rs`, `docs/contract/{codes.toml,experiment-file.md,index.md}`, the new `guard_harness_facts.rs`, `support/harness_facts.rs` and `fixtures/harness_baseline/**`.
6. Behaviour changes beyond D10, none refused for Claude Code runs: `ContainerPlan` now carries `gateway` and `state`; a driver with no gateway is refused at run time; a gateway name that repeats a name of `plan.env` is a new `DuplicateName` refusal; a version probe that sets an environment or a directory is refused (a control container cannot honour it); `control_argv` reserves no names; the census records `census.tools` and `census.init_tools` when `mcp_servers` is `None` (it recorded nothing), and its source is the live reader's first init report that has tools; `parse_version` of Claude Code accepts a little more than before.
7. `ingest` raw-format recognition still calls the Claude Code crate's `detect` (`commands/ingest.rs`, inside the binary, not through the registry); the reader itself comes from the registry.
8. `runner/evidence/mod.rs` has a stale module doc line about event keys; no end-to-end test covers the `classify_end` wiring beyond the grep gate.

## 5. Cleanup

The leaf's target directory, the scratch worktree and its target directory are removed after this report.

## 6. Fix round 1

The verifier found nothing moved: the cf3de30 baselines, DIGEST and GOLDEN stood. This round closes what survived mutation and the one remaining Claude Code shape. Every fix is a row of a table guard or a strengthening of an existing guard, and each guard was seen red by the named mutation before it was relied on.

### 6.1 Commits

| sha | subject |
|:--|:--|
| 7a7de7e | `fix(isolation)!: retire the claude-named version refusal code` (`BREAKING CHANGE:` footer) |
| 319a968 | `refactor(runner): take the run totals from the driver's live reader` |
| 782e0ef | `test(isolation): guard the refusals the driver wiring adds` |
| 9613160 | `refactor(cli): recognise raw trace streams through the registry` |
| a662e6f | `refactor(isolation): harness-neutral version field, one agent home` |
| eb29f71 | `rdm(runner): grep the evidence for the harness's result frame` |
| 3ba5c17 | `test(isolation): keep the new guards clippy-clean` |

The report commit follows.

### 6.2 What changed, and the mutation that bites

| Finding | Change | Guard | Mutation | Red before the change / with the mutation | Green after |
|:--|:--|:--|:--|:--|:--|
| F1 (ruled by the coordination session) | `ImageError::VersionUnreadable` carries `isolation.harness_version_unreadable` for every harness; the message names the harness and the probe argv. `codes.toml` keeps the old row with `retired = "<new code>"`, and its header defines retirement. The contract pages list the old code as retired. | `guard_refusal_codes::r1_every_variant_code_equals_its_row`, new rows: a retired row names a live replacement for the same variant and side, only live rows must be raised, and no table emits a retired code | the variant pointed back at `isolation.claude_version_unreadable` | `VersionUnreadable: code isolation.claude_version_unreadable is retired and a refusal still emits it` | 4 of 4 |
| F5 and F4 | `drive` is held to `classify_end` and to the observed census by one guard that runs a job over a driver whose live reader reports an inventory and totals and whose observed end (`agent_error`) differs from its exit classification (`completed`) | `guard_container_record::harness_census_and_end_come_from_the_drivers_live_reader` | `classify_exit` in place of `classify_end` at the call in `runner/exec/mod.rs` | `the end is the driver's observed end`: `completed` against `agent_error` | pass |
| | | | `census(None)` | `census.tools`: `None` against `["Bash","Agent"]` | pass |
| F2 | `Observed.totals` (`RunTotals`: usage, cost, turns; none by default) is filled by the Claude Code reader from its last `result` line, and the core reads only `Observed`. `run_totals.json` pins the three `run.*` values on every Claude Code fixture. | `harness_census_equals_cf3de30_on_every_fixture` (extended by `run_totals.json`) and the guard above | the reader's `observed()` leaves the totals empty | `run totals of sessions/c1`: `{}` against the three recorded values | pass |
| | | | the core takes the totals from an empty view | `run.num_turns`: `None` against `2` | pass |
| F7 | one table, `harness_env_refusals_name_their_variant`, one row per refusal (reserved name, repeated gateway URL and token variable, state variable already set, no gateway in the driver, no gateway in the plan); one table, `harness_env_state_paths_are_in_writable_mounts`, one row per mount case (the read-only home folded in); a row each for the probe's environment and directory in `harness_version_comes_from_the_drivers_probe_and_parse` | those three | gateway-name collision check removed (`container/run.rs`) | `harness_env_refusals_name_their_variant` red | pass |
| | | | probe environment and directory check removed (`isolation/image.rs`) | `harness_version_comes_from_the_drivers_probe_and_parse` red | pass |
| | | | `already set` check removed (`apply_harness_env`) | `harness_env_refusals_name_their_variant` red | pass |
| | | | no-gateway refusal of `apply_harness_env` removed | `harness_env_refusals_name_their_variant` red | pass |
| | | | no-gateway refusal of `agent_argv` removed | `harness_env_refusals_name_their_variant` red | pass |
| | | | state check dropped from `mounts` | `harness_env_state_paths_are_in_writable_mounts` red | pass |
| | | | home check dropped from `mounts` | `harness_env_state_paths_are_in_writable_mounts` red | pass |
| | | | reserved list fixed to `ANTHROPIC*` | `harness_env_refusals_name_their_variant` red | pass |
| F6 | `DriverEntry.detect` and `Registry::recognise`; ingest asks the registry for the recogniser and the reader, and names no harness. | `registry_unknown_harness_is_refused_naming_it` (now a table, six rows: the file, an object and a bare name given to `--set`, for `run` and for `resolve`); `guard_ingest::i1_the_verb_ingests_a_directory` and `i4_*` for the recogniser | the bare-string arm of `requested_harness` returns none; `resolve` falls back to `claude-code`; `Registry::recognise` finds nothing | the table red for the first two; `i1` and `i4` red for the third | pass |
| F3 | `ImageRef.cc_version` is `harness_version`; the label key `candler.cc_version` is unchanged | the leaf's H1 grep and the e2e label reads | none (a rename) | | pass |
| LOW | the three `harness_version_*` tests are one table, the three `harness_census_*` inventory tests are one table; the mounted home and the state directories come from `agent_home`; the baseline README says the producers were adapted to the old API | | | | pass |

### 6.3 Test counts

| Test file | Before | After |
|:--|--:|--:|
| `candler-core/tests/guard_harness_facts.rs` | 14 | 9 (version 3 to 1, env 5 to 4, census 5 to 3; the rows hold what the removed tests held, plus the new refusal rows) |
| `candler-core/tests/guard_container_record.rs` | 5 | 6 |
| `candler/tests/guard_registry.rs` | 3 | 3 (the first is a six-row table) |
| `candler-core/tests/guard_refusal_codes.rs` | 4 | 4 |
| workspace | 668 | 664 |

### 6.4 Gates

| Gate | Result |
|:--|:--|
| H1, H2, H3, H4 (`harness_version_`, `harness_env_`, `harness_census_`, `harness_resolution_`) | PASS: 1, 4, 4 and 1 tests; the label key is still in `image.rs` |
| R1 (`registry_`) | PASS: 3 tests |
| The core names no harness (`claude|anthropic|opencode` over `candler-core/src` and `candler-schemas/src`) | PASS with no waiver: no line |
| `.version_probe(` in the core; no `"subtype"` or `"init"` under `runner/evidence` | PASS |
| New grep: no `"result"`, `total_cost_usd` or `"num_turns"` under `runner/evidence` | PASS (see 6.5 for the form of the last key) |
| Only `drivers.rs` names a driver in the binary | PASS |
| `.classify_end(` in the runner, no `.classify_exit(` in the core | PASS |
| STD: `fmt --check`, clippy `--workspace --all-targets -D warnings`, `nextest --workspace`, `doc -D warnings` | PASS: 664 run, 664 passed, 22 skipped |
| LINUX | PASS |
| SPAWN | PASS: `guard_signal` 9 of 9 |
| DIGEST | PASS: numstat 0, field_paths no removed line, `guard_cell_digest` 9, the core's digest binaries 45 |
| GOLDEN | PASS: `guard_golden` 2, `guard_oracle_ac` 1 |
| SUBJ | PASS: `cog check 9bdfc1c..HEAD`, every subject at most 72 characters |

### 6.5 For the coordinator

1. **The ruling on F1 is in.** `isolation.harness_version_unreadable` is emitted for every harness. The retirement is a `retired = "<replacement>"` key appended to the old row: the old row's lines do not move, and the key is the smallest form that a reader of the row sees at once. `guard_refusal_codes` reads it, the contract pages list a retired code as a row that says it is retired and names its replacement, and no schema file changed. The commit carries `BREAKING CHANGE:`.
2. **The grep gate's last key** is written `"num_turns"` (quoted) in the roadmap leaf, not bare `num_turns`: the measured value paths `run.num_turns` and `run.cost_usd` live in `runner/evidence/run.rs` and a bare match would fail on them. I edited the leaf's gate line for this (the one `rdm` commit).
3. **`Observed` gained `totals` (`RunTotals`)**, an additive field with a default of none, and `RunFacts` a `new` constructor, because the struct is non-exhaustive. `candler-harness` is touched only in `spec.rs`. The OpenCode crate builds `Observed::new` and needed no change; it reports no totals, so its runs record no `run.usage`, `run.cost_usd` or `run.num_turns`, as before.
4. **`run_totals.json`** was recorded with the pre-change derivation (the last `result` event's keys), re-derived independently of the new code. `runner/evidence/run.rs` and the reader's result path are byte for byte what cf3de30 has, which the README says. The same producer, now through `Observed`, equals it on all eight fixtures.
5. **Ingest and mixed paths.** The reader of a raw path's streams is that of the first stream the registry recognised. With one registered harness this is Claude Code's, as before. A path that mixes streams of two harnesses reads all of them with the first one's reader until the core's factory takes the harness name; that is a core change outside this leaf.
6. Finding 8 of the first round (no test covers the `classify_end` wiring) is closed by the guard of 6.2. Finding 7 (`ingest` calls the Claude Code crate's `detect`) is closed by `DriverEntry.detect`.
