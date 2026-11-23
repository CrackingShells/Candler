# Leaf report: cull_cli_harness (v0)

Cull of the CLI surface and harness drivers tests, from the `feat/diet` tip `0fcd6562`.

## Before and after

| Measure | Before | After |
|:--|--:|--:|
| Family tests (`guard_cli*`, `guard_inspect`, `guard_registry`, `guard_ground_truth_cli`, the harness crates' `tests/it`) | 138 | 123 |
| `cargo nextest list -p candler -p candler-harness -p candler-harness-claude-code -p candler-harness-opencode` | 205 | 190 |

The sweep counted 140 tests and about 81 kept. This leaf removes 15 and keeps 123, because every cull without a covering guard stayed (see Kept against the verdict).

## Deletions

| File | Test | Verdict | Covering guard or structure |
|:--|:--|:--|:--|
| `guard_ground_truth_cli` | `run_refuses_a_copy_and_accepts_a_tree_without_one` | deleted | `guard_ground_truth_refusal::refuses_ground_truth_in_every_agent_bound_place` (core: code, input and place named) and `guard_cli_e2e::i8_e2e::i8_canary_never_reaches_the_agent`, which the m5b report saw red when `register_ground_truth` leaves `compose_resolver` (the CLI wiring) |
| `guard_two_shapes` | 3 tests (document-shaped and lines-shaped harness drive both traits) | deleted | `conformance_shapes::conformance_lines_shape`, `conformance_document_shape`, `conformance_opencode_facts_shape` (the same driver shapes through the shared suite) |
| `guard_cell_reading` | 2 tests | deleted | structure: they test the test helper's cell accessors; every conformance fake reads its cell through them |
| `guard_unsupported_setting` | `an_unsupported_setting_is_one_coded_refusal_at_its_pointer` | deleted | `guard_opencode_driver::opencode_command_line_maps_or_refuses_every_setting` (refusal with code, side and pointer, read through `unsupported_of_cell`) |
| `guard_fixture_provenance` (claude-code) | `provenance_matches_the_fixture_files` | deleted | structure: fixture hashes |
| `guard_mcp_names` | `a_name_re_keys_the_server_and_leaves_no_name_leaf` | deleted | `guard_opencode_driver::opencode_a_server_name_renders_the_server_under_it` (same re-keying at the driver) |
| `guard_mcp_names` | `a_malformed_name_is_refused` | deleted | structure: a name grammar |
| `guard_cli_evaluate` | `e1_supervise_runs_an_evaluate_spec_written_by_hand` | deleted | `e4_evaluate_with_no_plugin_option_grades_exactly_the_runs_just_run` and `e1_evaluate_prints_the_job_id_on_stderr_and_the_report_on_stdout` run the same supervisor through the public verb; the hidden verb is structure |
| `guard_cli_evaluate` | `r2_a_suite_changed_after_the_job_started_prints_as_a_suite_record` | deleted | `r2_a_changed_suite_keeps_its_code_through_the_verb` (same code `evaluation.suite_changed`, side `suite`, through `evaluate`) |
| `guard_cli_evaluate` | `e1_evaluate_refuses_without_a_plugin_directory` | deleted | `r2_a_configuration_evaluate_cannot_use_is_a_config_record` (`config.no_plugin_dirs`, refused before any job, job directories counted). The message wording `plugins.dirs` is no longer pinned |
| `guard_opencode_driver` | `opencode_state_paths_are_the_recorded_layout` | deleted | structure: a layout pin; `conformance_opencode_facts_shape` runs the per-run paths |
| `guard_opencode_driver` | `opencode_capabilities_declare_no_session_and_outside_execution` | deleted | `conformance_opencode_facts_shape` (no session, census from the gateway record, execution outside the child) |
| `guard_cli_diff_refusal` | `r2_a_bad_equivalence_file_is_refused_with_its_code` | merged | moved whole into `guard_cli_refusal`, name kept; helper renamed `candler_in_home` |

The `support/mod.rs` of `candler-harness` lost `sid`, `ts`, `stream`, `Export` and `read_export`, which no remaining test used.

## Kept against the verdict

- `guard_mcp_names::two_servers_rendering_one_name_are_refused` stays: no other test refuses a colliding name.
- `guard_cli_evaluate` keeps 13 of 16, not about 10. The two lost-job rows (`e1_evaluate_says_when_its_job_was_lost`, `e1_a_lost_evaluate_job_is_recovered_with_no_docker_on_the_path`) observe different outputs (the command's message, `kill`'s docker-free recovery). The three `e5` rows and `r2_a_refusal_raised_in_the_job...` have no cover in another file.
- `guard_opencode_driver` keeps 12 of 14, not about 10. `opencode_version_is_parsed_from_the_probe` is the only test of the version probe; `opencode_detect_recognises_only_what_it_reads` has no conformance row.
- `guard_cli_surface` (u1, `every_data_verb_prints_its_record`) and `guard_cli_refusal` r2 are untouched.

## Bites

| Guard | Bite in product code | Seen red | Reverted |
|:--|:--|:--|:--|
| `guard_cli_refusal::r2_a_bad_equivalence_file_is_refused_with_its_code` | `diff.rs` message "is not valid" becomes "is invalid" | fails at the text-form assertion | yes |
| same | `evaluation/refusal.rs` code `suite.equivalence_invalid` becomes `suite.equivalence_malformed` | fails at the code assertion | yes |

A third attempt (the downcast to `EquivalenceError` removed from the classifier) stayed green: the code reaches the record through the catalogue, not through that function. It was reverted and is not a bite of this guard.

## Gates

fmt, workspace clippy, `cargo doc` with `-D warnings`, owned-crates nextest (190 passed), DIGEST (18 passed) and GOLDEN (10 passed) pass; no vector was edited; the ownership grep prints nothing.

## Follow-ups

- The `e5` environment rows and `guard_cli_run` hold overlapping treatment rows; a later pass with the isolation family can fold them.
- The leaf's closing commit (`reports(api): close the CLI surface and harness drivers cull`) is left to the coordinator with the verifier.

## Verifier

pending (coordinator)
