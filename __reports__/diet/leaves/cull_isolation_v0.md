# cull_isolation: leaf report (v0)

Leaf: `__roadmap__/diet/cull_isolation.md`. Branch `diet/cull_isolation`, cut from `feat/diet` at `0fcd6562`.

## Before count

Owned files, counted with `cargo nextest list -p candler-core` (default run plus `--run-ignored only`):

| Run | Family tests | `-p candler-core` tests |
|:--|--:|--:|
| Before | 90 (71 default, 19 ignored e2e) | 429 (406 default, 23 ignored) |
| After | 83 (65 default, 18 ignored e2e) | 422 (400 default, 22 ignored) |

The sweep's "100" counted the family loosely; the owned-file count above is the one measured. Seven tests go; the rest of the sweep's 39 culls fall in files this leaf's verdict list does not name (see Follow-ups).

## Deleted and merged tests

| File | Test | Verdict | Covering guard |
|:--|:--|:--|:--|
| `guard_layering_i22` | `b16_destroy_verbs_only_in_guard` | deleted | structure (it scans the source for destroy verbs); destroy discipline is held by `guard_container_guard::b10_guard_refusals` and `container_stop_e2e::b13_client_killed_container_gone` |
| `guard_egress_proxy` | `plan_agent_networks` | deleted | the agent's networks are observed by `egress_allowlist_holds_on_a_real_daemon` (bite `agent_on_egress` drives `agent_networks`); the rows are plan structure |
| `guard_egress_proxy` | `plan_allow_network_names_follow_the_allowlist` | deleted | structure (a name grammar); `egress_allowlist_holds_on_a_real_daemon` observes that two allowlists do not share a network |
| `guard_egress_proxy` | `plan_one_proxy_per_distinct_allowlist` | deleted | structure; `egress_allowlist_holds_on_a_real_daemon` runs one proxy per allowlist on the daemon |
| `guard_egress_proxy` | `plan_proxy_has_no_listed_alias_and_agents_get_add_host` | deleted | structure (argv shape); the resolution promise it protects is observed by `egress_proxy_never_resolves_a_listed_host_to_itself` and the allowlist e2e |
| `guard_egress_proxy` | `plan_session_starts_and_ends_one_proxy_per_allowlist` | deleted | structure (handle and start/end calls of a fake gateway); `egress_allowlist_holds_on_a_real_daemon` leaves no rig label behind |
| `guard_egress_proxy` | `plan_render_one_host_per_port` | moved to `guard_egress_setting` | the config text is what nginx runs; kept, name unchanged |
| `guard_egress_proxy` | `plan_render_shared_port_needs_sni` | moved to `guard_egress_setting` | kept, name unchanged |
| `guard_unstarted_destroy` | `unstarted_destroy` | moved to `guard_container_record::unstarted_destroy` | kept, name unchanged |
| `guard_unstarted_destroy` | `unstarted_destroy_session_start` | kept, moved with its helper to `guard_container_record::unstarted_destroy` | no covering guard (see Bites and Kept against the sweep's verdict) |
| `create_e2e` | `agent_create_then_attach` | kept in `create_e2e.rs` | not merged: see Kept against the sweep's verdict |
| `create_e2e` | `server_that_exits_refuses_the_job_in_docker` | deleted | `guard_server_check::server_that_exits_refuses_the_job_with_its_stderr` (refusal naming the server's stderr) and `container_stop_e2e::destroy_verifies_by_one_inspect` (the probe's container is gone by id) |

Files removed: `guard_layering_i22.rs`, `guard_egress_proxy.rs`, `guard_unstarted_destroy.rs`; their `mod` lines in `tests/it/main.rs` go with them. The i8 canary, gateway b17 and b37, the egress allowlist e2e and `evaluator_only_input_never_reaches_the_agent` are untouched.

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

fmt, clippy, `RUSTDOCFLAGS="-D warnings"` doc, DIGEST (18 tests), GOLDEN (10 tests) pass. `cargo nextest run -p candler-core` passes 400 of 400 with one retry (the flaky test above). The e2e subset `test(/guard_container_e2e/)` with `--run-ignored only` passed 18 of 18 on the real daemon, then `create_e2e` once more after its last edit. Docker before and after the run differ only in the id of docker's own `bridge` network; the run left no container, image, volume or network of its own, so nothing was removed.

## Verifier

pending (coordinator)
