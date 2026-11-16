# Ground-truth e2e

**Goal**: A real run through `candler run` shows that an evaluator-only input's canary is absent from the agent's container at every checkpoint, and from every surface while the run is live (I8).
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] OWNED: a new e2e file under `crates/candler/tests/it/` registered in `guard_cli_e2e`, its fixtures, and a seeker mode added to `assets/docker/testagent/` (`container_create` has merged, so no sibling edits it); no `src/` file. A defect found in `src/` is reported, not fixed here
**Success Gates**:
- ⬜ [cmd] The e2e passes with Docker up: `cargo nextest run -p candler --no-tests=fail --run-ignored only -E 'test(/^guard_cli_e2e::.*i8_/)'`
- ⬜ [cmd] The run's container and images are removed by id: `docker images -q` and `docker ps -aq` before and after are equal, as recorded in the report
- ⬜ [cmd] SPAWN, STD (`-p candler`), LINUX, E2E and SUBJ pass
- ⬜ [nl] The leaf report records each checkpoint's observation and the three bites seen red: the audience filter flipped in `mount_inputs`, the input added to the environment, and the D9 step disabled with the canary planted in a plug-in input
- ⬜ [nl] The adversarial verifier closed with no open finding. It attacks: mounts (the store root, HOME placement, the collect container, the blob directory); the environment (the driver overlay, state env, secrets); logs (gateway, docker, raw frames); the driver's files (skills, base context, MCP config, the plug-in directory); images (build and plug-in contexts); the egress proxy; and D9's walk
**References**: [R01 I8](../../../../../../__design__/architecture.md); [R06 the attack list](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md); the existing seams `crates/candler/tests/it/guard_input_seam.rs`, `crates/candler-core/tests/it/guard_declared_inputs.rs`

## Step 1: Seek the canary at every checkpoint of a real run
**Goal**: One real-image e2e looks for the canary from outside and inside the container, before, during and after a run.
**Implementation Logic**:
1. Verify first how the cli e2e rig runs (`guard_cli_e2e.rs`, `support/cli_run.rs`): which harness stub, which model, whether the real gateway runs. Use the fake model and the real gateway if the rig can, and report what it could not use.
2. The experiment has two inputs carrying canary bytes and a canary file name:
   - an evaluator-only input;
   - a plug-in environment (values and agent inputs only, so the testagent base serves) whose agent inputs hold an ordinary file.
3. The seeker mode of the testagent reports, at start, after its first turn and at its end before destroy:
   - every readable file outside `/proc` and `/sys` that matches the canary by name or bytes;
   - every `/proc/*/environ`;
   - the mount table.
4. The other checkpoints:
   - from the host, after create and before start: the container's inspect (mounts, env, labels) and the image history;
   - while the run is live: `candler ps`, `show` and `inspect`;
   - after the run: gateway logs, the raw frames and the collect container's mounts.
5. Expected at every checkpoint: no match. Oracle: the canary as written in the fixture.
6. Bites, each seen red and recorded:
   - the audience filter flipped in `mount_inputs`;
   - the input added to the agent's environment;
   - the D9 step disabled, with the canary planted in the plug-in's agent input.
**Deliverables**: `crates/candler/tests/it/guard_cli_e2e/i8_e2e.rs` (or the module form `guard_cli_e2e` uses): `i8_canary_never_reaches_the_agent`; the seeker mode in `assets/docker/testagent/testagent.sh`; its fixture experiment and plug-in
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail --run-ignored only -E 'test(/^guard_cli_e2e::.*i8_/)'` (expected: PASS)
**Commit**: `test(isolation): seek ground truth in a real agent container`
