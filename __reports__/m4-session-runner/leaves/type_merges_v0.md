# Leaf report: type_merges (v0)

Type: findings. Branch `m4/type_merges`, base 914f192.

## Commits
| sha | subject |
|:----|:--------|
| 2e23008 | refactor(schemas): name the session's parent field without the stutter |
| a2085b8 | refactor(runner)!: frame streams with the reader's own framing type |

The report is committed last, as `reports(runner): record the type_merges leaf`.

## Gates
| gate | result | last relevant line |
|:-----|:-------|:-------------------|
| `FrameMode` gone | PASS | `git grep` finds nothing in `crates/**` |
| runner reads `.framing()` | PASS | match in `crates/candler-core/src/runner/exec/mod.rs` |
| frame and supervision guards (frames, seams, supervise, public_surface) | PASS | 39 tests run: 39 passed |
| schemas commit leaves schema files and accessor | PASS | `git diff --quiet a26fe53 -- crates/candler-schemas/schema` exits 0, `parent_session` accessor present |
| one schemas commit touches only the schemas crate | PASS for mine, see finding 1 | gate form prints 30dcde1, not mine |
| step 1 check: `-p candler-schemas` and schema diff | PASS | 198 passed |
| DRIFT | PASS | 2 passed, schema dir clean |
| SPAWN | PASS | guard_signal 9/9 |
| STD (`-p` schemas, core, harness) | PASS | fmt clean, clippy clean, 385 passed (14 skipped), doc clean |
| LINUX | PASS | clippy on x86_64-unknown-linux-gnu, workspace tests, finished |
| DIGEST | PASS | numstat and field_paths diff clean, guard_cell_digest 9/9, import/legacy/oracle/run_context/engine/seams 39/39 |
| GOLDEN | PASS | guard_golden 2/2, guard_oracle_ac 1/1 |
| SUBJ (base 914f192) | PASS | `cog check`: No errored commits; all subjects at most 72 characters |
| OWNED | PASS | `git diff --name-only 914f192..HEAD` lists 11 paths, all in OWNED |

## Guards
The leaf names no new guard. The existing guards in `guard_frames` (one Document case), `guard_seams`, `guard_supervise` and `public_surface` were migrated to `Framing` with no change to an oracle or an expected value, and they pass. `guard_cell_digest` and the schema vectors, which pin the wire form of `Session`, pass unchanged. No bite was injected.

## Findings
1. **Spec defect in the G10 and leaf gate form (flagged).** The "touches only the schemas crate" command prints 30dcde1 (`build(deps): add the statistics and error-path crates for m3`). That commit is on `main` below the base and touches `crates/candler-schemas/Cargo.toml` and `Cargo.lock`. It is a `build` commit, so G10's release-triggering count (feat|fix|perf|refactor|style|revert) is unaffected: the count is 1, mine. The leaf's own gate (the loop) is not clean on this branch and will not be on the milestone tip. Request: restrict the loop to release-triggering types, or exclude 30dcde1.
2. **Untested behaviour (flagged).** The runner's choice of framing from the live reader has no guard. No test in the workspace runs a reader whose `framing()` is `Document` through `runner/exec`. The only evidence is the `.framing()` grep gate and the Claude Code reader being `Lines`, so its runs are unchanged (GOLDEN and DIGEST pass). The leaf's OWNED list has no runner test file, so I did not add one. The `traits` or `session_runner` leaf that first has a Document reader should carry the guard.
3. **Deviations.** None from the leaf. In `spec.rs` the `Framing` lint reason now reads "changes this enum and the frame writer together" (the leaf said only to reword). The `FrameMode` lint reason went with the enum.
4. **Behaviour change.** `candler_core::frames::FrameMode` is removed (approved, `BREAKING CHANGE:` footer names `candler_harness::spec::Framing`, `Whole` becomes `Document`). No refused input, no moved digest.
5. **Names.** `Session::parent` (private) and `SessionBuilder::parent` (private) as in the leaf; the accessor `parent_session()` and the builder setter keep their names; the wire name is kept by `#[serde(rename = "parent_session")]`. `RawSession` keeps the wire name as its field.
