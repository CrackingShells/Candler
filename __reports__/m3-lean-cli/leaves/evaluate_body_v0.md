# evaluate_body, leaf report v0

An evaluation as a supervised job body of its own: the evaluator slot pool and plug-in discovery, an engine that takes slots, selects runs and reports each committed outcome, and `supervise_evaluate`. A Sonnet implementer did the work on branch `m3/evaluate_body`, cut from `feat/m3-lean-cli` at 8defdec. Every change is in `candler-core`; `JobSpec`, `JobServices`, the wire and `commands/**` are untouched.

```
feat(jobs): supervise an evaluation as a job
feat(evaluation): take evaluator slots and report committed outcomes
feat(evaluation): add the evaluator slot pool and plug-in discovery
reports(evaluation): record where an evaluator's strays appear
```

## P4

Question: are an evaluator's descendants reparented to the evaluate job's supervisor, and when do strays appear?

Rig: the Linux image of LINUXRUN, a throwaway test (deleted, not committed) that made itself the subreaper (`become_subreaper`), the `engine_fixture` manifest template with a kind that starts `sh -c 'trap "" TERM; sleep 120'` through `Popen(start_new_session=True)` and returns `pass`, the `guard_engine` environment, and `evaluate` over two runs. A thread listed every process whose parent is the test, with state and command line, every 10 ms (finer than the 50 ms the leaf asks for). Three variants: A, two requests at once, no stray sweeper; B, two requests at once, `ProcMarkerSweeper` installed on the supervisor; C, one request at a time, no sweeper. Each ended with one `sweep_orphans`.

| fact | A (concurrency 2, no sweeper) | B (sweeper) | C (concurrency 1, no sweeper) |
|---|---|---|---|
| stray's parent when first seen | the test (subreaper) | the test | the test |
| delay from the evaluator's exit to that | within one 10 ms poll | within one poll | within one poll |
| a stray with the test as parent while its evaluator is alive | never | never | never |
| stray alive until | the final `sweep_orphans` (~500 ms after the engine returned) | killed 0 to 10 ms after the evaluator's exit, by the request's own ladder sweep; the final sweep found nothing | the final `sweep_orphans`; request 1's stray lived through all of request 2 |
| the evaluator, once exited, a zombie child | no `Z` seen for any `uv run` or `python` process | none | none |
| `sweep_orphans` killed | 2 | 0 | 2 |

Each evaluator is two processes, `uv run` (child of the test) and its `python` (child of `uv run`, which does not exec); the kind's stray is a child of `python`. A stray therefore has the test as its parent from the instant `python` exits.

**Answer: strays are reparented to the supervisor at the end of each request**, when their evaluator exits, never earlier. They outlive their request: the stop ladder signals the evaluator's group, and the stray left it with `setsid`. Without a marker sweeper the stray stays until the supervisor sweeps (C: request 1's stray ran through request 2). With `ProcMarkerSweeper` on the supervisor, the sweep that the ladder runs after every request kills it by its marker in about 10 ms, so the strays never reach the end, provided the marker is the request's own. The evaluator processes themselves are waited for by the supervisor's runtime (no zombie leader).

Rule that follows: the body sweeps once, after the engine returns, as the net (needed by E3, and for a stray that the ladder's marker sweep did not find). No sweep after each run plan: with the per-request marker the ladder's sweep already ends a stray per request, and a stray that escaped it was idle in this probe (state `S`, no CPU, no slot-sized resource) while later requests ran. The probe's stray sleeps; a CPU-bound stray was not tried, because the per-request sweep (B) ends any stray that carries the marker.

## Guards and bites

Guards 1 to 4 of Step 2 were seen red as compile errors (the items did not exist). The guards of Steps 3 and 4 were written with the code and are seen red through their bites: each mutation below was applied, the guard went red, and the file was restored. Linux rows ran through LINUXRUN, one container at a time. The brief's `--no-tests=fail` is on every run.

| guard | file | bite | seen |
|---|---|---|---|
| `e2_user_config_reads_slots_evaluators_and_refuses_a_bad_value` | guard_resolve | the project file stops refusing the `slots` table | red |
| `e2_slots_evaluators_is_a_setting_not_a_cell_value` | guard_resolve | the path dropped from `is_setting` | red |
| `e2_default_evaluator_slots_follow_the_cpu_count` | guard_engine | the CPU count used whole | red |
| `e1_plugin_dirs_find_one_manifest_each` | guard_registry | the first of two manifests taken silently; a scan of the subdirectory | red, red |
| `e2_one_evaluator_slot_serialises_requests_and_two_overlap` | guard_engine | no slot taken (equal to a semaphore that never blocks): the one-slot run overlaps | red |
| `e2_agent_slots_held_by_a_job_do_not_delay_an_evaluator` | guard_engine | the evaluators drawing from `<state>/candler/slots`: 61 s, the timeout | red |
| `e1_engine_emits_one_outcome_event_per_committed_record` | guard_engine | records committed, no event; the event sent before the commit (the sink reads the store at each event) | red, red |
| `e1_selection_keys_and_pending_pick_the_runs_they_name` | guard_engine | `Pending` selecting every run (6 for 5); a file of another version counted (4 for 5); `Keys` selecting every run | red x3 |
| `e3_each_request_carries_its_own_marker` | guard_engine | the constant marker back (the two requests share it) | red |
| `e1_a_run_spec_keeps_its_bytes_in_the_spec_file` | guard_evaluate_job | the enum tagged: the bytes of a run spec move | red |
| `e1_the_evaluate_job_ends_with_one_outcome_event_per_committed_record` | guard_evaluate_job | no `evaluation.planned`; planned emitted after the outcomes; outcomes with no event | red x3 |
| `e1_the_evaluate_job_refuses_a_changed_suite_and_zero_slots` | guard_evaluate_job | the digest unchecked; the zero read as the default pool | red, red |
| `e3_a_setsid_term_ignoring_stray_of_an_evaluator_is_dead_by_job_ended` (Linux) | guard_evaluate_job | see the table below | see below |
| `e3_a_lost_evaluate_job_is_recovered_and_its_evaluator_is_reaped` (Linux) | guard_evaluate_job | the engine's records outside `children/` | red |

The stray guard has three overlapping defences: the marker sweep that each request's stop ladder runs, the orphan sweep after the engine returns, and the subreaper role that lets the orphan sweep see the stray. The spec's bite ("no subreaper or no sweep") does not bite one of them alone, and that is the design:

| mutation | result |
|---|---|
| no marker sweeper and no orphan sweep | red |
| no marker sweeper, no orphan sweep, no subreaper | red |
| no marker sweeper and no subreaper (orphan sweep kept) | red |
| only the orphan sweep removed | green |
| only the marker sweeper removed | green |
| only the subreaper removed | green |

## Gates

| gate | command | result |
|---|---|---|
| OWNED | the leaf's `grep -vE` over `git diff --name-only 8defdec..HEAD` | PASS |
| shared files only grow, schemas untouched | `git diff --numstat` on `jobs/model.rs` and `jobs/runtime/mod.rs`, `git diff --quiet` on `candler-schemas` | PASS (34 and 1 lines added, 0 deleted) |
| E1 | `nextest run -p candler-core --no-tests=fail -E 'test(/^e1_/)'` | PASS, 6 tests |
| E2 | `-E 'test(/^e2_/)'` | PASS, 5 tests |
| E3 | native: marker row only, 1 test. LINUXRUN: `--test guard_evaluate_job -E 'test(/^e3_/)'` (2 tests) and `--test guard_engine` (1 test), before and after SPAWN | PASS |
| public surface | `-E 'binary(public_surface)'` | PASS, 14 tests |
| digest guards | `-E 'binary(guard_engine) \| binary(guard_registry) \| binary(guard_run_context) \| binary(guard_resolve)'` | PASS, 31 tests |
| STD, `-p candler-core` | `fmt --check`, `clippy --all-targets -D warnings`, `nextest run` (220 tests, none failed), `doc --no-deps` with `-D warnings` | PASS |
| LINUX, `-p candler-core` | `clippy --target x86_64-unknown-linux-gnu --tests -D warnings` | PASS |
| SPAWN | `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && nextest run -E 'binary(guard_signal)'` | PASS, 9 tests |
| SUBJ | `cog check 8defdec..HEAD`, subjects at most 72 characters (longest 68) | PASS |
| Q1 | `binary(guard_platform_seam)` | PASS, 3 tests |

No `confirm_*` test was written. The workspace form of STD is not run: see the first spec defect.

## Deviations and decisions
- **`evaluation.planned` is sent by the engine**, through the sink given to `evaluate_reporting`, once the runs are selected and planned. Its count is the records of every entry of every plan, `not_applicable` ones included. The plain `evaluate` drops it.
- **The marker** of a request is `evaluation-<id>` and of a `describe` is `evaluation-describe-<id>`. `describe` takes no slot: the spec has only the batches take one, and describes run one at a time before any request.
- **`Keys`** skips a key the store does not hold or whose run is not sealed, as a glob that matches nothing does.
- **`RunSelection`** is `#[non_exhaustive]`, which clippy's `exhaustive_enums` asks for. A `JobSpecFile` match stays exhaustive.
- **`JobSpecFile`** carries one more attribute, `#[expect(clippy::large_enum_variant)]`, because `JobSpec` is 672 bytes and `EvaluateSpec` 144. The doc paragraph on version-locked specs was appended to the existing doc, since the shared file may only grow. The scaffold's encoding of `Run` is the bare object, as the spec hoped.
- **An engine error ends the job `failed`** with the reason on the supervisor's stderr (`supervisor.log` when detached) and no `report.json`. `RuntimeError` has no variant for it, and `mod.rs` may only gain `pub mod evaluate;`.
- **A job told to stop** ends `killed` whatever the engine returned; the report of that call is the partial one.
- **The recovery of a lost evaluate job** reaps the evaluator from `children/` and not the strays (no marker sweeper in the recovery, as for a run job).
- **The duplication** of `session::run` in `jobs/runtime/evaluate.rs` is deliberate (`session.rs` is M4's). E1 and E3 pin the order: handlers, session, subreaper, lock, `job.started`, state, body, orphan sweep, `job.ended`, state, lock last. A body that panics ends with no `job.ended`.
- **No platform inventory row** is needed: `evaluate.rs` and `pending.rs` call no unix-only API themselves (Q1 passes), so the one permitted edit outside OWNED was not made.

## New error enums and variants
No new enum. New variants of existing `#[non_exhaustive]` enums, which `refusal_late` codes:
- `RegistryError::NoManifest { dir }` and `RegistryError::ManyManifests { dir }`;
- `EvaluateError::Slots(SlotError)`.

New public types without errors: `EvaluatorSlots`, `RunSelection`, `EvaluateSpec`, `EvaluateEntry`, `EvaluateJobOutcome`, `CancelBridge`.

## Behaviour changes, reported and not marked
1. **`slots.evaluators`**, a new user-config key (refused in a project file), a setting, default `max(1, cpus / 2)`. Evaluator processes of an evaluate job draw from `<state>/candler/evaluator-slots`; `regrade` has a pool of two under `<env_root>/slots-regrade`. The commit type is `feat(evaluation)`.
2. **The marker** of an evaluator process changes from the constant `candler-evaluation` to `evaluation-<id>`; the child records of an evaluation name it.
3. **`EvaluateRequest`** loses `concurrency` and `runs` and gains `slots`, `selection` and `record_dir`. This is a library break with no external user. `candler evaluate --concurrency` has nothing to map to until `evaluate_verb` decides what it means (a pool of that size, per call, or the machine-wide pool).
4. **Plug-in manifest file names** under `plugins.dirs`: `plugin.toml` (the fixtures') and `candler-plugin.toml` (the golden plug-in's), one per directory, a choice for the coordinator since R01 names none.
5. **`candler evaluate` document** now comes from `EvaluateReport::to_document`; its four keys are unchanged.

## Fix round: rebase and bridge
- **Rebase** onto `feat/m3-lean-cli` at fe1a942. One conflict, in `experiment/paths.rs`: both appends kept (`SLOTS_EVALUATORS`, then `refusal_core`'s `is_known_path`).
- **`slots.evaluators` and `is_known_path`:** it is known through `is_setting`, the first test of `is_known_path`, so unknown-path refusal in `--set`, `[fixed]` and factors never rejects it. Established by a test, `e2_slots_evaluators_is_a_known_path` (bite: the path dropped from `is_setting`, seen red), and it needs no row in the `LEAVES` array.
- **Bridge** (one edit outside OWNED, authorised by the coordinator): `crates/candler/src/commands/evaluate.rs` maps the runs glob to `RunSelection::Glob` (`All` when absent) and `--concurrency N` to a pool of N slots in `<env_root>/slots-evaluate`, job `evaluate-<pid>`. Output and exit codes are unchanged; the `guard_cli*` binaries pass and no line of them changed. `evaluate_verb` replaces it. The commit follows the report commit rather than `a0cd268`, because rewinding the branch was not allowed.

## Spec defects found
- **The workspace does not build between this leaf and `evaluate_verb`.** `crates/candler/src/commands/evaluate.rs` builds an `EvaluateRequest` with `runs` and `concurrency`, and `commands/**` is not OWNED. `cargo check -p candler` fails on those two fields; `-p candler-core` is clean. The coordinator needs `evaluate_verb` on top of this branch, or a two-line edit to that file, before the workspace form of STD can run.
- **The E3 bite is not single-mechanism** (table above): the three defences overlap, so the bite is "no marker sweep, no orphan sweep" and "no marker sweep and no subreaper".
- **E1's "an event emitted for a file that was cancelled" cannot be reached by a test.** `commit` reads the cancel at two points a few microseconds apart in one blocking task, and a request cancelled in flight faults before `publish`. The guard instead cancels a request in flight (a failed request with no event and no file) and a call before it starts, and it checks, at each event, that the record is already in the store.
- **The `^e3_` filter** matches the full test name, so a test inside a module (`linux::e3_…`) never matches. The Linux guards are top-level functions with `#[cfg(target_os = "linux")]`.
- `JobSpecFile` is untagged, so a malformed evaluate spec is reported as "did not match any variant", with no pointer to the field. `ps` shows `-` for a spec it cannot read, as for a run spec.

## Disk and time
Build directory `/private/tmp/claude-501/m3-target-evaluate_body`: 2.3 GiB at the end, removed. The Linux builds went to the shared volume `m3-linux-target` (path `/w`); the coordinator removes it at close-out. No container is left; the strays of P4 and E3 died with their containers or by the sweep.

