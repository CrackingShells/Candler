# Input seam

**Goal**: Through the binary, a run that declares an agent input and an evaluator-only input, then an evaluation whose plug-in needs both kinds, shows two things: the plug-in reads both trees with matching digests, and the agent's side never held the evaluator-only bytes (I8 end to end).
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone after L1: `git merge-base --is-ancestor feat/m2c-wire-alignment HEAD && git log --first-parent --format=%s feat/m2c-wire-alignment | grep -qE '^merge\(.*\): land evaluator_inputs$'`
- [ ] [nl] OWNED: `crates/candler/tests/guard_input_seam.rs` (new), a new fixture plug-in under `crates/candler/tests/fixtures/plugins/` (it uses the Python helper through local `uv`, as M2a's test plug-ins do), and a new support module. No `src/` change: a seam defect is reported to the coordinator, who sends it back to the owning L1 leaf
**Success Gates**:
- ⬜ [cmd] S1: `cargo nextest run -p candler --no-tests=fail -E 'binary(guard_input_seam)'`.
  - `candler run` uses the fake docker and harness of `tests/support/cli_run.rs` and an experiment with `[[inputs]]` of kind `servers` (agent) and of kind `truth` (evaluators). The truth file holds a canary string.
  - `candler evaluate` then runs the fixture plug-in, whose `needs.inputs` is `["servers", "truth"]`. The plug-in writes into its outcome's evidence the sha256 of every file it read under each context `path`.
  - Those hashes equal the files' sha256, and each context `digest` equals W3's list digest of its tree.
  - The canary string occurs nowhere under the run dir, the job dir or the recorded docker argv.
  Oracle: the files' sha256 by `shasum`, the canary, and W3's vectors
- ⬜ [cmd] STD (`-p candler`), LINUX, SPAWN and SUBJ pass
- ⬜ [nl] The leaf report records S1's bite: the evaluator-only input routed to the mounts, which turns the canary assertion red; or the list blob stored under another key, which turns the plug-in's read red
- ⬜ [nl] The `input_seam` verifier closes with no open finding. It attacks the seam between `declared_inputs`' writer and `evaluator_inputs`' reader: the blob keys, the list format and the read-only materialisation; and I8, by looking for the canary where the guard does not
**References**: [R10 guard S1, D2–D3](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md); [R01 §Evaluator protocol, §Invariants I8](../../../../__design__/architecture.md); the `declared_inputs` and `evaluator_inputs` leaf reports

## Step 1: The seam guard
**Goal**: One end-to-end guard through the binary, seen red, then green.
**Implementation Logic**:
1. Build the fixture plug-in.
   - Its manifest and `describe` declare one kind whose `needs.inputs` is `["servers", "truth"]`.
   - Its `evaluate` hashes every file under each context input's `path` and reports one outcome whose evidence carries the hashes.
   - It names no domain.
2. Write the guard with the fake docker and harness from `tests/support/cli_run.rs`. Do not edit that file: add a support module, or call it as it is.
3. See S1 red with one of the bites, then green.
**Deliverables**: `crates/candler/tests/guard_input_seam.rs`: S1; `crates/candler/tests/fixtures/plugins/needs-inputs/` (the fixture plug-in)
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'binary(guard_input_seam)'` (expected: PASS)
**Commit**: `test(experiment): guard the input seam from the run to the evaluator`
