# Leaf report: evaluator_inputs (findings)

Branch `m2c/evaluator_inputs`, base 3307ca8. One implementation commit, then this report.

## 1. Commits
| sha | subject |
|:----|:--------|
| (after autosquash) | feat(evaluation): serve an evaluator's input needs from the run (includes the `guard_registry.rs` change of F1) |
| (after autosquash) | reports(evaluation): record the evaluator_inputs leaf |

## 2. Gates
| gate | result | last line |
|:-----|:-------|:----------|
| EV1 | PASS | `2 tests run: 2 passed, 6 skipped` (filter `test(/needs_inputs/)`) |
| EV1 oracle `gen_context_inputs.py --check` | PASS | `context_inputs.toml: up to date` |
| EV2 | PASS | `1 test run: 1 passed, 7 skipped` |
| the four context binaries (`guard_run_context`, `guard_engine`, `guard_registry`, `guard_seams`) | PASS | after fix round 1 (F1) |
| STD (`-p candler-core`): fmt, clippy `-D warnings`, doc `-D warnings` | PASS | clean |
| STD nextest `-p candler-core` | PASS | all pass, with `nuisance_set_is_the_hand_written_one` excluded (red on the base, fixed on the milestone) |
| LINUX (`-p candler-core --tests`) | PASS | `Finished` |
| SUBJ | PASS | `No errored commits`; subjects at most 72 characters |
| G5a | PASS | numstat shows 0 deletions; both vector files are additions |
| OWNED | PASS | the diff lists only `context/{assemble,materialise,mod}.rs`, `registry/{mod,needs}.rs`, `guard_run_context.rs`, `gen_context_inputs.py`, `context_inputs.toml` |
| SPAWN | PASS | `guard_signal`: 9 passed |

`mod.rs` of `context/` and of `registry/` are inside the OWNED globs (`context/*`; `registry/mod.rs` is the doc line, see F3).

## 3. Guards
**EV1 `needs_inputs_are_served_from_both_manifest_lists`** (guard_run_context)
- Guards: R01 Evaluator protocol and I8. A need for `servers` (in `comparable.inputs`) and `truth` (in `evaluation_inputs`) gives one `{kind, path, digest}` per kind; each path is a tree of the blobs' bytes that cannot be written; the `context_digest` covers `{kind, digest}` of exactly what was served.
- Three vectors: both kinds, the evaluator-only kind alone, the declared kind alone. A `build_inputs` entry sits in every manifest and is never served.
- Oracle: `context_inputs.toml` from `gen_context_inputs.py` (python rfc8785, no Candler code): the list blob's canonical text, the input digest, the context digest, and the file contents as literals.
- Bites, each seen red and restored:
  - `evaluation_inputs` ignored (drop the second lookup): `the run assembles: InputRefused { kind: "truth", reason: "is not an input of the run" }`.
  - Inputs left out of the context (not added to the builder): `the kinds served: a need for both lists`, left `[]`.
  - A wrong digest in the context entry (the first file's hash in place of the list's): `digest of truth: a need for both lists`, left `sha256:fbf76...`.
  - A pure "inputs omitted from `context_digest`" cannot be injected below the schema, which owns that function and is untouched. The projection's covering of `{kind, digest}` is proved by the vector digest, which differs per need.

**EV2 `needs_inputs_refused_when_the_run_cannot_serve_them`** (guard_run_context)
- Guards: R01 Evaluator protocol. A need is refused naming the kind and the reason, and is never served empty or reported as unsupported.
- Cases: `ghost` (in neither list), `build_inputs` (reserved, even where the manifest holds it), `imported` (a digest in `comparable.inputs` with no list blob).
- Oracle: hand-written kinds and reasons.
- Bite: a missing kind dropped silently (an empty list) gives `a need the run cannot meet is refused: Assembled { ... inputs: [] ... }`.

**`needs_inputs_refused_for_the_reserved_kind`** (unit, `registry/needs.rs`): the registry refuses `build_inputs` before any run is read. Oracle: hand-written. Not bitten separately; the same refusal sits in `assemble` and is bitten by EV2.

## 4. Probe answers
None.

## 5. Findings
- **F1 (spec defect, fixed in round 1).** `guard_registry.rs` pinned `unsupported in this version: inputs` for an `inputs` need. OWNED was widened to that file: the `("inputs", ...)` row now expects the reserved-kind refusal of a `build_inputs` need at suite check (`input kind \`build_inputs\` is reserved for the image and cannot be needed`), and the doc says so. Oracle: the hand-written message. Bite: the registry accepting `build_inputs` turns the row red with `inputs: CheckedSuite { ... inputs: [Name { text: "build_inputs" }] ... }` from `expect_err("inputs")`. The older roadmap and report lines are left as history.
- **F2 (design choice, flag): the registry cannot check an input need "against the run".** `check(needs)` runs at suite time with no run. It refuses only the reserved kind there. Whether the run holds the kind, and whether its blobs exist, is decided in `assemble`, per run. The refusal reaches the engine as a batch fault carrying the text. Step 3's "checked against the run" is met there.
- **F3 (deviation, small):** `registry/mod.rs` is not named by the leaf's OWNED line (`registry/needs.rs` only). I edited one module doc line there that said inputs are unsupported. Revert it if that is unwanted; the doc would then be stale.
- **F4 (accepted): directories of a materialised input tree stay writable; its files are 0444**, as in `outputs/`. A read-only directory would make the engine's `remove_dir_all` of the scratch area fail. This cannot expose the store: every file is a fresh copy (`create_new`, a new inode), never a hard or symbolic link to a blob, so a plug-in that makes a file writable, or replaces it in a writable directory, changes only the scratch copy, which is hash-checked on writing and deleted with the request. The blobs keep their own 0444 inodes.
- **F5 (names):** new public items `AssembledInput` and `ContextError::InputRefused {kind, reason}`, and `NeedError::ReservedInput {kind}`. No shared-constraint name differs. The input-blob layout in the campaign README was unambiguous (blob key, list blob bytes are the RFC 8785 form, `path` relative); the reader verifies `input_list_digest(entries) == digest` with the schemas function, so a non-canonical list blob is refused.
- **F6 (test helper):** the fabricated manifests recompute the cell digest from a probe manifest before sealing, because `comparable.inputs` is hashed. They use the store API and manifest JSON only.
- Nothing in `candler-schemas` was needed. No confirms are left.
