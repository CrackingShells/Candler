# Leaf report: ground_truth_refusal (v0, findings)

Branch `m5b/ground_truth_refusal`, base 3be9fd3. Work commit 62275435 `feat(isolation): refuse ground truth in what the agent receives`.

## 1. Commits
- 62275435 feat(isolation): refuse ground truth in what the agent receives
- f36c6c34 reports(isolation): record the ground_truth_refusal leaf (tip)

## 2. Gates
- Refusal rows (`guard_ground_truth_refusal::`): PASS, 2 passed.
- `guard_input_seam::` + `contract_pages`: PASS, 8 passed. `grep -q isolation.ground_truth_exposed` on `codes.toml` and `index.md`: PASS.
- STD (`-p candler-core -p candler`): fmt, clippy `-D warnings`, nextest (428 passed), doc `-D warnings`: PASS.
- LINUX clippy: PASS. SUBJ (`cog check`, 72 chars): PASS. SPAWN (`guard_signal`): PASS, 9 passed.
- GOLDEN: PASS (`guard_golden` 2 passed, `guard_oracle_ac` 1 passed). DIGEST: PASS (no vector diff; `guard_cell_digest` 9, import/oracle/engine/seams set 45, `guard_harness_facts` 10). `argv.json` untouched.
- E2E: not required (no container path touched). OWNED check: see finding F1.

## 3. Guards (fix round 1 adds the last three rows)
- `guard_ground_truth_cli::run_refuses_a_copy_and_accepts_a_tree_without_one` (candler crate, the binary with the PATH-fake docker). Guards: R01 I8 / D9, the composed `candler run` resolver refuses a copy in an agent input and accepts a tree without one. Oracle: hand-written canary literal and the input seam's trees. Bite: drop the `register_ground_truth` call -> "the launcher refuses" assertion fails (the launch succeeded). Bite: skip the agent-input comparison -> same failure.
- `passes_when_only_empty_files_match` (core). Guards: empty files on both sides are no copy. Oracle: `printf '' | shasum -a 256`. Bite: remove the empty-digest filter -> this test, `guard_input_seam` and the CLI guard all fail (the refusal is returned for an empty file).
- `refuses_ground_truth_in_every_agent_bound_place` (5 rows: experiment context, experiment Dockerfile, plug-in context, plug-in agent input, treatment tree).
  Guards: R01 I8 — evaluator-only bytes in agent-bound bytes are refused with the code, naming input and place, even renamed.
  Oracle: sha-256 of the canary and of the pinned skill's `SKILL.md`, worked with `shasum -a 256`.
  Bites (WIP commit, restore from a copy), failing messages seen: plug-in inputs skipped -> `plug-in input: ()` (expect_err got Ok); treatment skipped -> `treatment tree: ()`; context skipped -> `experiment context: ()`.
- `passes_without_a_copy_and_reads_nothing_without_ground_truth`.
  Guards: R01 I8 / D9 — no copy resolves; no evaluator-only input means no read (the context directory is deleted and a walk would fail).
  Oracle: hand-written. Bite: early return `if truth.is_empty()` replaced by `if false` -> the test panics at the second resolve (cannot read the build inputs).
- Guards written after the step in the same sitting; red was seen through the bites, not before the code (deviation from "see red first").
- The one-`if` guard against reading the evaluator-only input into agent-bound bytes: none kept, as the leaf says.

## 4. Probes
None owned.

## 5. Claims checked
- "Re-use the build-context walk; never walk twice": `BuildInputs::read` is public and reused (one walk, per resolve). But the image step walks the same context in its own step, and steps share no result, so a resolve with an evaluator-only input and a context walks it twice. Only that case; no evaluator-only input, no walk.
- "Reuses the digests the walks already compute": true for the context (same code) and the treatment (`SkillTree::files`); the plug-in agent inputs have no shared walk (`resolve_input_groups` needs a store and stores blobs), so the step hashes them itself.
- "Treatment tree read as `treatment:` sources / `SkillTree`": the tree is read at run time only (`runner/exec/mod.rs` `skill_files`); the step runs the same `git archive` through the Supervisor.

## 6. Findings (after fix round 1)
- **F1 (resolved by ruling): wired.** `ResolvedInputs.files` (new `InputFiles`, from `experiment/inputs.rs`) returns every input's `{path, sha256}` entries; `run.rs` builds `GroundTruth::from_inputs` and passes it through `foreground` to `compose_resolver` (`commands/preflight.rs`), which registers the step after `register_m2b_steps`, `skills()` from the driver. `commands/resolve.rs` passes an empty `GroundTruth` (that command declares no input). No change to `candler-schemas`, no wire field, no stored digest moved.
- **F2 (accepted):** `resolve/preflight.rs` variant and branch; `guard_refusal_codes.rs` row.
- **F3 (done):** the experiment's own agent inputs are a sixth place, "`<file>` in agent input `<kind>`", compared by the digests the input walk already stored (no second walk). The plug-in environment's agent inputs stay walked by the step, so no place is reported twice.
- **Zero-length files:** a file with no byte never matches, on both sides (`GroundTruth::named` skips the empty digest). The input seam's own trees hold an empty file on each side; without the skip `guard_input_seam` is refused.
- **Residuals, each with the condition that would make it worth building (for design_record Future features):**
  - Re-encoded, compressed or otherwise transformed copies are not found (digests of identical bytes only). Build a content-aware check when a real experiment ships a derived copy of its truth (for instance a snapshot in another format) that a run is shown to read.
  - Symlinks are never matched (a context symlink is listed by its target text; plug-in input symlinks are skipped). Build it when a context is allowed to follow links out of the project.
  - A resolve with an evaluator-only input walks the build context twice (the image step and this step). Not shared: steps share no result and the walk is in `image.rs`, which this leaf does not own. Build a shared walk if a context large enough to cost seconds shows in a run.
  - The `candler run` guard covers the experiment's own agent input; the plug-in environment's agent input and context rows are guarded at the resolver (core), where the same composed step runs.

## Newly refused input
- (a) Refused: an experiment whose evaluator-only input has a non-empty file with the same sha-256 as a file in any of: the Dockerfile or build context (the experiment's or a plug-in environment's); an agent input of the experiment; an agent input of the selected plug-in environment; the treatment's skill tree at its pinned commit. The step runs at resolve preflight (after the identity and plug-in environment steps) and only when an evaluator-only input exists. Empty files never match.
- (b) Code `isolation.ground_truth_exposed` (side experiment). Message: "evaluator-only input `<kind>` (<path>) has the bytes of <place>: the agent would read the ground truth; remove the copy from what the agent receives". The CLI prints "candler: refused: <message>" (not the code).
- (c) Example, observed through `candler run`: the input-seam experiment (agent input `servers`, evaluator-only input `truth`) with `inputs/servers/copy.bin` holding the bytes of `truth/answer.txt` is refused: "candler: refused: evaluator-only input `truth` (answer.txt) has the bytes of `copy.bin` in agent input `servers`: the agent would read the ground truth; remove the copy from what the agent receives".
- (d) Evidence that no existing experiment is refused: `guard_input_seam::` (evaluator-only and agent trees with empty files on both sides) passes; `guard_ground_truth_cli::run_refuses_a_copy_and_accepts_a_tree_without_one` launches that experiment through the composed `candler run` resolver without a copy (accepted) and with one (refused); GOLDEN, DIGEST pass; with no evaluator-only input the step reads nothing.
- Ruling: Accepted by the coordination session 2026-11-16 with the zero-length skip; wiring (run.rs, preflight.rs, store helper) and the sixth place (experiment agent inputs) stand as OWNED extensions.

## 7. Docker
No Docker use. Images, containers, networks unchanged. No e2e test added.
Target dir left at /private/tmp/claude-501/m5b-target-ground_truth_refusal.

## Fix round 1 gates
All at the pre-squash tree, same content as the tip: contract pages + input seam 8 passed; fmt, clippy workspace, LINUX clippy clean; nextest `-p candler-core -p candler` 428 passed; doc clean; GOLDEN 2 + 1 passed; DIGEST (base e05c508) no vector diff, `guard_cell_digest` 9, import/oracle/engine/seams/harness_facts 55; SUBJ and `cog check` PASS; SPAWN left to the coordinator. OWNED diff: the paths in the ruling plus `commands/resolve.rs` (one argument, an empty `GroundTruth`) and `experiment/inputs.rs` (the store-read helper).
