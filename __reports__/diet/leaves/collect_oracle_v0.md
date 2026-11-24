# Collect oracle: leaf report v0

Conformance now checks every driver's `collect` against the files the scripted agent wrote, not
against the driver's own fixture (cause D1, defect B1). `Case` carries `agent_wrote`; the check
feeds those files to the driver's `collect` and asserts that every one is kept by its file globs,
naming the ones a driver leaves out.

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| With B1's fix reverted on a scratch branch, the shared conformance case fails for OpenCode only, naming the files the agent wrote | PASS | see §Bites: on the tip with the revert, `candler-harness-opencode conformance_opencode_recorded_runs` fails with `the collect does not keep the files the agent wrote: ['notes.md', 'outputs/answer.txt']`; `candler-harness-claude-code conformance_claude_code_fixtures` passes on the same tree |
| On the base, the same revert passes conformance | PASS | see §Bites: on `origin/feat/integrity` with the revert, `candler-harness-opencode conformance_opencode_recorded_runs` passes |
| The oracle is the set of files the scripted agent wrote; `Case::with_collect` stays only where it checks something the shared case does not | PASS | see §Oracle and §`with_collect` |
| `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass | PASS | `fmt --check` clean; clippy `-D warnings` clean; `nextest run --workspace` 741 passed, 32 skipped; doc with `-D warnings` generated |

## Oracle

The shared case's oracle is `Case::agent_wrote`: the set of files the scripted agent wrote, named
by the case and never copied from what a driver's fixture expects. `check_collect_oracle` runs the
driver's `collect` with no observed session (a harness's file globs do not depend on the sessions a
stream named) and reads only its `File` specs; a file is kept when a keep glob matches it and no
`!` glob drops it. The two harness drivers and the shape suite run the case. Because the workspace
holds only the files the agent wrote, "keeps exactly those files" reduces to "keeps every one the
agent wrote", and a missing file is named in the failure.

The glob matcher (`glob_matches`) reproduces the grammar the core's collector reads (`**`, `**/`,
`*`, `?`, literal characters, `!` exclusion), so it is not more lenient than
`candler-core::runner::exec::gather::collect_files`; the unit test
`conformance::tests::collect_glob_matches_the_core_grammar` pins the forms.

## `with_collect`

No `Case::with_collect` was removed: each remaining one checks something the shared case does not.

- `candler-harness-opencode/tests/it/conformance.rs` keeps its `with_collect` of the export `Exec`
  command. The oracle runs `collect` with no observed session, so it never exercises the export
  command the fixture pins; removing the fixture would lose that check.
- `conformance_shapes.rs` keeps the `lines_cases` glob (`home/.claude/projects/*/cc-1.jsonl`) and
  the `document_cases` export command, each tied to a session the oracle's session-less run never
  produces.
- `candler-harness-claude-code` never set `with_collect`; the shared case is where its `collect`
  first gets a check.

## Bites

Both bites rebuilt the OpenCode driver crate on its own scratch branch and target dir, per the
leaf's scratch rules. The revert of B1 is `git revert` of `8d88cb50` (the fix's change to
`OpenCodeDriver::collect`), resolved by keeping the revert's removal of the now-redundant
`guard_build_context` byte guard that B1 itself added; `driver.rs`, the opencode `conformance.rs`
and `guard_opencode_driver.rs` applied cleanly.

| Bite | Scratch branch (from) | Command | Red seen | Reverted |
|---|---|---|---|---|
| Revert B1 on the tip | `scratch/collect_oracle-tip` (from `d39e367f`, the tip with the shared case) | `CARGO_TARGET_DIR=…/collect_oracle-tip cargo nextest run -p candler-harness-opencode -E 'test(/conformance/)'` | `conformance_opencode_recorded_runs` FAILED: `Failure { case: Some("collect_keeps_the_files_the_agent_wrote"), check: "collect", detail: "the collect does not keep the files the agent wrote: [\"notes.md\", \"outputs/answer.txt\"]" }` | yes |
| Claude Code passes the same reverted tip | same scratch | `CARGO_TARGET_DIR=…/collect_oracle-tip cargo nextest run -p candler-harness-claude-code -E 'test(/conformance/)'` | none: 7 tests passed, `conformance_claude_code_fixtures` included | (no red) |
| Revert B1 on the base | `scratch/collect_oracle-base` (from `origin/feat/integrity`, no shared case) | `CARGO_TARGET_DIR=…/collect_oracle-base cargo nextest run -p candler-harness-opencode -E 'test(/conformance/)'` | none: `conformance_opencode_recorded_runs` passed | yes |

The tip bite shows the shared case catches B1 and names the files; the Claude Code pass on the same
tree shows it fails for OpenCode only; the base bite shows that without the shared case the same
revert is invisible.

## Deviations

- The leaf's step-1 commit subject is `test(harness): …`, but `harness` is not an allowed scope
  (CONTRIBUTING.md §Scopes and `cog.toml`); the commit-msg hook rejects it. The subject was
  committed as `test(library): check collect against the files the agent wrote`, using the scope
  the shared harness crate has used before, and the predicate of the subject is unchanged.
- A pre-existing flaky test, `candler-core::it guard_protocol_client::every_failure_path_of_an_evaluator_is_accounted_for`,
  failed once under full-suite parallel load (`NotMeasured("deadline")` where `Reported` was
  expected); it passed when run alone and on the final full `nextest run --workspace`, and this leaf
  touches none of its code.

## Sonnet fallback

The RiVault attempt (`d39e367f`) failed the second gate as the leaf writes it. This section
supersedes its Oracle, `with_collect` and Bites sections and the gate rows above that rest on them.

| What the RiVault attempt did | Why it failed | Superseded by |
|---|---|---|
| A hand-typed `agent_wrote` list, copied in three tests, with no workspace | No scripted agent wrote the files | The case writes `agent_wrote` into a real temp workspace, beside `harness_wrote` (a driver-private skill file) |
| `glob_matches`/`walk`, a hand copy of the core glob grammar | A second reader of the same input (D5) | Deleted, with its unit test; the setup's `collector` runs `candler_core::runner::exec::gather::collect_files`, the code real runs use |
| Asserted only that every file is kept | "Exactly" unchecked | The stored set must equal `agent_wrote`; the failure names `missing` and `extra` paths |
| OpenCode `with_collect` carried `**/*` and `!.opencode/**` globs | A second hand-typed collect spec | Replaced by `Case::with_collect_commands`, which compares only the command and exec specs (the export command); file specs are checked by the workspace case |

Placement: `candler-harness` cannot depend on `candler-core` (core depends on it). The collector is
passed in as `Setup::with_collector(fn(&Path, &[CollectSpec]) -> io::Result<Vec<String>>)`; each
test that calls `run` gives a three-line adapter over `collect_files`, with `candler-core` as a
dev-dependency of `candler-harness`, `candler-harness-claude-code` and `candler-harness-opencode`.
A case that names `agent_wrote` with no collector fails. Claude Code, OpenCode and the shape suite
run the case; the shape suite also shows a driver that stores nothing (names `missing`), one that
stores the private file (names `extra`), and one that stores exactly the agent's files (passes).

Bites, on scratch worktrees with their own target dirs, `git revert 8d88cb50` with
`guard_build_context.rs` reset to HEAD (the revert of the opencode `conformance.rs` was resolved to the
tip's file on the tip; it applied on the base):

| Tree | Result |
|---|---|
| Tip + revert (`Compiling candler-schemas, candler-testkit, candler-harness, candler-core, candler-harness-opencode, candler-harness-claude-code`) | 14 run, 13 passed, 1 failed: `candler-harness-opencode conformance_opencode_recorded_runs`: `the collect does not store exactly the files the agent wrote: missing ["notes.md", "outputs/answer.txt"], extra []`; Claude Code passes |
| Base + revert, fresh compile of the same crates | 14 run, 14 passed |
| Tip | fmt clean, clippy `-D warnings` clean, `cargo doc -D warnings` clean, 14 conformance tests pass |

## Verifier

Left for the lead's fresh Sonnet verifier, with this leaf, this report and the diff.

## Left

Nothing in scope. The build gates pass, and `container-e2e` is a repository check for the lead to
watch on the pull request.
