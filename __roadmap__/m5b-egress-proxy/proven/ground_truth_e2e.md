# Ground-truth e2e

**Goal**: A real run through `candler run` shows that an evaluator-only input's canary is absent from the agent's container at every checkpoint, and from every surface while the run is live (I8, B7).
**Estimate**: $7.0 (Sonnet, one sitting; the adversarial verifier included)
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] L0 is merged, so D9's step exists for the third Bite: `git log --format=%s | grep -q '^merge(isolation): land ground_truth_refusal'`
- [ ] [nl] OWNED: a new `crates/candler/tests/it/guard_cli_e2e/i8_e2e.rs` with its `#[path]` module line in `crates/candler/tests/it/guard_cli_e2e.rs` (a single file today, not a directory), its fixture experiment and plug-in, and a seeker mode in `assets/docker/testagent/testagent.sh`. It edits no `src/` file: a defect found there is reported, not fixed. A Bite is a temporary local edit (for example in `mount_inputs`, `runner/exec/mod.rs`), never committed. CI's expected list gets this leaf's name through `egress_attack`
**Success Gates**:
- ✅ [cmd] The e2e passes with Docker up, under the B4 flock: `cargo nextest run -p candler --no-tests=fail --run-ignored only -E 'test(/^guard_cli_e2e::.*i8_/)'`
- ✅ [cmd] The run's container and images are removed by id: `docker images -q` and `docker ps -aq` before and after are equal, as recorded in the report
- ✅ [cmd] SPAWN, STD (`-p candler`), LINUX, E2E and SUBJ pass (README, standard gates)
- ✅ [nl] The leaf report records each checkpoint's observation and the three Bites seen red: the audience filter flipped in `mount_inputs`, the input added to the agent's environment, and the D9 step disabled with the canary planted in a plug-in agent input
- ✅ [nl] The adversarial verifier (Sonnet, a fresh agent that did not write the e2e) closed with no open finding. It attacks: mounts (the store root, HOME placement, the collect container, the blob directory); the environment (the driver overlay, state env, secrets); logs (gateway, docker, raw frames); the driver's files (skills, base context, MCP config, the plug-in directory); images (build and plug-in contexts); the egress proxy; and D9's walk
**References**: [R01 I8](../../../__design__/architecture.md); [R04 the attack list](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md); [R03 B7, I8 guard](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5b-approved-plan.md); existing seams `crates/candler/tests/it/guard_input_seam.rs` and `crates/candler-core/tests/it/guard_declared_inputs.rs`; [moved leaf](../../m5-container-isolation/client/seams/egress/proof/ground_truth_e2e.md) — source text

## Step 1: Teach the testagent to seek a canary
**Goal**: The testagent has a seeker mode that reports what the agent could read.
**Implementation Logic**:
1. Verify first how the testagent's modes are chosen in `testagent.sh`, and that the cached base serves it (B7: no env layer is built).
2. The seeker mode reports, at start, after its first turn and at its end before destroy:
   - every readable file outside `/proc` and `/sys` that matches the canary by name or bytes;
   - every `/proc/*/environ`;
   - the mount table.
3. Its output goes where the harness's trace goes, so the test reads it from the run's evidence. The canary comes in as a pattern argument, never as a value baked into the image.
**Deliverables**: `assets/docker/testagent/testagent.sh`: the seeker mode (its name and arguments documented in the script header)
**Consistency Checks**: `git grep -n 'seeker' -- assets/docker/testagent/testagent.sh && sh -n assets/docker/testagent/testagent.sh` (expected: PASS)
**Commit**: `test(isolation): add a canary seeker mode to the testagent`

## Step 2: Seek the canary at every checkpoint of a real run
**Goal**: One real-image e2e looks for the canary from outside and inside the container, before, during and after a `candler run`.
**Implementation Logic**:
1. Verify first how the cli e2e rig runs (`guard_cli_e2e.rs`, `support/cli_run.rs`): which harness stub, which model, whether the real gateway runs. Use the fake model and the real gateway if the rig can, and report what it could not use.
2. The experiment has two inputs carrying canary bytes and a canary file name: an evaluator-only input; and a plug-in environment (values and agent inputs only, so the testagent base serves) whose agent inputs hold an ordinary file.
3. Checkpoints:
   - inspect before start (one inspect of env, mounts and labels, plus the image history; a created container cannot be exec'd), then `docker exec -i -u 10001 <agent id> sh -s` with the seeker live and at the end, before the destroy (B7's premise, a testagent as the agent, is withdrawn: the agent is real Claude Code);
   - from the host, after create and before start: the container's inspect (mounts, env, labels) and the image history;
   - while the run is live: `candler ps`, `show` and `inspect`;
   - after the run: gateway logs, the raw frames and the collect container's mounts.
4. Expected at every checkpoint: no match. Oracle: the canary as written in the fixture, never read back from Candler's code.
5. Bites, each seen red and recorded: the audience filter flipped in `mount_inputs`; the input added to the agent's environment; the D9 step disabled with the canary planted in the plug-in's agent input.
6. Remove what the run created, by exact id, and compare `docker images -q` and `docker ps -aq` before and after.
**Deliverables**: `crates/candler/tests/it/guard_cli_e2e/i8_e2e.rs`: `i8_canary_never_reaches_the_agent`; `crates/candler/tests/it/guard_cli_e2e.rs`: its `#[path]` line; the fixture experiment and plug-in beside it
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail --run-ignored only -E 'test(/^guard_cli_e2e::.*i8_/)'` (expected: PASS)
**Commit**: `test(isolation): seek ground truth in a real agent container`
