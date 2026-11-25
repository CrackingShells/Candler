# Sealed run producer leaf, v0

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| With the pre-S1 sealing reapplied, the output-reading guards fail on the tip and stay green on the base | PASS | §Bites |
| Probe wall times are in the report | PASS | §Probe |
| No guard on S1's path calls `seal_synthetic`; it is deleted; no new hand-made seeder | PASS | deleted; two cases the runner cannot produce use `seal_workspace` over `seal_run` (§Probe, Choice) |
| No digest or golden vector edited | PASS | the diff touches no `*.toml` vector, no manifest digest and no schema |
| fmt, clippy, nextest, doc | PASS | §Checks |
| `container-e2e` green on the pull request | pending the PR | |

## Probe

Question: can the real runner produce the state of the analyze-heavy guards?

| What | Wall time | Source |
|---|---|---|
| `guard_analyze` guards over the hand-made store (30 to 32 runs, two engines' outcomes) | 1.1 s to 4.8 s each, 13 tests with `guard_run_evaluate` in 7.1 s | nextest, `-p candler`, tests in parallel |
| one scripted native run through the runner (`candler run`, PATH-fake docker, scripted agent; `World::launch`) | 4.3 s to 5.1 s each (`guard_cli_run::t3_cli_two_levels_plan_two_cells` 5.1 s, `b35_run_returns_supervisor_outlives` 4.3 s) | nextest, 10 tests in parallel |
| the same plus a scripted evaluator (uv plug-in) and `candler evaluate` (`guard_run_evaluate`) | 5.0 s to 7.2 s | nextest |

The fake agent holds each container for one second, and a run adds the supervisor and the
store seal. The analyze guards need three arms of ten valid runs and two invalid ones, so 32 native
runs: at about 4.5 s each that is minutes, against 1 to 5 s. Also, `analyze` reads outcomes and
manifests, never outputs, so no analyze guard is on S1's path.

**Choice (second round).** The probe does not show the real runner too slow: a scripted native run
through `candler run` costs about 4.5 s on an idle machine (9 to 27 s per guard when nine runs go
at once on a loaded one). So the guards that read what an agent left take their runs from it. The
first round's `seal_workspace` for all of them is superseded.

The one native producer is `World::launch` (crates/candler/tests/it/support/cli_run.rs): the
PATH-fake docker whose agent, started with `start -a`, copies the files `World::agent_leaves`
set into its workspace (by default it writes `answer.txt`). The runner collects, filters, hashes
and seals them. No evaluator is scripted: these guards read outputs, not outcomes.

Moved to the real runner, in `crates/candler/tests/it/guard_run_context_files.rs`:
`the_file_rule_decides_the_paths_and_the_digest` (six cases, one run each, one after another) and
`a_materialised_file_is_a_private_read_only_copy` (the 0444 half).

Kept on the fallback, `seal_workspace` (crates/candler-core/tests/it/support/sealed_run.rs, which
calls the runner's `seal_run`), for two cases that the real runner cannot produce:
- `.claude/` stored in a run: the runner never collects it, `EXCLUDED_DIRS` in
  `crates/candler-harness-claude-code/src/discovery.rs:6-7`. Guard:
  `the_file_rule_treats_a_stored_dot_claude_by_its_glob`.
- an executable tree entry: the runner's tree marks every file `executable = false`,
  `crates/candler-core/src/runner/evidence/tree.rs`, `TreeEntry::file(path, ..., false)`. Guard:
  `an_executable_entry_materialises_as_a_private_0555_copy`. Flag: no real run can ever carry an
  exec bit, so the 0555 materialisation is unreachable from real use today.

The trade for those two: the runner's collection and tree filter are not exercised for them,
only its sealing step; the seeder calls `seal_run` and does not copy it.

`seal_with_inputs` and `store_inputs` (`guard_run_context.rs`) stay: they seal runs with an empty
tree and store input-list blobs, so they read inputs and context digests, not outputs, and
stay green when `seal_run` drops output blobs.

## Producer

- Real runner: `World::agent_leaves(&[(path, bytes)])` then `World::launch()`, `finish`, and
  `Store::open(&world.store())`; `sealed_run_with(files)` in `guard_run_context_files.rs` does the four.
- Fallback: `seal_workspace(store, template, attempt, files) -> RunKey` and `File`.

## Bites

The pre-S1 sealing is `seal_run_observed` without its first loop (`writer.output` for each file of
the tree): digests in the tree, no blobs. The earlier bite outputs came from one shared target dir
and are superseded: it served a stale test binary. Each revision below was built in its own fresh
target dir (`integrity-target/sealed_run_producer-base`, `-tip`, deleted after) with
`cargo nextest run -p candler-core -p candler -E 'test(/guard_run_context/) | test(/guard_run_evaluate::/)'`.

| Revision with the revert | Compile lines | Result |
|---|---|---|
| base `origin/feat/integrity` | `Compiling candler-core v0.15.0 (.../integrity-scratch/sealed_run_producer-1/crates/candler-core)`, `Compiling candler v0.15.0 (.../sealed_run_producer-1/crates/candler)` | 10 run: 9 `guard_run_context` PASS (`the_file_rule_decides_the_paths_and_the_digest` PASS, `a_materialised_file_is_a_private_read_only_copy` PASS), `guard_run_evaluate` FAIL |
| tip | same lines with `sealed_run_producer-2` | 12 run: 7 passed, 5 failed: `guard_run_context_files::the_file_rule_decides_the_paths_and_the_digest`, `guard_run_context_files::a_materialised_file_is_a_private_read_only_copy`, `guard_run_context::an_executable_entry_materialises_as_a_private_0555_copy`, `guard_run_context::the_file_rule_treats_a_stored_dot_claude_by_its_glob`, `guard_run_evaluate` FAIL; the context ones with `the run materialises: Io { op: "open a blob" ... NotFound }` |

On the base the output-reading `guard_run_context` guards stay green because `seal_synthetic`
stores the blobs itself, which is S1's mechanism. On the tip they fail on the missing blob.
`guard_run_evaluate` already failed on the base, through the real runner.

## Checks

Leaf target dir, machine loaded by other builds: `cargo fmt --check` exit 0; clippy `--workspace --all-targets -D warnings` exit 0; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` exit 0. `cargo nextest run --workspace --no-fail-fast`: 742 run, 740 passed, 2 failed
(`guard_supervise::the_parents_environment_reaches_neither_the_child_nor_its_pre_stop`,
`guard_wiring::wiring_stop_execs_in_the_container_with_the_observed_id`, 56 s). An earlier run failed
`guard_container_record::stopped_run_has_its_agent_recorded_before_the_hook_sweeps` instead, at 72 s. All three
pass alone (0.7 s, 12.7 s, 130 s), touch nothing this leaf changed, and look like timing under load.

## CI failure of 299469f9

`Gates` failed on `guard_run_context_files::the_file_rule_decides_the_paths_and_the_digest` (4.06 s): two of
the six case threads panicked at `support/cli_run.rs:331`, "no run sealed: 0 sealed, 1 expected". The fake
client's call log showed a job that built the gateway and the networks, started (or not even started) the
egress proxy, then tore down (`container stop`, `rm`, `network rm`) with no agent container ever created, while
`finish` saw `job.ended`. `container-e2e` was green. The log holds no job events, so the reason the two jobs
ended is not recorded; `assert_sealed` now prints the jobs' `events.jsonl` so the next occurrence names it.

Cause, as far as it was shown: the six `candler run` worlds were started at once by threads inside one test, on
a two-core runner that also runs the other tests of the binary. Every other guard in the binary runs one world.
Locally the same test started eight times at once (48 worlds) failed in two copies, there in world setup
(`git commit` of the skills repository failed, `cli_run.rs:221`), so concurrency among worlds is what breaks;
the two symptoms differ, and the CI one was not reproduced. The cases now run one after another, about 4.5 s each;
four copies of both guards at once (28 worlds) then all passed. No retry and no longer timeout was added.

## Verifier

A fresh Sonnet agent was given the leaf, this report and the diff, twice: on the first round's diff
and on this one's. Both times it found no guard on S1's path that still makes its own sealed run. It
swept `put_blob|writer.output|RunWriter::create|TreeEntry::file|RunTree::builder|seal_run|seal_workspace`
over the tests: the remaining hand-made runs (`seal_with_inputs`, `store_inputs`) seal an empty tree and
read inputs; `guard_declared_inputs`, `guard_seams`, `guard_store`, `guard_run_exec` and the
analyze, CLI and golden seeders read no output bytes. It confirmed both reasons for the fallback:
`EXCLUDED_DIRS = [".claude"]` (`discovery.rs:7`) and `TreeEntry::file(path, digest, false)`
(`tree.rs:79`).

## Follow-ups

- `guard_run_context`'s `mini_store` guards (`a_plug_in_cannot_write_through_to_the_store`,
  `a_corrupt_blob_is_refused_and_not_digested`) read outputs of imported runs, whose producer is the
  importer; they stay green under the revert and are not on S1's path.
- Analyze, CLI and golden seeders (`guard_analyze.rs`, `guard_cli.rs`, `guard_cli_surface.rs`,
  `guard_golden_design.rs`) are untouched: none reads outputs.
- Caps for a later pass: a real native run costs about 4.5 s under parallel load, so a store of
  more than a handful of real runs needs its own cap.
