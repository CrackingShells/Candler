# cull_plan_store: leaf report v0

Leaf: `__roadmap__/diet/cull_plan_store.md`. Branch `diet/cull_plan_store`, cut from `feat/diet` at `0fcd6562`.

## Before and after

| Count | Before | After |
|:--|--:|--:|
| Tests in the owned files (`tests/it`, candler-core) | 77 | 69 |
| `cargo nextest list -p candler-core` | 406 | 398 |

The diet plan counted 83 tests for the family against about 59 kept. The 83 includes tests outside the owned files and the `PlanOutcome::summary` test, which this leaf leaves to `shed_dead_dup`. Judged test by test, the owned files hold 77 tests, and 8 of them go (69 remain). The sweep's guess for `guard_run_plan` (9 of 16 kept) did not hold: 15 of 16 stay, because each remaining row observes a behaviour no other row at the same interface observes.

## Deletions

| File | Test | Verdict | Covering guard |
|:--|:--|:--|:--|
| `guard_tree_walk` | `a_listing_is_sorted_by_the_joined_string` | repeats a kept guard | `guard_build_context::guard_build_context` (the pinned digest holds `a-b` before `a/b`) |
| `guard_tree_walk` | `each_link_policy_treats_a_link_as_named` | pins the walker's policy enum | `guard_build_context::guard_build_context` (links recorded, never followed) and `the_files_of_a_run_leave_out_links` (links skipped); `Refuse` is used by `experiment/inputs.rs` and held by `guard_declared_inputs` ("symlink in a directory") |
| `guard_tree_walk` | `a_name_rule_refuses_or_skips_by_name` | pins a closure hook no consumer sees | structure |
| `guard_resolve` | `e2_slots_evaluators_is_a_known_path` | repeats a kept guard | `guard_resolve::e2_slots_evaluators_is_a_setting_not_a_cell_value` (asserts `is_setting` for both paths) |
| `guard_resolve` | `value_at_refuses_a_bad_path_as_failed` | pins an internal step-context accessor | structure; no remaining test pins the `PreflightError::Failed` text for a malformed path (accepted loss) |
| `guard_run_plan` | `b3_same_seed_same_order` | repeats a kept guard | `guard_run_plan::b3_plan_runs_uses_the_experiment_seed` (one file planned twice plans one order) |
| `guard_settings_out_of_cell` | `t2_treatment_is_the_one_admitted_setting` | treatment-level row repeated | `guard_run_plan::t3_each_cell_uses_its_level` and `t3_treatment_table_levels_admit_four_fields` (a treatment factor plans one cell per level) |
| `guard_settings_out_of_cell` | `t2_one_treatment_digest_unchanged` | treatment-level row repeated | `guard_run_plan::t3_one_level_plans_the_old_bytes`, an equivalent pin (experiment digest of a different one-level file), not the same literal |

The constant `ONE_TREATMENT_DIGEST` goes with the last row (its only user). Both walker tests that survive moved into `guard_build_context` with their names; they read the `write_context` tree, so the run-files row lists the non-ASCII file too.

## Merged

| From | Into | Tests moved |
|:--|:--|--:|
| `guard_tree_walk` | `guard_build_context` | 2 (`a_staged_copy_has_the_digest_of_its_listing`, `the_files_of_a_run_leave_out_links`) |

`guard_build_context` keeps its two pinned literals unchanged.

## Kept against the sweep or the ask

- `guard_assert_clean` (1): restored after the verifier's finding. It holds the only checks that the secret scanner finds a needle inside a gzip file, reports hits sorted and refuses an empty directory; without it a broken scanner would let every secrets-never-on-disk guard pass vacuously, and `guard_protocol_client` names it as its oracle.

- `guard_run_plan`: 15 of 16 kept. `b3_other_seed_other_order` and `b3_each_round_holds_each_cell_once` look like repeats of `b3_plan_runs_uses_the_experiment_seed` but observe `run_order` (seed ignored, rounds mixed), which that row does not. `t3_check_facts_per_level` is the multi-level case that `b27` (one level) does not reach. `t3_one_level_plans_the_old_bytes` holds digest and order literals of the experiment digest.
- `guard_plugin_environment` (11): `equivalent_appended_requirements` looks close to `equivalent_values_and_lock`, but only it holds the order and the once-only rule of an appended list. `a_plugins_mcp_servers_reach_claude_code` and `..._opencode` are the plug-in contract, one per driver.
- `guard_treatment_context::ground_truth_in_the_treatment_context_is_refused_before_any_build` repeats I8 at another path (the treatment context), so it stays.
- `guard_seams::a_secret_in_the_environment_reaches_no_file` is the one secrets-never-on-disk row at the op API in this family.
- `guard_run_evidence::b39_an_environment_at_the_root_hides_the_workspace` stays: the root marker is a case the nested-tree row does not hold.
- `guard_run_plan::the_summary_of_a_plan_holds_the_caps_of_the_file` (the `PlanOutcome::summary` test) is left to `shed_dead_dup`.

## Bites

| Guard | Bite in product code | Seen red | Reverted |
|:--|:--|:--|:--|
| `guard_build_context::a_staged_copy_has_the_digest_of_its_listing` | `Listing::stage` re-makes each link with a fixed target text (`tree.rs`) | yes: digest of the copy differs | yes |
| `guard_build_context::the_files_of_a_run_leave_out_links` | `collect_files` walks with `Links::Record` instead of `Links::Skip` (`gather.rs`) | yes: the directory link is read as a file and the walk fails | yes |

No other guard changed.

## Gates

| Gate | Result |
|:--|:--|
| `cargo nextest run -p candler-core` | 398 passed |
| `cargo fmt --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | pass |
| DIGEST filter | 20 passed, no vector edited |
| GOLDEN filter | 10 passed, no vector edited |
| Ownership grep | prints nothing |

## Follow-ups

- `t3_one_level_plans_the_old_bytes` pins the planner's output from before treatment levels existed. With no external users, that compatibility claim is a candidate for the supervisor to rule on.
- `guard_resolve::steps_run_in_order_and_every_failure_is_reported` (a 130-line test of the `PreflightStep` contract) and `guard_resolve::the_default_store_root_...` were kept; a later pass could split or trim them.

## Verifier

PASS with findings, no blocker, then fixed. Deleting `guard_assert_clean` left the scanner unguarded; it was restored in 54ddabea and three report rows were corrected.
