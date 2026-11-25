# Leaf report: serve_context (v0)

Branch `leaf/run-context/serve_context`, cut from `feat/run-context` at `62038d1c`.

## Spec deviations

1. **The guards compile on the base through `union_needs`.** The engine's fold of a plug-in's needs was inline in `engine/run.rs::send`. The Step 1 commit moves it, unchanged and still dropping `raw` and `manifest`, into `context::union_needs`, so the serving guard declares each part through the fold the engine calls and fails on an assertion, not on a missing name. Step 2 adds `raw` and `manifest` to it.
2. **`Assembled`'s list carries two fields beyond `(source, workspace path, digest)`**: `executable` (the copy's `0555` or `0444`) and `served` (what the file is: artifact, input file, trace, raw stream, manifest), which `materialise` needs to fill the context from the list. Inputs keep a `(kind, digest)` entry, because an input with no files still has a context entry.
3. **The live vector rows record the stored files' hashes.** The corpus holds neither trace, raw frames as stored nor manifest in the form the store writes, so the generator takes the hash of each stored file from a store folder (`--store`) and records it in the row; `--check` without a store reuses the recorded hashes. The digest, the projection and the key spelling are the generator's. The guard checks each recorded hash against the file the store holds before it compares digests; the import's output was identical across two imports.
4. `docs/contract/index.md` and `__roadmap__/m3-lean-cli/refusal_scope.tsv` changed with the new code (`guard_refusal_codes` reads the tsv: `ContextError` has 11 variants).

## Guards, each seen red on the base and under its bite

| Guard | Base failing message | Bite and its failing message |
|---|---|---|
| `kinds_that_declare_the_run_parts_are_served_their_stored_bytes` | `the context serves the trace` (`context.trace()` is `None`) | (a) silent ignore of `needs.trace`: same message. (b) hard link: `assertion left == right failed: the mode of trace.jsonl, left: 420, right: 292` |
| `a_plug_in_cannot_write_through_to_the_store` (extended to trace, raw, manifest) | `chmod u+w: Os { code: 2, kind: NotFound }` (no copy of the trace in the workspace) | hard link: `the store blob of cont-train.json, left: sha256:7e2e..., right: sha256:9e79...` (the write reaches the store) |
| `the_live_context_digest_is_the_independent_vector` (three new `[[served]]` rows; the four existing rows untouched) | `with_skill/run-6 serving ["trace", "raw", "manifest"]`, left `sha256:8048...`, right `sha256:3407...` | (a) `trace` key always added: `with_skill/run-6` fails, left `sha256:b94f...`, right `sha256:8048...` (an existing row). (b) key left out when served: `with_skill/run-6 serving ["trace", "raw", "manifest"]`, left `sha256:492d...`, right `sha256:3407...` |
| `needs_parts_refused_when_the_run_cannot_serve_them` | `a part the run lacks is refused: Assembled { ... files: [] ...}` (served empty) | refusal removed (trace): `assertion left == right failed: trace, left: "store.io_failed", right: "evaluation.run_part_missing"` |
| union (gate 5): the four guards above under `union_needs` dropping `raw` | n/a | `needs_parts_refused...`, `kinds_that_declare...`, `a_plug_in_cannot...` and the live vector fail |

The hard-link bite also turns `an_imported_executable_materialises_as_a_private_0555_copy` red. Existing guards (`a_corrupt_blob_is_refused_and_not_digested`, the inputs guards, the file rule, `a_workspace_that_contains_the_store_is_refused`) pass unchanged on the tip. The generator's existing output is a byte prefix of the new file.

## Which parts a sealed run can lack

| Part | Can a sealed run lack it? | Producer that decides |
|---|---|---|
| manifest | No | `RunWriter::create` writes `manifest.json` before the run directory is renamed into place; `seal` never removes it. `assemble` reads it for the metadata and the input needs, so a missing one is a `Store` error. No guard, no refusal of its own. |
| trace | Yes | `seal_run_observed` writes it before `run.done`, but the importers do not always: `import/raw.rs` seals a run whose reader failed or whose format is unknown with an invalid manifest and no trace, and `import/assemble.rs` (legacy folders) writes none when no reader exists or normalising fails. `seal_interrupted` seals with none. |
| raw | Yes | The runner and both importers store at least one stream, but `seal_interrupted` seals a run that has none, and `seal_run_observed` stores whatever slice it is given. |

The refusal is `ContextError::PartMissing`, code `evaluation.run_part_missing` (side `plugin`, as for a missing input). The guard seals a run over a manifest alone with the existing `seal_with_inputs` helper (the store's own writer) and writes the code by hand.

## The one copy routine

`materialise.rs::copy_stored(root, &AssembledFile) -> (abs path, digest)`: create-new, `0600`, digest check, then `0444` or `0555`. Its one call site is the loop over `Assembled::files()` in `materialise`; artifacts, input files, trace, each raw stream and the manifest are entries of that list. `assemble.rs` builds the list: artifacts from the tree, input files from the list blob, and `stored_file` for the trace, each raw stream and the manifest, which hashes the file at assembly (the store records none for them); the copy is checked against that hash. Each part is read three times: hashed at assembly, copied, and hashed again as written.

## The needs union

`context::union_needs` folds `trace`, `raw` and `manifest` as flags (any kind sets it) and the four lists as before; `engine/run.rs::send` calls it. The serving guard builds its needs from one kind per part through it. `registry/needs.rs::check` meets all three parts by construction, so its doc names them.

## Ruling

`__reports__/run-context/01-ruling_trace_served_v0.md` replaces the M2a ruling "trace not served". `assemble.rs`'s doc states what is served and refused.

## What is left

- The fresh Sonnet verifier of Step 3 is the coordinator's.
- `container-e2e` runs on the pull request, not here.
- A raw directory entry that is not `<stream>.jsonl.gz` is skipped, not refused. That a sealed run holds none is not proven for an interrupted run. What holds: both producers write only `<stream>.jsonl.gz`, and a crash debris name `.tmp-...` fails `StreamName`'s pattern, so it is skipped.
- Directory modes of the workspace stay as they were (writable directories are older than this leaf). Making them read-only once the copies are written is a follow-up.
- The refusal code of a digest mismatch is still `evaluation.blob_corrupt`, though it now also covers the trace, a raw stream and the manifest; renaming a code is left to a decision of its own.

## Checks on the tip

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass. `cargo nextest run --workspace --no-fail-fast`: 737 run, 732 passed. The known failure `guard_harness_facts::harness_census_equals_cf3de30_on_every_fixture` (no baseline for `streams/shell_exit_stderr_timeout`, fixed on its own branch) failed. Four more failed while the machine ran at a load average near 40 (`guard_opencode_driver::opencode_skills_land_in_the_project_skill_directory`, `guard_container_stop::stop_asks_the_daemon_and_falls_back_to_the_client`, `guard_claude_code_driver::b22_skill_tree_is_the_tag`, `guard_container_record::stopped_run_has_its_agent_recorded_before_the_hook_sweeps`); run alone, all four pass.

## Review round

| Fix | Evidence |
|---|---|
| Mode set on the open handle in `copy_stored` before the handle is dropped; no `fs::set_permissions` by path. | `a_plug_in_cannot_write_through_to_the_store` and `an_imported_executable_materialises_as_a_private_0555_copy` pass. Directory modes left, listed above. |
| Digest mismatch names the stored source path and says "no longer has the digest recorded for it", true for a blob and for a stored part; code unchanged, description in `refusal.rs` and `docs/contract/index.md` widened. | `a_corrupt_blob_is_refused_and_not_digested` now checks the named path for a blob and for a trace changed after `assemble`; `guard_refusal_codes` passes. |
| `PartMissing { run: RunKey, part: RunPart }`, `RunPart` a `#[non_exhaustive]` enum (`Trace`, `Raw`); the text names the run. | `needs_parts_refused_when_the_run_cannot_serve_them` checks the part and the run in the text. |
| `assemble` refuses a missing trace only on `NotFound`; any other error is `ContextError::Io`, as `stored_streams` does. | Read; no guard (a non-`NotFound` error needs a file system fault). |
| `Served::Input` carries no payload; the path text is built only for the parts that the context records; `text_of` is the one conversion. `below` stays: it joins a directory and a name, which `stored_path` does not. | Compiles clean under clippy `-D warnings`. |
| Docs: hashed at assembly, copy checked against that hash; module line wrapped; the broken sentence of the vector guard mended. | `cargo doc` with `-D warnings`. |
| The corrupt-blob guard extended with a stored trace; the manifest copy's hash compared with the stored `manifest.json`; the `3` replaced by a check that the `[[served]]` rows cover trace, raw and manifest. | Red first: with the copy-versus-hash check removed for stored files (scratch branch, deleted), `a_corrupt_blob_is_refused_and_not_digested` failed with `a trace changed after assembly must be refused, naming the trace, got Ok(RunContext { ... })`. |
| Ruling note: a plug-in is served the union of what its kinds declare, and its kinds share the process. | `git grep -n -i "kind saw\|kind sees\|what a kind"` finds none in the files this leaf wrote. |
| Report: the claim about non-stream files in `raw/` replaced by what holds. | Above, "What is left". |
