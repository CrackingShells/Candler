# declared_inputs leaf: report v0

The `declared_inputs` leaf of M2c is implemented on `m2c/declared_inputs` (cut from `feat/m2c-wire-alignment` at 2aa7afd). Declared agent inputs are hashed, stored, merged into the `inputs` root by the image step, and mounted read-only file by file. Evaluator-only inputs are stored and listed in the manifest only. One defect in the design needs a decision (F1), and the leaf's `run.rs` edit goes one hunk beyond OWNED (F2).

## 1. Commits

| sha | subject |
|:----|:--------|
| b5c06cb | `reports(isolation): record how many read-only input mounts docker takes` |
| 0ad953c | `feat(experiment): hash and store the inputs an experiment declares` |
| 8e48c96 | `feat(runner): mount agent inputs and record evaluator-only inputs` |

The report commit follows these.

## 2. Gates

| gate | result | last relevant line |
|:-----|:-------|:-------------------|
| I1 `declared_input_` | PASS | 10 tests under the two prefixes pass (I1, I3, I4 and the resume test share the prefix) |
| I2 `evaluator_only_input_` | PASS | `evaluator_only_input_never_reaches_the_agent` passes |
| I3 `declared_input_refused_` | PASS | 1 test, 12 cases |
| I4 `declared_input_with_build_inputs` | PASS | 1 test |
| resume refuses a changed byte, naming `inputs.doc` | PASS | `declared_input_changed_byte_is_refused_on_resume` |
| G5c `guard_run_exec`, `guard_job_orchestrate`, `guard_run_plan`, `guard_image` | PASS | 8 tests across 4 binaries, 8 passed |
| STD, `-p candler-core -p candler` | PASS, with one exclusion | 166 passed, 18 skipped; fmt, clippy `-D warnings`, doc `-D warnings` clean. The filter was `not test(nuisance_set_is_the_hand_written_one)`, as the coordinator told me: that test is red on the base (`wire` added `evaluation_inputs` to the schemas' nuisance paths; `verdict_gating` fixes `compare.rs`) |
| LINUX (`-p candler-core -p candler`) | PASS | clippy for `x86_64-unknown-linux-gnu`, Finished |
| SPAWN | PASS | `guard_signal` 9 passed, before the test runs |
| SUBJ (base 2aa7afd) | PASS | `cog check`: No errored commits; no subject over 72 characters |
| OWNED check | PASS with 4 deviations (F2, F3) | see Findings |
| DRIFT, G5a | not applicable | no schemas file and no test vector touched |
| no `fn confirm_` | PASS | `git grep` finds none |


## 3. Guards

All in `crates/candler-core/tests/guard_declared_inputs.rs`. Every bite below was injected, seen red with the message quoted, and restored (`cp` from a backup; the tree was clean afterwards). Only I3 and I4 were red before their code: I1, I2 and the resume test exercise code that was already in the tree (`mount_inputs`, the leak check, the resume comparison), so they were green on arrival and their bites are the evidence that they can fail.

| id | test | Guards | Oracle | Bite and the failure seen |
|:---|:-----|:-------|:-------|:--------------------------|
| I3 | `declared_input_refused_names_kind_and_path` | I14: a wrongly declared input is refused, naming kind and path | hand-written table, 14 cases | `..` accepted (segment test changed): `parent segment: ResolvedInputs { agent: [... kind "up" ...] ... }` (expect_err on an Ok) |
| I4 | `declared_input_with_build_inputs_merge_into_one_sorted_root` and `declared_input_without_build_inputs_stands_alone` | one sorted `inputs` root from the one deriver | hand-written digests and order | a second step deriving `inputs`: `resolves with no step deriving a path twice: DerivedTwice { path: "inputs", first: "image_id@1", second: "second@1" }` |
| I1 | `declared_input_agent_files_are_mounted_readonly_and_digested` | `comparable.inputs` holds one `{kind, digest}` per agent input; every file is a read-only bind mount at `at/<path>` from its blob | list digests worked by hand from the RFC 8785 text (sha2 over a literal string); argv expectations hand-written | mount without `readonly`: ``a.txt` is mounted once, read-only` left 0 right 1`; a directory file dropped: ``srv/sub/two.txt` is mounted once, read-only` left 0 right 1` |
| I1/B24 | `declared_input_copied_into_the_output_is_an_input_leak` | an output that copies an input is `invalid(input_leak)`, a run that copies nothing is not | hand-written; the copy is the input file's bytes under another name | not bitten separately: the leak check is existing code with its own guard (`guard_run_evidence`); this one proves it sees the job's inputs |
| I2 | `evaluator_only_input_never_reaches_the_agent` (fix round 1: also the list and file digests in argv, mounts, workspace, `tree.json`; no mount under the jobs directory) | I8: absent from `JobSpec.inputs`, argv, workspace, run dir, job records, `tree.json` and `comparable`; present in `evaluation_inputs` with list and file blobs | a canary string in the input's files and the list text written out by hand; the agent input is the control that the scans see a mounted input | every input routed into the mounts: `only the agent's file is a mount` left 3 right 1 |
| resume | `declared_input_changed_byte_is_refused_on_resume` | I11: a changed input byte is refused, naming `inputs.doc` with the stored and observed digests; the same input resumes | the literal path; digests from the two resolutions | list digest taken over an empty blob (content ignored): `a changed input byte must be refused` |
| F1 | `declared_input_with_an_empty_file_runs_clean`, `declared_input_mount_points_are_not_outputs`, `declared_input_empty_match_is_no_leak_and_a_real_copy_is` | I10: a tree with empty files (`zero.txt`, `pkg/__init__.py`) is declarable and no run of it is `input_leak`; the mount points are not outputs (a hand-built workspace lists exactly `empty_output.txt`, `srv/made.txt`); an empty-digest match is no leak and a copy of `alpha` still is | hand-built trees; the sha256 of the empty string as a literal | stubs collected again: `left: ["a.txt", "empty_output.txt", "srv/made.txt", "srv/sub/two.txt"]`; empty match counted: `assertion failed: input_leak(&evidence(EMPTY), &inputs(EMPTY)).is_none()`; both together: `an empty file: a run that copies nothing is not a leak` left `input_leak` |

Rig: the orchestrator over the job rig of `support::job_orchestrate`, with a recording isolation that builds the real `docker run` argv (`run_argv`) and records the workspace, and a collecting harness. No Docker daemon is involved.


## 4. Findings

| # | kind | finding |
|:--|:-----|:--------|
| F1 | spec defect, fixed in fix round 1 | An empty input file made every run `invalid(input_leak)`: the mount point of an input file is an empty file in the workspace, it was collected as an output, and the leak check compared digests only. Fixed as ruled: `runner/exec/gather.rs::collect_outputs` (the output collector, called from `exec/mod.rs`) leaves out every path an input is mounted at, and `runner/evidence/leak.rs` ignores a match on the empty digest. The interim refusal of an empty input file is removed, so trees with empty `__init__.py` are declarable. |
| F2 | change outside OWNED | `run()` in `commands/run.rs` gains the lines that resolve the inputs before the resolver runs and set `request.inputs` (`with_declared_inputs`), because the image step reads the setting during resolution and `plan_job` runs after it. `plan_job` takes the resolved inputs as a new parameter. The `request_settings` signature is unchanged, so the sibling `treatment_source` hunk is separate. |
| F3 | change outside OWNED | One line, `evaluation_inputs: Vec::new(),`, in the `JobSpec` literals of `tests/support/job_orchestrate.rs`, `tests/support/job_runtime.rs` and `candler/tests/guard_cli_kill_live.rs`: the new field makes each literal fail to compile. |
| F4 | accepted | `JobSpec`, and so `job.json`, holds the evaluator list `{kind, digest}`: digests, never content, and the job dir is never mounted. I2 now also looks for the list digest and the file digests in the argv, every mount, the workspace (content and file names) and `tree.json`, and asserts that no mount source lies under the jobs directory. |
| F5 | note | `guard_resolve_surface` needs no `field_paths.toml` row for `request.inputs`: its path-class test passes with `request.inputs` added to `is_setting`. Nothing to report to `treatment_source`. |
| F6 | note | The wire refuses the reserved kind and a bad `at` before the resolver sees them, and `with_inputs` refuses a duplicate kind, so those three I3 classes are reachable only through the wire. The resolver checks a duplicate kind anyway (a slice built in code). I did not add dead checks for the other two, and no test duplicates the wire's guards. |
| F7 | addition to the I3 table | Beyond the leaf's list the resolver refuses: an empty, absolute or `..` path, a symlink as the input itself, a non-regular file, a non-UTF-8 name, and two agent files mounted at one workspace path or one under another's. |
| F8 | names | The input resolver is `candler_core::experiment::inputs::resolve_inputs`, returning `ResolvedInputs {agent, mounts, evaluators}`. The setting constant is `paths::REQUEST_INPUTS`. `CellRun::with_evaluation_inputs` carries the evaluator list so `CellRun::new` keeps its signature. |
| F9 | process | Several of my cargo calls ran without the target-dir export (the shell does not persist) and wrote to `<worktree>/target`; that build directory and the SPAWN `cargo clean -p` ran there. I removed `<worktree>/target` (3.4 GiB). The gates after that used `/private/tmp/claude-501/m2c-target-declared_inputs`. |


## P2

Question: does docker take one read-only bind mount per file (40 files in nested directories), with the parent workspace directories still writable, under the run container's flags (`--init --read-only`, tmpfs `/tmp`, `--cap-drop ALL`, `no-new-privileges`, the workspace as a read-write bind mount at `/work`)?

Evidence (Docker Desktop on macOS, `ubuntu:24.04`, 40 files in 5 x 3 nested directories, 40 `type=bind,readonly` mounts under a `/work` bind mount):

| trial | result |
|:------|:-------|
| mount points not made in the host workspace | the daemon refuses at container creation: `create mountpoint for /work/d0/s0/f15.txt mount: mountpoint ... is outside of rootfs` (one file is enough) |
| an empty file made at each mount point in the host workspace first | the container starts; 40 of 40 files read with their content |
| append to a mounted file | `Read-only file system` |
| move a mounted file | `Device or resource busy` (the mount point cannot be renamed away) |
| new file beside a mounted one, new directory with a file in it | both succeed |
| host side after the run | source files unchanged; the workspace holds the 40 empty stubs plus the new file |

Decision: one read-only file mount per file, with the mount point made first by the host user as an empty file (and its parent directories) in the workspace. This is what `mount_inputs` already does, so its form stays; the stubs are required, not a convenience. A materialised read-only directory mount is not needed.

Containers created by the probe, all removed by exact id: `23855e7ace73` (failed create, removed), `4965ab8299e0`, `1fcf09540b76` (created, never started), `4317593f1dd7`. The throwaway tree was removed.

## Fix round 2

Two should-fix items from the verifier, no blocker.

1. **Unmountable names are refused in the resolver (I3).** `resolve_inputs` refuses a comma, line break or NUL in a file or directory name of an input (checked in `walk` and `walk_dir`, so the file name of a file input too) and in `at`, naming the kind and the path, before anything is stored. `MountSpec::to_arg` keeps its check as the last line of defence. New I3 rows: `a,b.txt` in a directory input, a comma in a directory name (`d,ir`), a file name with a newline, the file name of a file input with a comma, `at = "x,y"`, and an `at` with a newline. Bite: the check made constant false; `comma in a file name of a directory: ResolvedInputs { agent: [... kind "commas" ...] mounts: [... path "a,b.txt" ...] }` (expect_err on an Ok). NUL cannot be made as a file name, so only `at` could carry it and no row tests it.
2. **The stub exclusion is pinned to exact paths.** `declared_input_mount_points_are_not_outputs` now also holds `a.txt.bak`, `srv/sub/two.txt2` and `srv/sub/two.txtd/f` (a file under a directory named like a mount point) and expects them kept. Bite: the exclusion changed to `starts_with`; `left: ["empty_output.txt", "srv/made.txt"]` against the five expected paths.

Commits: the resolver check and the I3 rows are a fixup of the step 2 commit, the output rows a fixup of the step 3 commit, and this section a fixup of the report. Gates after the round: see the message that carries it.

