# Sealed run producer leaf, v0

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| With the pre-S1 sealing reapplied, the output-reading guards fail on the tip and stay green on the base | PASS | §Bites |
| Probe wall times are in the report | PASS | §Probe |
| No guard on S1's path calls `seal_synthetic`; it is deleted; no new hand-made seeder | PASS | `seal_synthetic` and its `File` struct are gone from `guard_run_context.rs`; its two guards call `support::sealed_run::seal_workspace`, which calls the runner's `seal_run` |
| No digest or golden vector edited | PASS | the diff touches no `*.toml` vector, no manifest digest and no schema |
| fmt, clippy, nextest, doc | PASS | §Checks |
| `container-e2e` green on the pull request | pending the PR | |

## Probe

Question: can the real runner produce the state of the analyze-heavy guards?

| What | Wall time | Source |
|---|---|---|
| `guard_analyze` guards over the hand-made store (30 to 32 runs, two engines' outcomes) | 1.1 s to 4.8 s each, 13 tests with `guard_run_evaluate` in 7.1 s | nextest, `-p candler`, tests in parallel |
| one scripted native run through `candler run` (PATH-fake docker whose agent writes `answer.txt`, `World::launch`) | 4.3 s to 5.1 s each (`guard_cli_run::t3_cli_two_levels_plan_two_cells` 5.1 s, `b35_run_returns_supervisor_outlives` 4.3 s) | nextest, 10 tests in parallel |
| the same plus a scripted evaluator (uv plug-in) and `candler evaluate` (`guard_run_evaluate`) | 5.0 s to 7.2 s | nextest |

The fake agent holds each container for one second, and a run adds the supervisor and the
store seal. The analyze guards need three arms of ten valid runs and two invalid ones, so 32 native
runs: at about 4.5 s each that is minutes, against 1 to 5 s. Also, `analyze` reads outcomes and
manifests, never outputs, so no analyze guard is on S1's path.

**Choice.** The analyze guards keep their seeder (they are not on S1's path, and the plan leaves
other seeders until someone touches them). The guards that read outputs (`guard_run_context`, the
only callers of `seal_synthetic`) take their runs from one function, `seal_workspace`, that
seals through the runner's own `seal_run`.

Why not the real runner for those two guards: the file-rule cases need an exact stored file set
(`.claude/`, `._x`, `.DS_Store`, an executable bit), one run per case; through `candler run` each
costs about 4.5 s and the file set would come from the fake agent's script and the runner's own
tree filter, not from the case. The core's in-process rig (`Rig`, `FakeAgent`) has an agent that
writes no file.

**The trade, named.** `seal_workspace` writes the files and the tree itself, so the runner's
collection of a workspace is not exercised; only its sealing step is: the blob of each file, the
manifest, the tree, `run.done`. The seeder calls `seal_run`, it does not copy it. The real runner
under a scripted agent stays the producer for the one guard that reads outputs through the
binary, `guard_run_evaluate`, which already goes through `World::launch` and fails on the base
under the revert too (§Bites).

## Producer

`crates/candler-core/tests/it/support/sealed_run.rs`:

- `File { path, bytes, executable }`
- `seal_workspace(store, template, attempt, files) -> RunKey`

## Bites

The pre-S1 sealing is `seal_run_observed` without its first loop (`writer.output` for each file of
the tree): digests in the tree, no blobs. Each worktree was cut from its revision, the loop
removed, and the output-reading guards run with
`cargo nextest run -p candler-core -p candler -E 'test(/guard_run_context::/) | test(/guard_run_evaluate::/)'`.

| Revision with the revert | guard_run_context (9) | guard_run_evaluate (1) |
|---|---|---|
| base `origin/feat/integrity` | 9 passed (`the_file_rule_decides_the_paths_and_the_digest` 0.94 s PASS, `a_materialised_file_is_a_private_read_only_copy` 0.93 s PASS) | FAIL, `evaluate exits 0: ExitStatus(512)` |
| tip | 7 passed, 2 failed: `the_file_rule_decides_the_paths_and_the_digest`, `a_materialised_file_is_a_private_read_only_copy`, both `the run materialises: Io { op: "open a blob", ... NotFound }` | FAIL, same |

On the base the two `guard_run_context` guards stay green because `seal_synthetic` stores the blobs
itself, which is S1's mechanism. On the tip they fail on the missing blob.
A first run of the tip showed them green: the test binary had not been rebuilt in the shared target
directory; both rows come from runs after `touch` of the sources and the compile lines in the
output.

## Checks

With the target dir of the leaf: `cargo fmt --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo nextest run --workspace` 740 passed, 32 skipped, 0 failed; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` exit 0.

## Verifier

A fresh Sonnet agent was given the leaf, this report and the diff, and asked to find a guard on S1's
path that still makes its own sealed run. It found none. It checked every hit of
`put_blob|writer.output|RunWriter::create|TreeEntry::file|RunTree::builder|seal_synthetic|seal_run`
over `crates/*/tests`: the two moved guards call `seal_workspace`; `seal_with_inputs` seals an empty
tree and reads no output; `guard_seams`, `guard_store`, `guard_run_exec`, the analyze, CLI, inspect
and golden seeders read no output bytes; `guard_run_evaluate` goes through the binary. The residual
hand-built tree in `seal_workspace` feeds `seal_run`, which is the named trade.

## Follow-ups

- `guard_run_context`'s `mini_store` guards (`a_plug_in_cannot_write_through_to_the_store`,
  `a_corrupt_blob_is_refused_and_not_digested`) read outputs of imported runs, whose producer is the
  importer; they stay green under the revert and are not on S1's path.
- Analyze, CLI and golden seeders (`guard_analyze.rs`, `guard_cli.rs`, `guard_cli_surface.rs`,
  `guard_golden_design.rs`) are untouched: none reads outputs.
- Caps for a later pass: a real native run costs about 4.5 s under parallel load, so a store of
  more than a handful of real runs needs its own cap.
