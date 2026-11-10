# Evaluator inputs

**Goal**: An evaluator whose `needs.inputs[]` names a kind gets `{kind, path, digest}` in its run context. `path` is a read-only tree materialised from the store, taken from `comparable.inputs` or `evaluation_inputs`. A need the run cannot meet is refused, naming the kind.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone after L0: `git merge-base --is-ancestor feat/m2c-wire-alignment HEAD && git log --first-parent --format=%s feat/m2c-wire-alignment | grep -qx 'merge(schemas): land wire'`
- [ ] [nl] OWNED: `crates/candler-core/src/evaluation/context/*`, `crates/candler-core/src/evaluation/registry/needs.rs`, `crates/candler-core/tests/guard_run_context.rs`, and a new vector generator and file under `crates/candler-core/tests/oracle/vectors/` (additions only: G5a forbids editing `gen_vectors.py` or an existing vector file). The manifests the tests read are built by the tests, with the store API and the input-blob layout of the campaign README
**Success Gates**:
- ⬜ [cmd] EV1: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_run_context) & test(/needs_inputs/)'`.
  - For a run whose manifest has kind `servers` in `comparable.inputs` and kind `truth` in `evaluation_inputs`, a need for both gives two context entries.
  - Each `path` is a materialised tree whose files' bytes match their blobs, and it is read-only.
  - The request's `context_digest` equals the second engine's value.
  Oracle: `uv run --with rfc8785 python crates/candler-core/tests/oracle/vectors/gen_context_inputs.py --check`, a new file
- ⬜ [cmd] EV2: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_run_context) & test(/needs_inputs_refused/)'`. These are refused, naming the kind and the reason: a need for a kind the run lacks; a need for kind `build_inputs`; and a kind whose digest has no list blob in the store, as on an imported run. `Unsupported { need: "inputs" }` no longer exists. Oracle: hand-written
- ⬜ [cmd] The other context guards pass: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_run_context) | binary(guard_engine) | binary(guard_registry) | binary(guard_seams)'`, and G5a holds
- ⬜ [cmd] STD (`-p candler-core`), LINUX and SUBJ pass
- ⬜ [nl] The leaf report records the bites:
  - EV1: `evaluation_inputs` ignored; inputs left out of `context_digest`;
  - EV2: a missing kind served as an empty list.
**References**: [R01 §Evaluator protocol (`needs`, the run context, the digests of a request)](../../../__design__/architecture.md); [R10 D3, guards EV1–EV2](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md); [R07 "Tier D `needs.inputs[]` kinds"](../../../__reports__/m2a-evaluation-parity/02-knowledge_transfer_v0.md); campaign README "Shared constraints" (input blobs)

## Step 1: Serve input needs from the manifest
**Goal**: Context assembly and the registry's need check serve `needs.inputs` from either manifest list, and refuse what they cannot serve.
**Implementation Logic**:
1. Write the new generator and its vectors, then EV1 and EV2, and see them red.
2. `evaluation/context/assemble.rs`:
   - for each kind in `needs.inputs`, find it in `comparable.inputs` or `evaluation_inputs`;
   - read its list blob and materialise the files under the request's scratch area, by hard links or copies as `root` does, read-only;
   - add `{kind, path, digest}` to `RunContext.inputs`.
   Remove the `Unsupported` arm for inputs; the arm for `snapshots` stays.
3. `evaluation/registry/needs.rs`: a need for inputs is checked against the run and refused with a reason naming the kind, in place of `Unsupported`.
4. The materialised tree lives only in the request's scratch area and is removed with it.
**Deliverables**: `crates/candler-core/src/evaluation/context/assemble.rs`: the input arm; `crates/candler-core/src/evaluation/registry/needs.rs`: the input check; `crates/candler-core/tests/oracle/vectors/{gen_context_inputs.py,context_inputs.toml}`; `crates/candler-core/tests/guard_run_context.rs`: EV1, EV2
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_run_context)'` (expected: PASS)
**Commit**: `feat(evaluation): serve an evaluator's input needs from the run`
