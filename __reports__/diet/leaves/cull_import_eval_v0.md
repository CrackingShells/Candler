# cull_import_eval: leaf report v0

Leaf: `__roadmap__/diet/cull_import_eval.md`. Branch `diet/cull_import_eval`, cut from `feat/diet` at `139ff8ea`.

## Before count

`cargo nextest list -p candler-core -p candler`: 560 tests. The family's files held 92 listed tests (engine 11, eval_infra 15, evaluate_job 5, import_mini 11, ingest 17, layering_i1 10, legacy_mapping 8, oracle_mini 5, package_names 1, protocol_client 3, registry 6) plus the `#[ignore]`d `oracle_aggregates` and `oracle_ac`, which need `CANDLER_GOLDEN_CORPUS`. After: 543 in the two crates, and 75 listed tests in the family's files. `guard_golden` and `guard_golden_parity` are untouched.

## Deletion table

| File | Test | Verdict | Covering guard |
|:--|:--|:--|:--|
| guard_oracle_mini | the_mini_corpus_yields_the_hand_counted_rows | tests the oracle helper | `guard_import_mini::the_store_holds_the_oracle_rows_and_every_difference_is_explained` (importer and oracle are independent derivations of the 136 rows; the test also holds the 6 and 136 literals) |
| guard_oracle_mini | weights_follow_the_status_defaults | tests the oracle helper | same; `guard_import_mini::mapping::unusual_grades_get_the_causes_the_leaf_names` holds the zero-weight literals |
| guard_oracle_mini | causes_follow_the_legacy_fields | tests the oracle helper | same row equality (causes are in the projection) |
| guard_oracle_mini | the_number_token_decides_the_value_kind | tests the oracle helper | same row equality (values are in the projection) |
| guard_oracle_mini | run_facts_come_from_the_verdicts_streams_and_metadata | tests the oracle helper | `mapping::the_mapped_runs_are_the_oracle_runs` and `mapping::contaminated_runs_have_no_records_and_incomplete_runs_have_all` |
| guard_oracle_aggregates | oracle_aggregates (ignored) | tests the oracle helper | `guard_golden::the_golden_corpus_imports_to_the_oracle_rows` (240 runs, 10,320 rows, the 8,291, 802, 742 and 485 status counts). The cover is `#[ignore]`d and corpus-gated, as the deleted test was; the per-role score sums have no cover (accepted loss, Follow-ups) |
| guard_oracle_ac (candler) | oracle_ac_rederived (ignored) | tests the oracle generator | none for the re-derivation: `guard_golden_parity` only consumes the committed `equivalence.json`, and is itself corpus-gated. Accepted loss (Follow-ups) |
| guard_layering_i1 | workspace_holds_no_domain_segment | source scan | structure; the README claim is held by the two rows moved into `guard_import_mini` |
| guard_layering_i1 | workspace_tracks_no_file_under_a_skipped_name | source scan | structure |
| guard_layering_i1 | tracked_check_names_the_tracked_file_under_a_skipped_name_only | self-test of the scanner | structure |
| guard_layering_i1 | plant_table_is_found_or_passes_as_written | self-test of the scanner | structure |
| guard_layering_i1 | harness_plants_are_found_only_in_the_core_and_the_schemas | self-test of the scanner | structure |
| guard_layering_i1 | plant_inside_a_gzip_file_is_found | self-test of the scanner | structure |
| guard_layering_i1 | plant_symlink_or_binary_file_under_assets_fails_the_scan_naming_it | self-test of the scanner | structure |
| guard_layering_i1 | segments_split_on_case_and_an_allowlist_entry_excuses_one_line_only | self-test of the scanner | structure |
| guard_legacy_mapping | the_mapped_values_hold_no_absolute_path | repeats a kept guard | `guard_import_mini::the_store_holds_relative_paths_only` (the same I17 promise over the same corpus, read from the store) |
| guard_protocol_client | no_evaluation_module_signals_a_process | structure (a source scan, like `guard_layering_i22`); I22 is held at the `Guard` | `guard_signal::the_guard_refuses_what_it_did_not_spawn_and_signals_nothing` tests the `Guard`, not that `src/evaluation/` never signals; the `Recording` signaller of `every_failure_path_of_an_evaluator_is_accounted_for` shows the client's own stops reach only the spawned group |
| guard_eval_infra | the_binary_reads_the_names_the_build_exports | reads `supervise.rs` as text | structure; the `git_*` and `packaged_*` rows read the identity through `eval_infra()` |
| guard_eval_infra | this_checkout_records_the_full_commit | repeats a kept guard | `git_checkout_clean_records_the_full_commit` (the 40-hex commit `git rev-parse` names, on a checkout the test builds) |

Support: the segment scanner (`SegmentRules`, `SegmentHit`, `SegmentScan`, `split_run`, `chunk_segments`, `denied_in`, `scan_file`, `scan_segments`) is deleted from `support/mod.rs`; only `guard_layering_i1` used it. `support/wiring.rs` is untouched.

## Merged and rewritten

- `guard_layering_i1` becomes two rows in `guard_import_mini`: `a_renamed_domain_imports_to_the_same_counts_and_leaves_no_name_in_the_store` and `without_markers_every_unmeasured_record_is_unrecorded`. The first checks the store with a 13-segment check (`assert_no_domain_name`) in place of the scanner, and compares every projected row (expectation id, role, status, value, weight, cause) and every manifest factor list with the oracle's, the domain names renamed in them, not only the counts.
- `guard_legacy_mapping` moves into `guard_import_mini` as `mod mapping`, with 7 tests (below).
- `guard_package_names` moves into `guard_import_mini` as `a_package_name_is_a_field_path_segment`.

## Kept against the sweep's verdict

- `guard_legacy_mapping`: the sweep kept 4 of 8; 7 stay. I deleted only the one test whose behaviour `guard_import_mini` observes. The others have no covering guard: `the_mapped_records_are_the_oracle_rows` alone holds that the records of one expectation id share one suite entry digest; `contaminated_runs_have_no_records_and_incomplete_runs_have_all` alone holds that an `incomplete` run is valid; `unusual_grades_get_the_causes_the_leaf_names` holds three injected grades that no corpus row has; `the_mapped_runs_are_the_oracle_runs` holds the factor lists and timestamps; `the_mapped_identity_is_the_oracle_vectors` holds the digest vectors; `every_fault_is_refused_naming_its_file_and_run` is the error contract; `every_file_is_accounted_for_exactly_once` is I12.
- `guard_ingest` keeps all 17, not 11. Each test holds a distinct CLI record, exit code or refusal code, or an op-API refusal at the other interface (`i1_a_directory_with_no_stream_is_refused` is the API twin of the CLI `i1_a_path_with_no_stream_is_refused`). I found no pair where one observes the other's behaviour at the same interface, and I did not check `guard_cli_refusal`, so I left the refusal rows.
- `guard_engine` (11), `guard_evaluate_job` (5), `guard_registry` (6), `guard_golden`: kept. They hold the plug-in contract, E1 to E3 process isolation and golden vectors.

## Flake fix

`every_failure_path_of_an_evaluator_is_accounted_for`, row `late.sh`. The stub sleeps 0.9 s and then answers; the row needs the answer after the 400 ms deadline and inside the 1.5 s cancel grace. On a loaded machine the stub starts late, so the answer misses the grace: the client stops the stub and the row sees an `instrument_error` or an unexpected stop. The fixture is outside the owned set, so the window moved: `Row` gains `cancel_grace_ms` (1500 for every other row) and the `late.sh` row uses 60,000. The lower bound is causal (the deadline clock starts before the process does, and the stub waits 0.9 s after it starts), and the upper bound is a minute, which no load adds. The row still ends in about a second. No retry and no longer sleep.

Gate: 20 of 20 passes with `--retries 0` on a quiet machine, and 10 of 10 with twice as many busy loops as cores running.

## Bites

Each bite is a one-line edit of product code; each was reverted with `git checkout` and the tree was clean of it afterwards.

| Bite | Guards seen red | Reverted |
|:--|:--|:--|
| `import/grading.rs` `marker_code`: the captured code becomes `htn_{code}` (the importer writes a domain name) | `a_renamed_domain_imports_to_the_same_counts_and_leaves_no_name_in_the_store`, `mapping::the_mapped_records_are_the_oracle_rows`, `the_store_holds_the_oracle_rows_and_every_difference_is_explained`, `entry_params_come_from_the_rules` | yes |
| `import/grading.rs` `marker_code`: the fallback becomes `htn_unrecorded` | `without_markers_every_unmeasured_record_is_unrecorded` | yes |
| `isolation/image.rs` `package_name_ok`: the `.` check dropped | `a_package_name_is_a_field_path_segment` | yes |
| `import/grading.rs` `marker_code`: markers apply only when the evidence holds `TIER_D` (the importer needs the domain name; no status changes, only causes) | `a_renamed_domain_imports_to_the_same_counts_and_leaves_no_name_in_the_store` (the counts-only version stayed green) | yes |

## Gates

`cargo nextest run -p candler-core -p candler`: 543 run, 543 passed, 30 skipped. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass. DIGEST: 18 passed. GOLDEN: 10 passed. No vector file is edited. The ownership grep prints nothing.

## Follow-ups

- `guard_ingest` and `guard_cli_refusal` may hold the same refusal codes (`import.no_stream`, `import.rules_required`); a piece that owns both can check and drop one side.
- `mod mapping` inside `guard_import_mini` still carries its own fixture-editing helpers (about 400 lines); `guard_import_mini` and `mapping` could share `support::legacy_import` for `Imported` if a later piece wants one reader.
- Accepted loss: with `oracle_aggregates` and `oracle_ac` gone, nothing re-derives the A-C reference files (counts.toml, per-cell sums, equivalence pairs, D-E counts) from the corpus any more; `guard_golden` and `guard_golden_parity` run only with `CANDLER_GOLDEN_CORPUS` and take those files as given.
- Residual: a direct signal from an evaluation module that bypasses the `Guard` is no longer caught by a test; I22 is held at the `Guard` only.
- The renamed-domain row checks snake and kebab segments, not camel-case splits or the `plan trace` sequence the scanner held.

## Verifier

pending (coordinator)
