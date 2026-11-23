# cull_supervision leaf report (v0)

Leaf: `__roadmap__/diet/cull_supervision.md`. Branch `diet/cull_supervision`, cut from `feat/diet` at `0fcd6562`.

## Before and after

`cargo nextest list -p candler-core` on macOS, counted by file.

| Family file(s) | Before | After |
|:--|--:|--:|
| `guard_supervise` | 22 | 18 |
| `guard_signal` | 9 | 11 |
| `guard_proc_stat` | 9 | 0 |
| `guard_session` | 10 | 9 |
| `guard_controls` | 4 | 4 |
| `guard_wiring` | 3 | 3 |
| `guard_platform_seam` | 3 | 0 |
| `guard_detach` | 1 | 1 |
| `guard_spec_file` | 4 | 2 |
| `guard_job_*` | 12 | 11 |
| **Family** | **77** | **59** |
| `-p candler-core` (all tests) | 406 | 388 |

The sweep counted 82. The five missing rows are Linux-only tests that do not list on macOS. One of them, `s4_a_real_process_with_a_hostile_name_reads_back_its_true_parent`, is deleted below. The `guard_job_*` rows include the two re-exec entries `reap_child` and `supervisor_child`, which stay because b32, b33 and b42 call them.

## Deletions

| File | Test | Verdict | Covering guard |
|:--|:--|:--|:--|
| `guard_platform_seam` | `q1_every_inventory_entry_names_its_use_and_a_windows_counterpart` | structure | structure: scans source for an inventory list |
| `guard_platform_seam` | `q1_the_binary_states_its_targets_with_one_compile_error` | structure | structure: scans source for a `compile_error!` |
| `guard_platform_seam` | `q1_unix_apis_stay_inside_the_inventory` | structure | structure: scans source for unix API names |
| `guard_job_spec_file` | `p0_run_spec_file_bytes_equal_the_job_spec` | structure | structure: pins a writer's byte form; the spec digest is held by the DIGEST guards |
| `guard_spec_file` | `o3_spec_file_holds_no_secret_value` | repeat | `crates/candler/tests/it/guard_cli_observe.rs::o3_launched_job_spec_json_holds_no_secret_value` (CLI, same file, same planted value) |
| `guard_spec_file` | `spec_file_child` | helper of the deleted test | none needed |
| `guard_supervise` | `reaping_a_group_the_supervisor_holds_leaves_it_held` | structure | structure: reads the supervisor's group table through `holds_group` |
| `guard_supervise` | `a_group_is_forgotten_when_it_is_empty_and_kept_while_it_has_members` | structure | structure: the group table; the observable end (no survivor) is `a_stopped_and_a_reaped_tree_leave_no_survivor` |
| `guard_supervise` | `handles_carry_bytes_requests_and_captures` | structure | structure: pins the stdin, stdout and capture handles, which no consumer sees |
| `guard_supervise` | `record_on_disk_equals_what_the_os_reports` | repeat | `a_stopped_and_a_reaped_tree_leave_no_survivor` reaps from the record alone, so a wrong pid, group or start fails it; `a_forged_record_does_not_read_alive` holds the start |
| `guard_session` | `harness_env_session_adds_input_flag_only_when_interactive` | structure | structure: pins an argv baseline against `argv.json` |
| `guard_proc_stat` | `s4_cross_check_accepts_a_disagreement_that_clears_on_a_reread` | structure | structure: reader-script rows of the `/proc` cross-check |
| `guard_proc_stat` | `s4_cross_check_accepts_a_group_disagreement_that_clears_on_a_reread` | structure | structure (same) |
| `guard_proc_stat` | `s4_cross_check_refuses_a_group_that_stat_and_status_disagree_on` | structure | structure (same) |
| `guard_proc_stat` | `s4_cross_check_refuses_when_stat_and_status_disagree_on_every_read` | structure | structure (same) |
| `guard_proc_stat` | `s4_cross_check_refuses_when_status_cannot_be_read_or_parsed` | structure | structure (same) |
| `guard_proc_stat` | `s4_proc_stat_refuses_a_line_that_is_not_a_stat_line` | repeat | `s4_proc_stat_reads_every_field_for_any_command_name` (same parser) and the signal-guard refusals |
| `guard_proc_stat` | `s4_proc_status_reads_ppid_and_the_leftmost_nspgid_by_key` | structure | structure: literal status texts |
| `guard_proc_stat` | `s4_a_real_process_with_a_hostile_name_reads_back_its_true_parent` (Linux only) | repeat | `s4_proc_stat_reads_every_field_for_any_command_name` holds the same parser and the same bites (`find` for `rfind`, a field off by one) with literal hostile names. Not run on macOS; see Follow-ups |

`guard_proc_stat` is merged into `guard_signal`: `s4_proc_stat_reads_every_field_for_any_command_name` and `s4_cross_check_refuses_files_that_name_another_pid` move with their helpers `tail` and `pair`, names unchanged. Both are pure, so the "never spawns" header of `guard_signal` stays true.

`guard_job_spec_file`, `guard_platform_seam` and `guard_proc_stat` go as whole files with their `mod` lines. Helpers left unused by the cuts are removed (`PsRow`, `ps_row`, `shell_ticks`, `shell_boot_id`, `Probe` in `guard_supervise`; `pre_stops`, `steps`, `wait_for` and the `events` field in `support/wiring.rs`).

## Kept against the sweep's verdict

| Test | Why it stays |
|:--|:--|
| `guard_spec_file::o3_evaluate_spec_file_holds_no_secret_value` | The CLI guard holds `candler run` jobs only. No other test reads the spec file of an evaluate job. |
| `guard_wiring::wiring_collect_exports_before_the_destroy_and_the_export_is_the_trace` | The only test that the trace export runs before the container is destroyed. `guard_container_record::create_collect_destroys_the_runs_own_container` checks the destroy, not the order. |
| `guard_wiring::wiring_resolved_the_run_keeps_its_argv_and_prepared_files_without_secrets` | The only test of the stored `resolved` record (argv, overlay names, prepared files, no canary). No guard found that reads that stored artifact. |
| `guard_supervise` the 18 left | Each is "no process left", "no secret on disk", the record lifecycle, the loopback-only pre-stop, or the ladder's bounds (`the_deadline_counts_from_the_spawn`, `a_grandchild_holding_the_pipe_does_not_hold_wait`, `a_sweeper_that_overruns_the_grace_is_not_waited_for`, `a_failed_pre_stop_still_reaches_term_then_kill`). None of the bounds has another test, so they stay although they pin timing. |
| `guard_wiring::wiring_stop_execs_in_the_container_with_the_observed_id` | Restored after the verifier's finding. The only test of three promises: `{session_id}` is filled from the live reader's observed id and is the last exec argument, "skipped: no session id observed yet" when none was seen, and "exec failed" lets the ladder go on. No other test runs a pre-stop exec. |
| `guard_supervise::reap_refuses_a_record_whose_group_the_process_does_not_lead` | Restored. The only pin of `ReapRefusal::GroupMismatch`; `reap_signals_nothing_for_a_record_it_cannot_verify` asserts only that a refusal exists. |
| `guard_supervise::a_child_that_exits_by_itself_reports_spawn_sweep_exit` | Restored. A natural exit (`Ended::Exited`, one sweep, no signal steps) is another path from the stopped child. |
| `guard_session::session_close_follows_the_turns` | Overlaps `session_close_wired_through_the_run_loop` on the idle rule, but only this one holds the repeated `Stop` line and the coalesced `num_turns = 0` result. |
| `guard_controls` (4) | Each is a behaviour of the core operation API (reuse key, refusal, record on the run, verdict independent of the model). No two repeat. |

## Bites

Each merged guard was seen red once through a change in `crates/candler-core/src/supervise/platform.rs`, then reverted (`git diff` of that file is empty).

| Bite | Guard seen red | Reverted |
|:--|:--|:--|
| `rfind(')')` replaced by `find(')')` in `ProcStat::parse` | `guard_signal::s4_proc_stat_reads_every_field_for_any_command_name` | yes |
| the pid check in `read_proc_checked` replaced by `if false` | `guard_signal::s4_cross_check_refuses_files_that_name_another_pid` | yes |

The bite recorded in `guard_wiring` and `guard_spec_file` is unchanged: no kept test in them was rewritten.

## Gates

| Gate | Result |
|:--|:--|
| `cargo nextest run -p candler-core` | 388 passed (after the fix round; `the_parents_environment_reaches_neither_the_child_nor_its_pre_stop` and `every_failure_path_of_an_evaluator_is_accounted_for` failed once under load 35 and passed on retry) |
| `cargo fmt --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | pass |
| DIGEST | 18 passed, no vector edited |
| GOLDEN | 10 passed, no vector edited |
| Ownership grep | prints nothing |

One slip on the way: dropping the `rig` field of `support/wiring.rs::Ran` freed the store tempdir at the end of `run`, and `guard_census_gateway` (not owned) failed. The field is back as `_rig`, with a doc line that says why.

## Follow-ups

- The deleted Linux-only `s4_a_real_process_with_a_hostile_name_reads_back_its_true_parent` was the one test where the kernel writes the `stat` line. It could not be compiled or run on this macOS host. If a Linux CI run shows a field-index drift the literal rows miss, bring it back in `guard_signal` (it spawns, so it would need its own binary).
- `guard_supervise` still has four timing bounds with no second guard. A later cull that moves them into a job-level test could drop them.
- `support/wiring.rs` stays because `guard_census_gateway` uses it. That piece can reduce it if it culls.

## Verifier

pending (coordinator)

## Fix round

The verifier found three deletions with no covering guard. All three are restored above. `support/wiring.rs` and `guard_wiring.rs` are back to their `feat/diet` text. `wiring_stop_execs_in_the_container_with_the_observed_id` takes about 90 s under a machine load near 35 (its helper `wait_for` gives the container record 10 s) and failed once for that reason before it passed on retry.
