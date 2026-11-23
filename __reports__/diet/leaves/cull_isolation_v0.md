# cull_isolation: leaf report (v0)

Leaf: `__roadmap__/diet/cull_isolation.md`. Branch `diet/cull_isolation`, cut from `feat/diet` at `0fcd6562`.

## Before count

Owned files, counted with `cargo nextest list -p candler-core` (default run plus `--run-ignored only`):

| Run | Family tests | `-p candler-core` tests |
|:--|--:|--:|
| Before | 90 (71 default, 19 ignored e2e) | 429 (406 default, 23 ignored) |
| After | 87 (68 default, 19 ignored e2e) | 426 (403 default, 23 ignored) |

The sweep's "100" counted the family loosely; the owned-file count above is the one measured. Three tests go; the rest of the sweep's 39 culls fall in files this leaf's verdict list does not name (see Follow-ups).

## Deleted and merged tests

| File | Test | Verdict | Covering guard |
|:--|:--|:--|:--|
| `guard_layering_i22` | `b16_destroy_verbs_only_in_guard` | deleted | structure (it scans the source for destroy verbs); destroy discipline is held by `guard_container_guard::b10_guard_refusals` and `container_stop_e2e::b13_client_killed_container_gone` |
| `guard_egress_proxy` | `plan_agent_networks` | deleted | structure (the internal and allow network of an agent are plan rows); the allowlist e2e observes the agent's network membership (bite `agent_on_egress`) |
| `guard_egress_proxy` | `plan_allow_network_names_follow_the_allowlist` | kept | two allowlists never share a network is held by nothing else (the allowlist e2e builds one proxy from one list) |
| `guard_egress_proxy` | `plan_one_proxy_per_distinct_allowlist` | kept | per-allowlist separation is held by nothing else |
| `guard_egress_proxy` | `plan_proxy_has_no_listed_alias_and_agents_get_add_host` | deleted | structure (argv shape); the promise it protects is observed by `egress_proxy_never_resolves_a_listed_host_to_itself` (kept) |
| `guard_egress_proxy` | `plan_session_starts_and_ends_one_proxy_per_allowlist` | kept | one proxy started and ended per allowlist is held by nothing else |
| `guard_egress_proxy` | `plan_render_one_host_per_port` | moved to `guard_egress_setting` | the config text is what nginx runs; kept, name unchanged |
| `guard_egress_proxy` | `plan_render_shared_port_needs_sni` | moved to `guard_egress_setting` | kept, name unchanged |
| `guard_unstarted_destroy` | `unstarted_destroy` | moved to `guard_container_record::unstarted_destroy` | kept, name unchanged |
| `guard_unstarted_destroy` | `unstarted_destroy_session_start` | kept, moved with its helper to `guard_container_record::unstarted_destroy` | no covering guard (see Bites and Kept against the sweep's verdict) |
| `create_e2e` | `agent_create_then_attach` | kept in `create_e2e.rs` | not merged: see Kept against the sweep's verdict |
| `create_e2e` | `server_that_exits_refuses_the_job_in_docker` | kept | the only check of the real `start_probe` on a daemon (exited with stderr, probe container destroyed by id); `guard_server_check` runs a scripted double and `container_stop_e2e` never calls `start_probe` |

Files removed: `guard_layering_i22.rs` and `guard_unstarted_destroy.rs`; their `mod` lines in `tests/it/main.rs` go with them. `guard_egress_proxy.rs` stays with three rows. The i8 canary, gateway b17 and b37, the egress allowlist e2e and `evaluator_only_input_never_reaches_the_agent` are untouched.

## Kept after review

The verifier proposed these repeats. Each observes something its proposed cover does not, so each stays:

| File | Test | Why kept |
|:--|:--|:--|
| `guard_image` | `t4_equal_locks_one_build` | Its bite keys the image by treatment level; `b05_one_build_per_digest` passes a digest directly and cannot see that. |
| `guard_container_guard` | `spawn_budget_lost_sweep_allow_networks` | Its bite drops `network ls` from `LabelSweeper::collect`; the two `b10` tests drive the guard only and never the sweeper. |
| `guard_declared_inputs` | `declared_input_empty_match_is_no_leak_and_a_real_copy_is` | The only direct check that an empty match is no leak; `declared_input_copied_into_the_output_is_an_input_leak` has no empty file and `passes_when_only_empty_files_match` checks the ground-truth refusal, a different verdict. |
| `guard_declared_inputs` | `declared_input_with_an_empty_file_runs_clean` | Checks through a run that empty files are declarable and the run is not sealed as a leak; no other test declares one. |

## Kept against the sweep's verdict

| File | Test | Why kept |
|:--|:--|:--|
| `control_e2e` | `control_each_kind_runs_on_the_real_base` | The sweep names no covering guard. It is the only test that runs the catalog capture (`catalog_spec`) and the run-file collect (`collect_spec`) on the real base image, and the only guard of the M4 failure (a probe container with no `HOME` of its own on a read-only root). `controls_e2e_pinned_harness_passes`, `version_probe_e2e` and `env_image_e2e` run other control kinds. `guard_container_run` renders those argvs against a fake, not a daemon. |
| `egress_proxy_e2e` | `egress_proxy_never_resolves_a_listed_host_to_itself` | Its oracle is docker's resolver asked from inside the proxy when nothing answers to the listed name. The allowlist e2e observes bytes at targets that exist and a loopback or link-local filter, not that case. Its unit twin (`plan_proxy_has_no_listed_alias_and_agents_get_add_host`) is the structure row that goes. |

| `guard_unstarted_destroy` | `unstarted_destroy_session_start` | The cited cover failed its bite. With the session-start arm of `drive` no longer destroying, only `guard_protocol_client::every_failure_path_of_an_evaluator_is_accounted_for` went red, and that test also fails without any bite when the machine is loaded (it passes alone and on retry), so it is no cover. The test is kept, with its `NoStdin` helper, inside the merged module. |
| `create_e2e` | `agent_create_then_attach` | The leaf asks for it to merge into `container_run_e2e`. Its `mod` line lives in `tests/it/guard_container_e2e.rs`, which the leaf's OWNED regex excludes (`e2` carries a digit), so the file stays at `guard_container_e2e/create_e2e.rs` and only its sibling test goes. Moving it is one `mod` line for the next piece that owns `guard_container_e2e.rs`. |

## Bites

Each bite is one edit in product code, run against the guard, and reverted with `git checkout` (the tree is clean of product edits).

| Guard (moved or merged) | Bite | Result |
|:--|:--|:--|
| `guard_egress_setting::plan_render_one_host_per_port` | `render_egress` writes `ssl_preread off;` | red (the other render row stays green, as expected) |
| `guard_egress_setting::plan_render_shared_port_needs_sni` | `render_egress` writes `hostnames_off;` | red |
| `guard_container_record::unstarted_destroy` | `DockerIsolation::run_request` drops `destroy_cid` in its failure arm | red |
| cover candidate for `unstarted_destroy_session_start` | the session-start arm of `drive` (`runner/exec/mod.rs`) drops `destroy_unstarted` | only a load-flaky test went red; no guard covers the arm, so the test is kept |

## Follow-ups

- The sweep's 61-kept target for this family is not reached by the verdict list this leaf was given. The tests below are unjudged and are candidates for a later pass: `guard_container_run::control_kind_flags_and_limits` (a renderer), the `guard_container_guard` spawn-budget rows, `guard_image` argv and key rows, and `guard_declared_inputs` rows that overlap `evaluator_only_input_never_reaches_the_agent`.
- `tests/it/guard_container_e2e.rs` still says in its header that `gateway_e2e` and `supervision_gates_e2e` hold ignored placeholders that panic; both now hold real guards. The header is outside the sweep's scope here and was not rewritten.
- `guard_protocol_client::every_failure_path_of_an_evaluator_is_accounted_for` (a `late.sh` signal count) and `guard_wiring::wiring_stop_execs_in_the_container_with_the_observed_id` (a 60 s timeout) each failed once in a full `-p candler-core` run while the machine's load average was 25 to 40. Both pass alone and on retry. They are timing guards and are untouched here.

## Gates

fmt, clippy, `RUSTDOCFLAGS="-D warnings"` doc, DIGEST (18 tests), GOLDEN (10 tests) pass. `cargo nextest run -p candler-core` passes 403 of 403 (23 ignored skipped) on the last run, with a retry allowed for the load-flaky tests above. The e2e subset `test(/guard_container_e2e/)` with `--run-ignored only` passed 18 of 18 on the real daemon, then both `create_e2e` tests (`agent_create_then_attach`, `server_that_exits_refuses_the_job_in_docker`) passed once more on the daemon after the fix round, 2 of 2. Docker before and after the run differ only in the id of docker's own `bridge` network; the run left no container, image, volume or network of its own, so nothing was removed.

## Verifier

FAIL, then fixed. `create_e2e::server_that_exits_refuses_the_job_in_docker` and the egress per-allowlist plan rows had no cover that observes them on a daemon. They were restored in 08e11922; the other proposed repeats are kept with reasons.
