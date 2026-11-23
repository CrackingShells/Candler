# s1_seal_blobs: leaf report v0

The seal now stores the blob of every file entry of `tree.json` before it writes the manifest, so `candler evaluate` grades the bytes the agent wrote (S1). Native and session runs both seal through `drive`, so both get the fix.

## First reads

- `runner/exec/seal.rs` wrote the manifest, tree, raw streams, trace and `run.done`, but no blobs. `evaluate` (assemble/materialise) opens `blobs/<hex>`, so the agent's files could not be found.
- Tree paths are relative to the workspace root, and the evaluation reads them under `outputs/`. A plug-in's artifact glob must therefore start with `outputs/`: the workspace file `answer.txt` is matched by `outputs/answer.txt`.
- `RunWriter::create` already writes `manifest.json`, so a seal stopped at the blob step leaves that file and nothing more.
- On the `feat/diet` tip, the new guard `a_run_that_writes_a_file_is_graded_from_its_stored_bytes` failed with `open a blob failed on .../store/blobs/2b4bea32...: No such file or directory (os error 2)`.
- K2 outside Candler: a read-only colgrep check of the HTN consumer (`htn-creator-workspace/candler-plugin/`, `candler-experiments/`, `candler-plugin.toml`) found no reader of `runs/<key>/workspace/`. Its only `workspace` code is `kinds/_files.py outputs_dir`, which reads the evaluation context's `outputs/`, not the run directory.
- K2 inside Candler: no production code reads the run's `workspace/` after the seal. Two guards do, and both are outside this leaf's OWNED files:
  - `guard_declared_inputs::evaluator_only_input_never_reaches_the_agent` scans that directory.
  - `guard_session::session_hooks_leave_the_run_as_it_is` lists it.
  Removing the directory turned both red, so it is kept (commit `7ede556d`) and the R01 row still names the kept `workspace/`. The blobs are stored either way.

## Bites

| Bite | Red seen | Reverted |
|---|---|---|
| Skip the blob step (`tree.entries().iter().take(0)`) | the CLI guard failed with "open a blob failed" | yes |
| Store the blob without the tree's digest (`put_blob_from_path(path, None)`) | b30 failed with "a changed output fails the seal: Ok(())" | yes |

## Verifier

PASS with findings, no blocker (spec-verifier). F1: the `# Errors` doc of `seal_run_observed` omits `DigestMismatch`. F2: no seal-level test checks the blobs of a session run; coverage rests on the shared `drive` path. F3: real Docker with root-owned agent files was not exercised. All three stay as follow-ups.

## Follow-ups

- K2 is undecided. To remove `workspace/` after the seal, those two guards must first check the blobs or the tree instead of the directory. That change belongs to a leaf that owns `guard_declared_inputs.rs` and `guard_session.rs`.
- The leaf's step 1 commit subject is 73 characters, one over the hook's limit of 72. It was committed as `test(runner): guard that a written output is graded from stored bytes`.
- Only the fake Docker client was exercised. A run against real Docker should confirm that root-owned files the agent writes can be read for the blob step.
