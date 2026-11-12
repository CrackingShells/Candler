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
