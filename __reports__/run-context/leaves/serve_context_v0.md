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

`materialise.rs::copy_stored(root, &AssembledFile) -> (abs path, digest)`: create-new, `0600`, digest check, then `0444` or `0555`. Its one call site is the loop over `Assembled::files()` in `materialise`; artifacts, input files, trace, each raw stream and the manifest are entries of that list. `assemble.rs` builds the list: artifacts from the tree, input files from the list blob, and `stored_file` for the trace, each raw stream and the manifest, which hashes the file once (the store records none for them) and the copy is checked against that hash.

## The needs union

`context::union_needs` folds `trace`, `raw` and `manifest` as flags (any kind sets it) and the four lists as before; `engine/run.rs::send` calls it. The serving guard builds its needs from one kind per part through it. `registry/needs.rs::check` meets all three parts by construction, so its doc names them.

## Ruling

`__reports__/run-context/01-ruling_trace_served_v0.md` replaces the M2a ruling "trace not served". `assemble.rs`'s doc states what is served and refused.

## What is left

- The fresh Sonnet verifier of Step 3 is the coordinator's.
- `container-e2e` runs on the pull request, not here.
- A raw directory entry that is not `<stream>.jsonl.gz` is skipped, not refused; a sealed run holds none.

## Checks on the tip

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass. `cargo nextest run --workspace --no-fail-fast`: 737 run, 732 passed. The known failure `guard_harness_facts::harness_census_equals_cf3de30_on_every_fixture` (no baseline for `streams/shell_exit_stderr_timeout`, fixed on its own branch) failed. Four more failed while the machine ran at a load average near 40 (`guard_opencode_driver::opencode_skills_land_in_the_project_skill_directory`, `guard_container_stop::stop_asks_the_daemon_and_falls_back_to_the_client`, `guard_claude_code_driver::b22_skill_tree_is_the_tag`, `guard_container_record::stopped_run_has_its_agent_recorded_before_the_hook_sweeps`); run alone, all four pass.
