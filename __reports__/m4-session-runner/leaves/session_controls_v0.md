# session_controls leaf report (v0)

Type: findings. Branch `m4/session_controls`, base 4ce963f.

## Commits
| sha | subject |
|---|---|
| ef011e5 | feat(runner): refuse session mode on a harness whose controls fail |
| bc08345 | test(runner): run the session controls on the pinned harness |
| (last) | reports(runner): record the session_controls leaf |

Net `git diff --shortstat 4ce963f..HEAD` (before this report, after fix round 1): 17 files, 1,949 insertions, 472 deletions. The deletion is S4's 425-line `session_e2e.rs`; `controls.rs` is about 930 lines after rustfmt, most of it the production control world (fake model, gateway, isolation, run, teardown).

## Design as built (coordinator rulings)
- **Seat A.** `jobs/orchestrate/mod.rs::gate_controls` runs once before `JobSession::open`, when any cell of the job is a session cell. `resolve` is untouched and `M2bServices` has no new field. `controls.rs` lives in `jobs/orchestrate/`; nothing in resolve uses it. Injection: `JobHost::controls()` returns the runner or the reason the host has none (a session job on a host with none is refused with that reason, `control = setup`). `DockerHost::with_controls` carries the build contexts; `supervise.rs` builds them from `asset_paths` (`AssetPaths` gained `fakemodel`).
- **Key** = (base image content digits from `candler-base:src-<digest>`, digest of the controls). The control digest covers the fake model's files and the controls' parameters (script names, `idle_s`, turns, least yield, close tolerance, wall) and the version of candler-core, which holds the verdict and the close rule. There is no version constant. The accessors `base_tag` and `context_digest` are the only change in `isolation/image.rs`; `ensure_base` now calls `base_tag`.
- **Record** `<store>/controls/<base>-<first 16 hex>.json`, written once (temp file plus `hard_link`, never replaced); a failing verdict writes nothing. A run records `session.control = {path, digest}`, carried by `CellRun::with_control` (a job-time value, not a resolved setting, because the gate runs after resolution; the cell digest is untouched).
- **Verdict band** `close - yielded` in `[idle_s, idle_s + 1 s]` (`run.duration_ms - session.yielded_at_ms`), plus the recorded `session.idle_s` equal to the control's. The real harness met it on the first e2e run. The controls prove the close rule at the control's `idle_s` only (stated in the module doc).
- **Fake model.** Image `candler-fakemodel:src-<12 digits of its files' digest>`, built through the docker driver when absent. It runs as a `candler.role=control` container with `--rm`, `timeout 900` and a tmpfs workdir, and is ended through a done file written by `docker exec` (`fakemodel.py` gained 12 lines for that). The CA is read by `docker exec cat`. No destroy verb and no host directory are used.
- **Refusal** `jobs.session_controls_failed`, Side::Environment, `OrchestrateError::Controls {harness, control, reason}`; row in `jobs/refusal.rs`, entry appended to `codes.toml`. Contract pages not regenerated.

## Gates
| gate | result |
|---|---|
| leaf `controls_` | PASS, 4 tests (`Summary 4 tests run: 4 passed`) |
| `controls_never_model_` | PASS (in the 4) |
| `controls_recorded_` | PASS (in the 4) |
| `controls_e2e_` (Docker) | PASS, 1 test, about 34 s |
| SPAWN | PASS, 9/9 |
| STD `-p candler-core -p candler` | fmt, clippy `-D warnings`, doc PASS; nextest 411/413. FAIL: `candler::contract_pages d1_regenerating_the_pages_changes_nothing` and `d3_every_author_code_has_a_page`, both expected: the new code has no regenerated page yet (coordinator regenerates at merge) |
| LINUX | PASS |
| SUBJ | PASS (`No errored commits`, no subject over 72) |
| DIGEST | PASS (numstat, `guard_cell_digest` 9/9, core digest binaries 45/45; run before the last controls-only edits) |
| GOLDEN | PASS (`guard_golden` 2/2, `guard_oracle_ac` 1/1; same note) |
| OWNED | see findings 3 |
| Docker | `docker ps -aq` and `docker images -q` identical before and after |

## Guards
Tests removed: `session_e2e_runs::session_e2e_c2_closes_at_idle`, `session_e2e_runs::session_e2e_c1_waits_then_closes` (module `session_e2e_runs` is gone). Added: `controls_refuse_a_broken_harness_only`, `controls_record_is_reused_until_the_control_changes`, `controls_never_model_reaches_the_verdict`, `controls_recorded_on_the_session_run`, and the e2e `controls_e2e_runs::controls_e2e_pinned_harness_passes` (module named so that `^controls_e2e_` matches). The CI expected list needs the swap.

| guard | Guards line | oracle | bites seen red |
|---|---|---|---|
| `controls_refuse_a_broken_harness_only` (table: working, late close, early close, never stops, ignores EOF, wrong idle, C1 notifications never delivered, headless, no runner) | a broken harness is refused for session mode, naming harness version and control, nothing written; headless untouched | scripts c1/c2 and the close rule by hand | upper band removed: "c2 closes 1.5 s late: got Ok(()), expected a refusal"; lower band removed: "c2 closes before idle_s: got Ok(())"; `close_forced` check removed: "c2 ignores end of input: got Ok(())"; refusal naming the model: "names the harness version left: \"m\" right: claude-code 2.1.0"; headless gated: "headless: nothing identified left: 1 right: 0" |
| `controls_record_is_reused_until_the_control_changes` | reuse without re-run, changed digest re-runs, session cells carry the record and headless cells none | record name from the digests by hand; sha256 of the file | re-run always: "a record that is there is reused without a run left: 4 right: 2"; key without the control digest: "one record, named by its key left: [\"0123456789ab.json\"]" |
| `controls_never_model_reaches_the_verdict` | no model outcome or tool list reaches the verdict | the runner's logged arguments | refusing on a model id: panic at the model row |
| `controls_recorded_on_the_session_run` | a session run records `session.control` | the reference given to the cell | recording dropped: "the run names its record left: None" |
| `controls_e2e_pinned_harness_passes` | real controls pass; record names both; no container left | scripts, docker ps before/after | `-i` replaced in the container command: test fails in 5 s |

The Grit reviewer's findings (TRAIT_OBJECT and BORROW annotations, imports, a roadmap reference in test docs, INDEX) were fixed.

## Findings
1. **Spec defect (ruled):** a resolve step cannot see per-cell session mode; the gate moved to job start.
2. **Behaviour change, flagged:** a job with a session cell is now refused, with `jobs.session_controls_failed`, when no control runner exists, when the base image or sidecar image is missing, or when a control fails. The sidecar `candler-gateway:latest` must be built (`candler run` does it before the job); the controls read it by that fixed tag.
3. **Outside OWNED (needed, flagged):** `assets/docker/fakemodel/fakemodel.py` (done-file exit), `isolation/image.rs` (two accessors, authorised), `jobs/refusal.rs` and `docs/contract/codes.toml` (authorised), `jobs/orchestrate/session.rs` (`open` split into `open_service`, `DockerHost` fields, `Controls` variant), `runner/exec/cell.rs` and `runner/exec/mod.rs` (`CellRun::control`, one measured entry), `crates/candler/src/commands/{preflight,supervise}.rs` (wiring), `tests/support/session_rig.rs` (`run_controlled`, `execute_controlled`), `tests/guard_container_e2e.rs`. Also the fake model's image and the base image id are re-read on every job; the controls build one world per control (about 10 s of setup each).
4. **Name differences:** the leaf names `SessionControlsStep`, `CONTROL_VERSION` and `resolve/steps/controls.rs`; none exists (rulings 1 and 3). `ControlRecord` is the private `Record`.
5. **Not covered:** a close-rule defect that only appears at an idle longer than the control's, and a change to the verdict's code that leaves its parameters alone (the digest covers parameters, not code).
6. **Test-first:** the production code was written before the guards; each guard's red was seen by injecting its bite afterwards, not by running it before the code.

## Fix round 1
1. **Rebuilt base.** The check of the record's `base_id` against the identity is gone; `base_id` stays in the record as documentation. `controls_record_is_reused_until_the_control_changes` now has the fake report another `base_id` on a later job and asserts the record is reused. Bite: put the check back, and the step fails at "the rebuilt base passes".
2. **Stale pass across releases.** `control_digest` includes `env!("CARGO_PKG_VERSION")` of candler-core; the module doc says what the key covers. No test (ruling).
3. **Cause kept.** `JobHost::controls` returns `Result<_, String>`; `supervise.rs` builds `control_contexts` and passes its error to `DockerHost::without_controls`, so the refusal names the real cause.
4. **Cancellation.** `gate_controls` takes the job's `JobControl`; `ensure` and `ControlRunner::run` pass `ctl.cancel.subscribe()` to the control's run loop. The private watch channel is gone.
5. **Reuse.** `sidecar_gateway` (in `session.rs`) is shared by `DockerHost::gateway` and the control world; the control world takes its docker client from `host.docker_cli` and builds its runtime before starting the fake; `wait_for` returns `Option<T>` and serves the authority read, the network join and the fake's end. The net size did not fall: the diff grew by about 40 lines (the host's reason and the cancel parameter outweigh the extraction), `controls.rs` 937 to 929.

Checks after the round: `controls_` 4/4, `controls_e2e_` 1/1 with Docker (containers and images identical before and after), clippy and fmt `-p candler-core -p candler`, DIGEST (numstat, `guard_cell_digest` 9/9, core digest binaries 45/45), and `guard_platform_seam`, `guard_layering_i22` and `guard_job_orchestrate`.
