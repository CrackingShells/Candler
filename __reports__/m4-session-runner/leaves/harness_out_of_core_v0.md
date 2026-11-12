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
