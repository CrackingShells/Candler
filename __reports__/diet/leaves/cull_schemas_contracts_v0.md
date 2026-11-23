# cull_schemas_contracts: leaf report v0

Leaf: `__roadmap__/diet/cull_schemas_contracts.md`. Branch `diet/cull_schemas_contracts`, cut from `feat/diet` at `139ff8ea`.

## Before and after

| Scope | Before | After |
|:--|--:|--:|
| Family (23 owned files, 139 tests) | 139 | 119 (one guard restored, one reader-only row added) |
| `candler-schemas` + `candler-core` + `candler` (nextest) | 785 | 765 |

Family before, by file: `catalog_properties` 24, `outcome_properties` 15, `guard_refusal` 11, `guard_execution_wire` 11, `guard_cell_digest` 9, `outcome_fixtures` 8, `guard_experiment_file` 8, `catalog_fixtures` 8, `contract_pages` 7, `guard_wire_shapes` 6, `guard_trace_wire` 5, `guard_name_grammars` 5, `guard_refusal_codes` (core) 4, `guard_evaluator_protocol` 4, `guard_jcs` 3, `schema_drift` 2, `guard_rpc_wire` 2, `guard_field_path_walk` 2, `guard_context_digest` 2, `guard_pattern_vectors` 1, `guard_import_origin` 1, `guard_event_kinds` 1.

## Deleted tests

21 tests go. Two files go whole, five files lose rows. No digest or golden vector was touched.

| File | Test | Verdict | Covering guard or "structure" |
|:--|:--|:--|:--|
| schemas `guard_name_grammars` | `from_lossy_folds_an_error_type_to_a_reason_code` | delete | structure: pins the output of a helper it replaced |
| schemas `guard_name_grammars` | `is_env_var_name_is_posix` | delete | structure: helper table |
| schemas `guard_name_grammars` | `plain_segment_and_plain_arg_differ_by_the_leading_dash` | delete | structure: helper table |
| schemas `guard_name_grammars` | `segment_of_collides_where_the_fold_does` | delete | structure: helper table |
| schemas `guard_name_grammars` | `segment_of_folds_a_name_to_a_field_path_segment` | delete | structure: helper table |
| schemas `guard_field_path_walk` | `nest_wraps_one_table_per_segment` | delete | structure: the walker of a field path, no consumer sees it |
| schemas `guard_field_path_walk` | `relative_to_matches_whole_segments_only` | delete | structure: same |
| core `guard_refusal_codes` | `r1_a_wrapper_answers_with_the_side_of_what_it_wraps` | delete | `guard_refusal_codes::r1_every_wrapper_answers_with_a_registered_code_and_side` (same file, same interface, runs by default, walks every delegate row with the wrapped error's code; bite A below turns it red) |
| schemas `catalog_properties` | `catalog_unknown_tool_keys_pass_the_schema_and_are_kept_by_the_reader` | delete | `catalog_snapshot_json_with_arbitrary_extra_keys_is_valid_and_round_trips` (same file, the reader keeps extra tool keys and the snapshot validates); the schema half is `schema_drift` |
| schemas `catalog_properties` | `catalog_minimal_snapshot_is_valid_in_both` | delete | `catalog_snapshot_built_through_the_builders_writes_valid_json_and_reads_back_equal` and the `catalog_fixtures` round-trip rows |
| schemas `catalog_properties` | `catalog_integral_float_keeps_its_literal` | delete | structure: pins how `serde_json` holds a number |
| schemas `catalog_properties` | `catalog_integer_beyond_u64_is_not_kept_exactly` | delete | structure: documents a limit of the JSON library |
| schemas `catalog_properties` | `catalog_object_key_order_is_not_kept` | delete | structure: documents serialisation order, equality is by value |
| schemas `catalog_properties` | `catalog_number_beyond_f64_is_refused` | delete | structure: documents a limit of the JSON library |
| schemas `outcome_properties` | `outcome_pristine_bases_are_valid_for_both_readers` | delete | structure: checks the test's own base records |
| schemas `outcome_properties` | `outcome_json_schema_reads_3_0_as_an_integer_and_the_rust_reader_does_not` | delete | structure: schema-vs-reader divergence pin; the schema half is `schema_drift`; the integer-literal wire is held by `outcome_writers_emit_integer_literals_and_both_readers_read_a_number_written_as_an_integer` |
| schemas `outcome_properties` | `outcome_the_schema_cannot_compare_weight_num_with_den_and_the_rust_reader_does` | delete | structure: divergence pin (see Follow-ups for the reader-only half) |
| schemas `outcome_properties` | `outcome_the_schema_states_only_the_blocked_weight_rule_and_the_rust_reader_states_all_three` | delete | structure: divergence pin; the status and weight combinations are rows of `outcome_every_mutation_is_refused_by_both_readers` |
| schemas `outcome_properties` | `outcome_the_schema_bounds_an_integer_only_by_its_type_and_the_rust_reader_by_64_bits` | delete | structure: divergence pin |
| schemas `outcome_properties` | `outcome_run_key_counters_past_32_bits_pass_the_schema_and_fail_the_reader` | delete | structure: divergence pin (see Follow-ups) |
| schemas `outcome_properties` | `outcome_the_schema_cannot_order_the_two_lines_of_a_range_and_the_rust_reader_does` | delete | structure: divergence pin (see Follow-ups) |

## Kept against the sweep's verdict

| Item | Sweep | Kept because |
|:--|:--|:--|
| `guard_import_origin` (1 test) | delete whole file | Its only test reads an import record with no base commit and no legacy block, the wire of a raw trace. `guard_execution_wire` only reads records that carry both. No other schemas test holds it, so there is no covering guard. |
| `guard_event_kinds` (1 test) | delete whole file | Its only test round-trips `evaluation.planned` and checks it cannot be built as an unknown kind. `every_first_set_event_kind_round_trips_its_wire_form` covers the first-set kinds and does not name this one. |
| `guard_wire_shapes` (6 tests) | merge into `guard_execution_wire`, 3 survive | The three `agent_environments_*` rows are the plug-in manifest contract. The other three have no cover: `tagged_enums_refuse_the_list_form...` is the only list-form test of `ValueSource`, `ProcessStart` and `ExitStatus` (`execution_records_refuse_the_list_form` names none of them); `derived_source_carries_an_optional_location` is the only hand-written wire text of a derived source; `catalog_wire_structs_are_refused_as_lists_in_every_fixture` is the only list-form test of the catalog wire. Merging would move six tests and their helpers for no deletion, so the file stays as it is. |
| `guard_refusal_codes::r1_every_error_enum_is_coded_or_internal` | delete (structure) | Restored in the fix round. The kept rows iterate `candler_core::refusal::catalog()`, so a new error type nobody registers is reached by neither of them; only this scan sees it, and refusal/1 codes are consumer interface 1. |
| `guard_refusal_codes` merge into schemas `guard_refusal` | merge, 2 survive | The file uses `candler_core::refusal::catalog`, `candler_harness` and core error types, which the schemas test crate cannot depend on. The two surviving rows stay in core: `r1_every_variant_code_equals_its_row` and `r1_every_wrapper_answers_with_a_registered_code_and_side`. |
| `catalog_properties`: 18 kept of 24 (sweep: about 8); `outcome_properties`: 8 kept of 15 (sweep: about 6) | | The rest each refuse a bad snapshot or a bad record at the reader (bad digest, bad source path, unknown transport, empty or repeated names, repeated keys, depth limit, non-string environment, `a_mutated_generated_record...`) and no other test holds that refusal. Tests of `contract_pages` (7) and the other 14 owned files were judged and all kept: each observes a wire format, a digest vector or a generated page that a plug-in author reads. |

## Bites

`codes.toml` and the product code were not left edited; each bite was reverted and `git status` showed only test files.

| Bite | Guard seen red | Reverted |
|:--|:--|:--|
| A: `answer_of` in `crates/candler-core/src/refusal.rs` returns `inner` instead of `find(inner)` | `r1_every_wrapper_answers_with_a_registered_code_and_side` red, `r1_every_variant_code_equals_its_row` green | yes |
| B: the code `config.file_unreadable` in `crates/candler-core/src/resolve/refusal.rs` renamed | `r1_every_variant_code_equals_its_row` red, the wrapper guard green | yes |
| C (control, not a bite that bit): the `NonUtf8Path` side changed to `Suite` | variant guard red | yes |
| D (control, not a bite that bit): `find` stops after the first link | both green: every wrapper implements `Refusal` itself, so the walk never needs a second link | yes |

The tests removed from `catalog_properties` and `outcome_properties`, and the two whole files, change no guard, so they need no bite. Both rows of `guard_refusal_codes` are unchanged in body.

## Gates

| Gate | Result |
|:--|:--|
| `cargo nextest run -p candler-schemas -p candler-core -p candler` | 763 passed |
| `cargo fmt --check` | pass (after `cargo fmt` removed two blank lines left by the deletions) |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | pass |
| DIGEST | 18 passed, no vector edited |
| GOLDEN | 10 passed, no vector edited |
| Ownership grep | prints nothing |

## Follow-ups

- Accepted loss: the guard that the Rust reader refuses an integral float for an integer field was held only by the deleted divergence pins; it is now the reader-only row `outcome_the_rust_reader_refuses_an_integral_float_for_an_integer_field` (kept `outcome_properties`; seen to discriminate by asserting the integer form accepted and the `2.0` form refused).
- The three reader-only refusals of the deleted divergence pins are held by default-run `#[cfg(test)]` tests in `candler-schemas/src`: weight `num` above `den` by `outcome.rs` `outcome_weight_follows_the_rule_of_the_status`, `outcome_reader_runs_the_cross_field_rules` and `outcome_reader_refuses_malformed_members`; range line order by the `EvidenceRef` reject list in `primitives.rs` (`path:a#L9-3`); run-key counters past 32 bits by `primitives.rs` `run_key_counters_past_32_bits_pass_the_schema_and_fail_the_reader`.
- The generated-record property `outcome_a_mutated_generated_record_is_refused_by_both_readers` repeats the table in `outcome_every_mutation_is_refused_by_both_readers` under proptest. It sits inside a `proptest!` block and was left in place.

## Verifier

pending (coordinator)
