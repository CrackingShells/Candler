# session_runner leaf, round 0

Type: findings. Branch `m4/session_runner`, cut from 40ef2fe. Target dir and scratch removed; docker lists equal before and after (images 12, containers, networks; the two images built here were removed by id).

**Verdict.** Done. A `runner.mode = "session"` cell runs with stdin open, is closed only once a turn has ended with nothing pending and `runner.idle_s` has passed, and records how it stopped. Headless cells, argv and digests are unmoved. Two things need a ruling: findings A and H.

## Commits

| Commit | Subject |
|---|---|
| c368298 | feat(experiment): opt a cell into session mode |
| accfc65 | feat(isolation): keep a session's stdin open |
| bad5e11 | feat(runner): close a session only once the agent has yielded |
| 19b40af | test(runner): drive real sessions against the fake model |
| a48b771 | refactor(runner): name the session mode, mark dropped errors (review fixes; squash into bad5e11 if wanted) |

## Gates

| Gate | Result | Last line |
|---|---|---|
| S1 `session_close_*` | PASS | 15 tests run: 15 passed (S1-S3, S5, P1, harness_ in one call) |
| S2 `session_measures_*` | PASS | same |
| S3 `session_hooks_*` | PASS | same |
| S4 `session_e2e_*` (Docker) | PASS | 2 tests run: 2 passed, 282 skipped |
| S5 `session_resolve_*`, `harness_resolution_*` | PASS | same 15 |
| P1 `session_profiles_*` | PASS | same 15 |
| `harness_env_*`, field_paths diff | PASS | no `-` lines vs cf3de30 (8 added lines) |
| SPAWN | PASS | 9 tests run: 9 passed |
| STD (`-p candler-core -p candler`) | PASS | 402 tests run: 402 passed, 23 skipped |
| LINUX | PASS | Finished `dev` profile |
| DIGEST | PASS | 9 passed; 45 passed |
| GOLDEN | PASS | 2 passed (2 slow); 1 passed |
| SUBJ | PASS | No errored commits; all subjects <= 72 |
| clippy `-D warnings` all targets, `cargo doc -D warnings` | PASS | clean |
| Reviewer (rust-conventions) | 3 minor findings fixed in a48b771 | |

## Guards, oracles, bites

Guards: S1 `session_close_replays_the_recorded_sessions`, `session_close_ignores_records_that_do_not_change_the_state`, `session_close_wired_through_the_run_loop`; S2 `session_measures_by_how_the_session_ended`; S3 `session_hooks_never_decide_whatever_the_log_does`, `session_hooks_leave_the_run_as_it_is`; S4 `session_e2e_c1_waits_then_closes`, `session_e2e_c2_closes_at_idle`; S5 `session_resolve_table`, `session_resolve_factor_levels_and_known_paths`; P1 `session_profiles_table_levels_reach_each_cells_argv`; `harness_env_session_adds_input_flag_only_when_interactive`. Every test name carries a guard id; none is extra.

Oracles: the recorded sessions c1 and c2 with their `timings.toml`; a Python stub whose own monotonic trace gives when each line was written and when stdin reached EOF; the scripts by hand; `sh -c` runs of the hook commands; the fake model's scripts for S4.

| Bite | Red, failing message seen |
|---|---|
| Idle counted from the last event | S1: `no extra record: due at 5.08932s, expected 3062.01 ms`; wired: `row 2: the input closed 1.608 s after the result, with a frame at 0.6 s` |
| Closed session recorded as a cap | S2: `closed: session.end` left `cap`, right `closed` |
| Hook exits 2 | S3: `SessionEnd with a home that does not exist: the hook exits 0` left 2, right 0 |
| `-i` removed | S4: `the run is valid: Some(ReaderError)` |
| Pending list ignored in the close rule | S4 c1: `session.turns` 1, expected 2 |
| `runner.mode` defaulted to headless in every cell | S5: `session_resolve_table` FAIL. The H4 guard was not run (not in this leaf's filter) |
| Capability check removed | S5 red (seen at unit level, round 1) |

All bites restored from copies; `git diff --stat` unchanged after each round.

## Findings

A. **Spec defect: a factor-level `runner.mode`.** S5 says a session as a factor level derives `runner.idle_s = 60`. Preflight steps see only the experiment-wide values, not per-cell levels, so the step cannot derive per cell. Done instead: `session_config` validates mode and idle again at run time and applies the default of 60 there. A cell from a factor level therefore carries no derived `runner.idle_s` in its values (the run records `session.idle_s = 60`). Needs a ruling if the stored value must be present.

B. **Binary edit outside OWNED**: `crates/candler/src/commands/preflight.rs` passes `driver.capabilities().session` into `M2bServices.session_capable`, a new field of the core struct. Needed for the "driver without the capability" refusal.

C. **Also outside the listed files**: `runner/exec/spawn.rs` (the live observer feeds the session watch) and `interactive: false` in the `ContainerPlan` literals of five test files.

D. The turn cap fires when the count exceeds the limit (`>`), not on reaching it, so `--max-turns` of the harness and the cell's limit agree on the last allowed turn.

E. `HOOK_SETTLE` = 250 ms: the Stop hook's log line can be visible on the host up to ~11 ms after `result`; a close is not decided before the settle and the log is re-drained first.

F. New measured path `session.idle_s` (the session facts left it implicit).

G. `run.num_turns` is the sum over the session's results; `run.usage` is the last result's only.

H. **S4 is not literally "through `candler run`".** The CLI has no way to trust the fake model's certificate authority. S4 builds the real pieces (gateway sidecar, `DockerIsolation`, the run loop, the Claude Code driver) by hand. Separately, the gate regex `^session_e2e_` cannot match tests inside a module named `session_e2e`; the module is named `session_e2e_runs` so the gate matches. `guard_container_e2e.rs` also gained `mod support;` and a crate-level `allow(clippy::duplicate_mod)` because the e2e modules path-include support files that `support` also holds.

I. A session driver that closes by an action (not EOF) is refused at run time.

J. **Shared hunks for the coordinator**: `runner/exec/mod.rs` in `drive()` (session config, `SessionRun::prepare`/`start`/`finish`, `live_observer_with`, `EndInput.with_session_stop`/`with_session_cap`, session totals, `session_values`, hook raw stream into `raw_files`); `isolation/container/mod.rs` one field in `plan()` (`interactive`).

K. Tests were written after the code; their value is shown by the bites above, not by a red-first history.

L. Hook-log visibility delay is measured on macOS Docker Desktop only; not measured on Linux.

M. Closing while a notification is racing is safe: EOF mid-turn lets the turn finish (session facts, row 8).

N. The hook raw stream is named `hooks.jsonl` by the Claude Code driver, so the archive is `raw/hooks.jsonl.jsonl.gz`. The test pins the real path. Renaming the stream to `hooks` in the driver would tidy it.

O. The e2e world uses the Rig's fixed job id; the e2e group runs serially, so no clash, but two sessions on one machine would collide.

## New codes

None. No new error variant, no `refusal_table!` row, `codes.toml` and the contract pages untouched (`contract_pages` passes in STD).

## For the verifier

Close cannot fire mid-turn or with pending work: one mutex covers tracker and rule; the decision re-drains the hook log after `HOOK_SETTLE`; the pure rule is replayed on both recorded sessions. An experimenter's stop (kill, budget) is classified as ours, makes the run invalid and records no `session.end` (S2 rows 6 and 7).

## Fix round 1

The verifier's findings, each closed by a row in an existing guard, seen red by the bite named. Bites were restored from copies. Commits: bc01d41 (B1), ccfdaba (M1/M2, L5, L6), 1af50af (M3, M4, M5, L1, L2, L8, L10).

| Finding | Closed by | Bite, failing message seen |
|---|---|---|
| B1 factor-level `runner.mode` | `session::settle()`, one pure rule called by the preflight step and by `fresh_cells` on each cell's own values; the cell gets `runner.idle_s = 60` with a Derived source (`cells_of` records it unless a factor sets the idle). Row: `session_resolve_settles_each_cell` | rule skipped in `fresh_cells`: the factor cell has no `runner.idle_s`, so it differs from its `[fixed]` twin |
| B1 refusals | New code `experiment.session_refused` (Side::Experiment, enum `SessionRefusal`, variant `Refused`; wrapped by `ResolveError::Session` and `PlanError::Session`), used at resolve and at planning. Rows: bad factor mode and no capability refused when planning; `[fixed] runner.idle_s` with factor modes `headless`, `session` is refused as a whole at resolve (idle applies to session cells only) | same test |
| M1/M2 | The tracker's state is known only once there are as many Stop lines as ended turns. Rows in `session_close_needs_a_stop_for_every_turn`: result before a late Stop, Stop before result, Stop-less turn after a quiet one, wake-up in a late Stop. The existing "second turn" row now needs its Stop line | known without the count: `a second turn ends with no Stop line` due at 3.1 s, expected never |
| M3 | `run.usage` is the sum over the results, read from each result frame; `run.cost_usd` is the harness's cumulative value (checked in c1: 0.0016, then 0.0024), `run.num_turns` the sum. Row: `session_measures_run_values_are_session_totals` | last result's usage: input tokens 100, expected 300 |
| M4 | A wall end with no Stop line ever read is invalid as infrastructure (`ended_by` unknown), with `session.end = hooks_silent`. The wall row now runs with Stop lines; a new row has none | check removed: `run.ended_by` cap wall, expected unknown |
| M5 | Row: limit 2 with one result of 2 turns continues and closes | `>=`: `run.ended_by` cap turns, expected completed |
| L1 | Turns are read from each result frame itself; a result with no `num_turns` adds none. Row: `session_measures_turns_come_from_each_result` | absent count read as 2: 4, expected 2 |
| L2 | The close rule reads its clock under the lock. `idle_s = 0` is documented (never sooner than the 250 ms settle). `yielded_at_ms` is documented from the runner's preparation of the session, a few ms before the container starts, not the attempt start (docs fixed; the code is unchanged) | none (documentation, ordering) |
| L5 | The hook redirects its own stderr; `session_hooks_never_decide_whatever_the_log_does` now asserts stderr is empty. The earlier report line "no output" was wrong for a failing log write | before the fix: `sh: .../missing/.candler/hooks.jsonl: No such file or directory` on stderr |
| L6 | The stream is named `hooks`; the archive is `raw/hooks.jsonl.gz` | |
| L8 | S4 no longer names a base tag: it takes the `candler-base:src-*` image the machine holds and says how to build one. The digest function is private to the image service, and building the base downloads the harness binary | |
| L10 | One step name (`session_mode@1`) for the derived values and the measures; `session_config` is the run-time backstop over `settle` | |

Notes without a fix. L4: the session's own stop is folded into "ours" in `classify_end`, as a deadline is. L7: S3 is behavioural on Candler's side only; what Claude Code does with a hook that fails is not exercised here.

Other changes of this round: `PlanCtx` gained `session_capable: Option<bool>` (the job runtime and `candler run` pass the driver's capability; `experiment_of` passes none); `plan_job` in the binary takes it; `ResolveError` and `PlanError` gained `Session`; `End` gained `hooks_silent`; the S4 world and guard rows above.

New code: `experiment.session_refused`, with its `refusal_table!` rows, its `codes.toml` row under `[resolve]` and the regenerated `docs/contract/experiment-file.md`.

Gates after the round: S1, S2, S3, S5, P1, `harness_env_`, `harness_resolution_`, resolve-surface and container-run binaries pass; S4 2/2 with Docker (images removed by exact id, lists equal); SPAWN 9/9; the workspace 656/657 (see below); LINUX, clippy and doc clean; DIGEST 9 + 45; GOLDEN 2 + 1; SUBJ clean.

**Environment finding.** The shared repository's `core.bare` became `true` while the round ran, so `git` fails in every worktree (`this operation must be run in a work tree`). I did not change the shared config: I ran git with `--work-tree` and `-c core.bare=false`. The one workspace failure, `guard_eval_infra::this_checkout_records_the_full_commit`, is this and nothing in the leaf; it needs `git config core.bare false` on the main checkout.
