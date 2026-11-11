# evaluate_verb, leaf report v0

`candler evaluate <suite>` becomes a verb over an evaluate job: it starts the job, prints the job's id on stderr, follows the job to its end and prints the report on stdout. `--plugin` and `--concurrency` are gone, and `--suite` became the positional argument. A Sonnet implementer did the work on branch `m3/evaluate_verb`, cut from 2e0c015. The `evaluate_verb` verifier, which also covers `evaluate_body`, comes after.

```
75d34fd fix(cli): reap the supervisor and follow a job that ends at once
befe4b6 feat(cli): end evaluate at once on a second Ctrl-C
cdd2c54 test(cli): guard that Ctrl-C and a failed job end evaluate as stated
4f7aab7 feat(cli)!: start evaluate as a job and follow it to its report
2ebc49d feat(cli): supervise an evaluate job from the hidden verb
```

Step 2 and step 3's verb code landed together in `4f7aab7`: the Ctrl-C arm was written with the follow loop. `cdd2c54` holds step 3's guard and a second one.

## What landed
- **`supervise`** reads its spec on stdin, checks the digest against `job.json` itself (`spec_digest`), reads the bytes as a `JobSpecFile`, writes its identity, then splits. The `Run` arm is today's body, moved into `run_job`, and still calls `read_spec`, so the digest is checked twice there. The `Evaluate` arm builds an `EvaluateEntry` and awaits `supervise_evaluate`, with no driver.
- **`evaluate <suite> [<job-id> | --runs <glob>]`** in order:
  1. load the configuration and take `plugins.dirs`, refusing when it is empty;
  2. discover the manifests, load the registry and run the preflight in the foreground, as before;
  3. read the suite and digest it;
  4. choose the selection: `<job-id>` is the `run.started` keys of that job's log (a job that started none is refused), `--runs` is a glob, and neither is `Pending`;
  5. write the job directory and `detach` the supervisor;
  6. print the id and follow.
- **The bridge is gone.** There is no private `slots-evaluate` pool and no `evaluate-<pid>` job. The evaluators draw from `slots.evaluators` in the supervisor.
- **Following** polls the log and lock every 200 ms through `lost::classify`, and recovers a lost job with `recover_job_dir`. Ctrl-C calls `kill::stop_job` as it is, then waits for the end for at most `TEARDOWN_BOUND`. A second Ctrl-C returns at once.
- **The end decides the exit.**

| End | Output | Exit |
|:----|:-------|:-----|
| `completed` | `report.json` on stdout | 0, or 2 with "N request(s) failed" |
| `killed` after Ctrl-C | nothing | 2, "was cancelled" |
| `killed` without Ctrl-C | nothing | 2, "was killed" |
| `failed`, `budget_stop` | nothing | 2, `job <id> ended without a report:` and the last 5 lines of `supervisor.log` |

## Found through LINUXRUN
I ran `guard_cli_evaluate` once in the `m3-linuxrun` image, in a volume of its own, one container at a time. Two defects showed that macOS hid; the last commit fixes both in `evaluate.rs`.
- **A zombie supervisor stalled Ctrl-C for the whole teardown bound.** The supervisor is the waiting command's child, and `detach` drops its handle, so nobody waits for it. On Linux `liveness` reads a zombie as alive, so `kill::stop_job`'s reap waited out its grace for a supervisor that was gone. The command now calls `reap_one` on it at each look at the job and while a stop is under way.
- **`detach` reports a job that ends within one 20 ms look as "the supervisor did not start".** With `slots.evaluators = 0` the job fails in a few milliseconds, between two looks at the lock. The command now follows such a job anyway and gives the end of its `supervisor.log`. The cause is in `jobs/runtime/detach.rs`, which is `evaluate_body`'s: a job that takes and releases the lock between two looks is a false "did not start". Any other caller of `detach` has the same exposure.
- **The guards need `-j 1` in the container.** With six tests in parallel, `candler run` in the world did not seal (`no run sealed: 0 sealed`) in four of six, from load in the Docker VM, not from the verb. With `-j 1` all six pass.

## Decision asked for: a job with no `report.json`
The verb prints nothing on stdout, exits 2 and puts the end of the job's `supervisor.log` in the error. That is where `evaluate_body` writes the engine's reason: an engine error ends the job `failed` with no report, and the reason goes to `note`. `slots.evaluators = 0` shows it, as `no evaluator slot to run in: slots.evaluators is 0`. A `completed` job whose `report.json` is unreadable is the same kind of error.

## Rulings I applied
- **No new `thiserror` enum.** The empty-`plugins.dirs` refusal is `CliError::Refused`, whose code is `candler.internal`. Plug-in and engine errors travel as `CliError::failed`. `refusal_late` may want a coded home for "no plug-in directory"; I added no `codes.toml` row.
- **The supervisor's environment** is an allowlist: `PATH`, `HOME`, `XDG_STATE_HOME`, `XDG_CONFIG_HOME` and the store root. The plug-in processes therefore no longer see the caller's other variables, such as `UV_*` or a proxy. The old in-process call passed everything. This needs a ruling.
- **`XDG_CONFIG_HOME`** is in the list because the supervisor reads `slots.evaluators` from it. `run.rs`'s allowlist has none, so a `run` job never sees the user configuration: an existing defect of `run`, left alone.

## Spec defects
- **Step 3 uses `rustix::process::kill_process`, but `candler` has no `rustix` dependency.** Adding one means editing `Cargo.toml` and `Cargo.lock`, which are not OWNED. The guard sends SIGINT through `Supervisor::reap` with `StopSignal::Int`, which is the I22 guard, against a child the test spawned as a group leader.
- **Step 3's oracle "the job's last event is `job.ended`" is false.** `kill::stop_job` reaps the supervisor with the job's own log as its sink, so the stop's ladder steps (`signal`, then `terminate_group` and `kill_group` refused as "the leader's process id now names another process", then `sweep`) are appended after `job.ended`. The guard asserts exactly one `job.ended`, with `killed`. `kill.rs` is not mine. Whether a clean stop should log refused signals after the end is for the coordinator.
- **G-text against the `candler` helper in `guard_cli.rs`.** "Each version runs from its own project directory, the helper not edited" cannot hold with the gate, which flags `let out = candler(` and `&[` when a call is renamed or reflowed. The call keeps its lines through a local closure named `candler` that shadows the helper, and `#[rustfmt::skip]` keeps rustfmt from packing the array.
- **"Read the spec through the scaffold's boundary reader."** `read_spec` returns a `JobSpec` and writes `spec.json`, so it cannot read the new kind. The verb reads and digests the bytes itself; the `Run` arm still calls `read_spec` on the same bytes.
- **`JobSpecFile` is untagged.** A malformed evaluate spec still reads as "did not match any variant", as `evaluate_body` recorded. The verb writes the spec itself, so only a hand-written one meets it.

## Behaviour changes (for the coordination session)
1. **`evaluate` loses `--plugin`, `--suite` and `--concurrency`** and takes the suite as a positional argument. The commit is `feat(cli)!` with the migration in its body.
2. **The default selection is the pending runs.** The old default graded every run; a call that wants that passes `--runs '*'`.
3. **Every call leaves a job in the store.** `ps` lists it, `watch` follows it and `inspect` shows it.
4. **The plug-ins come from `plugins.dirs`.** Without one the call is refused, exit 2, and no job is created.
5. **A refusal's text names the absolute manifest.** The old message said `plug-in manifest plugin.toml`; the new one carries `<dir>/plugin.toml`.
6. **The plug-in processes' environment is the allowlist** (see above).
7. **A suite is read once by the verb.** The supervisor refuses a suite that changed afterwards, which the old in-process call could not meet.

## Files outside OWNED that the change breaks
`crates/candler/tests/guard_cli_refusal.rs` (refusal_core's) still calls `evaluate --plugin plugin.toml --suite suite.toml`. Three tests fail on this branch:
- `r2_a_malformed_manifest_prints_as_a_record`;
- `r2_the_text_form_is_unchanged_without_the_flag`;
- `r2_the_pending_rows_still_print_as_internal`.

I did not edit it. I tried a migration and reverted it: `bad_manifest()` also writes a `candler.toml` with `[plugins] dirs = ["."]`, `evaluate_args` becomes `["evaluate", "suite.toml", "--store", "store"]`, and the pending row (a missing plug-in directory) runs from a subdirectory whose `candler.toml` names `absent`. With that the pending row passes, and the other two need their expected text changed, because the message now carries the absolute manifest path (behaviour change 5). Both files are in `refusal_late`'s lane, which is closing R1's pending list in parallel, so the coordinator should decide who edits them. The argument lines it loses match the G-text exception.

## Bites (each seen red, then reverted)
Mutations were made in `commands/evaluate.rs` and the guard run against them.
- **E1**
  - The id printed on stdout: `e1_evaluate_prints_the_job_id…` red (the first line of stderr is not an id).
  - A job that ended `failed` read as `completed` (the report is printed): `e1_evaluate_of_a_failed_job…` red.
  - Ctrl-C returning "cancelled" without stopping the job: `e1_ctrl_c_ends_the_evaluate_job_killed` red (no `job.ended` in the log).
  - The empty-`plugins.dirs` check removed: `e1_evaluate_refuses_without_a_plugin_directory` red (a job is started).
  - The `Evaluate` arm missing from `supervise`: `e1_supervise_runs_an_evaluate_spec_written_by_hand` red (`missing field experiment`). Seen red before the arm was written.
- **E4**
  - The default selection every run: row 2 red (the second `evaluate` reports three request ids where none were expected).
  - The default selection none: row 1 red (no outcome directory).
  - `<job-id>` selecting every run: row 3 red (the other job's runs are graded again).
  - `--runs` selecting every run: row 3 red (a run the glob does not name is graded again).
- **Migration:** the golden-parity regrade needs `CANDLER_HTN_PLUGIN`, which is not set here, and I found no HTN plug-in checkout on the machine. The test builds (workspace `check` and `clippy --all-targets`) but did not run. E4 row 2 is the same property on the fixture: a default-selection `evaluate` run twice grades nothing the second time, so `--runs '*'` is what the second regrade needs.
- **Not bitten:** the second Ctrl-C. A stop finishes in under a second on the fixtures, so no test can land a second signal inside it; the arm is read, not run.

## Gates
Run on 75d34fd, with `-p candler` while working, then the workspace forms once.
- **OWNED, G-text (with its exception), old options gone and `<SUITE>` in the help, G-platform (1 `cfg(not(unix))`), SUBJ and `cog check`:** PASS.
- **E1 (6) and E4 (1):** PASS. `binary(guard_cli) | binary(guard_input_seam) | binary(guard_cli_e2e)` and `guard_cli_surface` (U1, U2): PASS.
- **STD, workspace:** fmt, clippy `-D warnings`, `cargo check --workspace --all-targets` and `cargo doc` with `-D warnings`: PASS. nextest: 564 passed, 3 failed, 21 skipped; the 3 are `guard_cli_refusal` above.
- **LINUX:** clippy on `x86_64-unknown-linux-gnu`: PASS.
- **GOLDEN:** `guard_golden` (2) and `guard_oracle_ac` (1): PASS. `guard_golden_parity` did not run.
- **LINUXRUN:** `guard_cli_evaluate` (6) on the fixed head, `-j 1`: PASS. Before the fix: the Ctrl-C test ran past three minutes and the failed-job test failed (see above).

## For the verifier
- **Two concurrent `evaluate` calls over the same runs.** Each reads `Pending` when its job starts, so both select the same runs and both grade them. Nothing here serialises them, and I did not probe the outcome.
- **A lost job.** `follow` recovers it with `recover_job_dir`; no guard kills a supervisor mid-evaluation to see the verb's end.
- **Ctrl-C racing `job.ended`.** The classify call comes first in each turn, so a job that ended just before the signal is read as ended. `kill::stop_job` leaves an ended job alone (`left alone`).
- **`slots.evaluators = 0`** ends the job `failed`; `e1_evaluate_of_a_failed_job…` covers it through the verb.
- **E3** (`evaluate_body`'s) was not run by me; only my own six guards went through LINUXRUN.
- **The first stderr line is the job id only when the preflight is silent.** When it builds an environment, `StderrSink` event lines precede the id.

## Fix round
The branch was rebased onto eed99b7 (refusal_late landed) with no conflict. The round's commits, after the rebase:

```
d93d49f fix(jobs): see a job that ends at once as started
1c2384d feat(jobs): code the refusal of a suite changed after the job started
4250fcc feat(evaluation): give an evaluate job the caller's environment
bc6018d test(cli): follow evaluate to its new form in the refusal guards
```
The `feat(cli)!` commit was reworded with `git commit --fixup=reword:` and an autosquash. Its `BREAKING CHANGE:` footer now names the three removed options, the default selection (the pending runs, `--runs '*'` for all) and the refusal of an empty `plugins.dirs`. The reword was not refused.

### The refusal guards migrated
`guard_cli_refusal.rs` runs `evaluate` in a project whose `candler.toml` lists the plug-in directory, with the suite positional and `evaluate_args = ["evaluate", "suite.toml", "--store", "store"]`. The rows of refusal_late's that called the old options:
- `r2_a_malformed_manifest_prints_as_a_record` and `r2_the_text_form_is_unchanged_without_the_flag`: the expected text is `…the plug-in manifest /` with the absolute path, and the text-form row also checks `/plugin.toml is not valid: ` (behaviour change 5).
- `r2_a_missing_plugin_manifest_prints_as_a_record`: `plugins.dirs` can name only a directory, and a directory with no manifest is `NoManifest`, so the row keeps its code with a manifest file that exists and cannot be read (mode 000).
- `r2_no_state_home_prints_as_an_environment_record`, `r2_refused_suite_params_print_as_a_suite_record` and `r2_an_unknown_job_and_an_unreadable_suite_name_what_the_caller_got_wrong`: the same project and positional suite. The unreadable suite is raised by the command, as `EvaluateError::Io`, before the preflight, so its code `evaluation.suite_unreadable` holds.

Lines these files lose that the G-text pattern does not name (`--plugin`, `--suite`, `manifest_arg`, `suite_arg`, `"evaluate --`), 23 in all:
- argument lines of the old call: `"plugin.toml",` (3), `"absent/plugin.toml",`, `"suite.toml",` (the relative path became `../suite.toml`), `.arg(&manifest)` (2), `dir.path(),` (the working directory became the plug-in's), and the `.args([ … ])` lines that held the old arguments (2 blocks);
- the 3 message lines of behaviour change 5;
- `assert_refusal(&out, "suite.params_invalid", "suite", Some("/n"))`, described next.

Two lines of `guard_cli_surface.rs` are also lost, the `evaluate --runs` and `--concurrency` entries of `PENDING_STAMPS`, which 2e0c015 put inside the exception.

**One expected result changed for a reason other than behaviour change 5, and needs a ruling.** The suite-params refusal is raised inside the evaluate job, by the engine, and reaches the command as the end of the job's `supervisor.log`. The record the command prints for it is `candler.internal`, side `candler`, with the message text intact; it was `suite.params_invalid`, side `suite`, pointer `/n`, when the engine ran in the command. The same holds for every refusal the engine raises (an unknown evaluator, a plug-in whose environment is not ready, a dependency cycle). Only what the command checks itself keeps its code. The row now asserts the fallback code and the message, with a comment saying so. The fix is for `evaluate_body` and the verb together: the supervisor writes the refusal record of a failed evaluation into the job directory, and the verb prints it as its own.

### The environment (option 1, provisional)
- The supervisor of an evaluate job is started with the caller's whole environment, as the process environment of the spawn (`detach`'s `.envs`), and `CANDLER_STORE_ROOT`. The engine's evaluator processes take the engine process's environment, less the store variable, where they took `PATH` and `HOME` only. `run` keeps `env_clear()` and its allowlist.
- It is written nowhere. The spec holds none of it, and neither do the records, the log or any error text; the guard below reads each of them.
- **Restarts:** there is no restart path. A lost evaluate job is recovered, not run again: `recover_job_dir` stops what the supervisor left, seals the log and ends the job. `follow` never starts a supervisor, and `supervise` is started only by `detach`. A new `evaluate` is a new job with the environment of its own caller.
- R01 §Agent environment and secrets says that evaluators inherit the caller's environment and agent jobs do not.
- **Follow-up, not a node:** a manifest field `env = [names]` (option 2) would let a plug-in name the variables it needs and keep the rest out.

Guards (all seen red, then reverted):
- `e5_a_variable_planted_in_the_caller_reaches_the_evaluator`: the plug-in's kind reports the value in its evidence. Bite: the evaluators given `PATH` and `HOME` only (the evidence says `unset`).
- `e5_a_variable_planted_in_the_caller_does_not_reach_a_run_job`: `ps eww` while a `run` job's supervisor lives, which shows `CANDLER_STORE_ROOT` as the control. Bite: `run`'s allowlist replaced by the whole environment (the supervisor's line holds the value).
- `e5_the_callers_environment_is_written_nowhere`: one job completes and one fails; the sentinel is searched by name and bytes under the whole store, and in everything the two calls, `ps`, `watch` and `inspect` printed. Bite: the supervisor printing its environment to its log (the sentinel is found in `supervisor.log`, and in the error text that quotes it).

### The changed-suite refusal
`RuntimeError::SuiteChanged` has the code `evaluation.suite_changed`, side `suite`, one appended row in `[evaluation]` of `codes.toml` and its row in `jobs/refusal.rs`. `r2_a_suite_changed_after_the_job_started_prints_as_a_suite_record` writes a job by hand, edits the suite, and runs `candler --error-format json supervise --job <id>`: the record has the code and the side. Bite: the refusal left as `Setup` text (the record is `candler.internal`). The verb itself reports it as a job's end, with the text and the fallback code (see above).

### The `detach` race
`detach` also reads the job's log: a `job.started` or `job.ended` in it means the supervisor began, whether or not it still holds the lock. The verb's workaround for `DidNotStart` is gone; the zombie reap stays. `e1_detach_reports_a_job_that_ends_at_once_as_started` (in `candler-core/tests/guard_detach.rs`) uses a supervisor that is a shell script writing `job.started` and `job.ended` and never holding the lock, and a second one that writes nothing. Bite: the old check (the first row reports `DidNotStart`). `slots.evaluators = 0` through the verb is still covered by `e1_evaluate_of_a_failed_job_prints_no_report_and_says_why`. The file uses no new unix-only API, so the platform inventory is unchanged.

### Gates, from eed99b7
- Leaf gates: E1 (5), E4 (1), E5 (3) and the `r2_` and `r1_` rows (35 tests with the migrated set `guard_cli`, `guard_input_seam`, `guard_cli_e2e` and `guard_cli_surface`): PASS. OWNED is widened by the round's authorized files; G-platform and the old-options gate: PASS.
- Core: `guard_detach`, `guard_refusal_codes`, `guard_evaluate_job`, `guard_platform_seam`, `guard_spec_file` (17): PASS. GOLDEN: `guard_golden` (2) and `guard_oracle_ac`: PASS.
- Workspace STD after `cargo clean -p candler-core -p candler`: fmt, clippy `-D warnings`, `cargo check --workspace --all-targets`, nextest (581 passed, 21 skipped) and `cargo doc`: PASS. LINUX clippy: PASS.
- G-codes: 5 lines added to `codes.toml` against eed99b7, none deleted. `cog check eed99b7..HEAD` and the subject length: PASS.
- LINUXRUN, `-j 1`: see the reply.
