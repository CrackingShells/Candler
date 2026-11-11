# Session controls

**Goal**: Before a session cell runs on a harness version, Candler checks that the instrument works (decision D4): control C1 (must wait) and control C2 (must stop) run the real harness in the real image against the scripted fake model, a failure refuses session mode for that harness version, and no model's runs are ever refused for the model's behaviour.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] OWNED: `crates/candler-core/src/resolve/steps/controls.rs` (new) and its registration in `resolve/steps/mod.rs`, the control record's reader and writer, `crates/candler-core/src/runner/evidence/**` (`session.control`); new tests. The CI entries (`.github/workflows/*`, `.config/nextest.toml`) are the coordinator's
**Success Gates**:
- ⬜ [cmd] Q1: a harness version whose control fails is refused for session mode, naming the harness version and the control; a passing record keyed by (base image id, control version) is reused without a re-run; a changed control version re-runs; headless cells never run or read a control: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^controls_/)'`
- ⬜ [cmd] A model's outcome never refuses: no code path from a model's run or a model's tool list reaches the control verdict: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^controls_never_model_/)'`
- ⬜ [cmd] Every session run records the control record it relied on: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^controls_recorded_/)'`
- ⬜ [cmd] The real controls pass on the pinned Claude Code (container e2e): `cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'test(/^controls_e2e_/)'` with Docker up, and in CI's `container-e2e` job once the coordinator lists the test
- ⬜ [cmd] SPAWN, STD (`-p candler-core`), LINUX, DIGEST, GOLDEN and SUBJ pass
- ⬜ [nl] The leaf report records Q1's bites: a refusal keyed on the model instead of the harness version, a record reused across a changed control version
- ⬜ [nl] The `session_controls` verifier closes with no open finding: it attacks whether any model behaviour can refuse, whether a broken harness (notifications never delivered, stdin EOF ignored) is caught, and whether the control's own fixed `idle_s` can mask a close-rule defect
**References**: [R01 §Session runner](../../../../../../__design__/architecture.md); [R09 D4](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-approved-plan.md); `assets/docker/fakemodel/` — scripts `c1` and `c2`

## Step 1: Refuse session mode on a broken harness
**Goal**: A preflight step looks up or produces the control record of the cell's base image and refuses on failure.
**Implementation Logic**:
1. Tests first (`controls_*`, `controls_never_model_*`, `controls_recorded_*`) with a fake control runner, seen red.
2. `SessionControlsStep` runs for session cells only. The record lives at `<store>/controls/<base-image-hex>-<control-version>.json`; the control version is a constant the step owns, bumped whenever the scripts or the control's `idle_s` change.
3. Without a record, it runs C1 then C2: a session run of the real harness in the cell's base image, the gateway sidecar's upstream pointed at the fake model with its CA, the control's own fixed short `idle_s`. C1 passes on two turns and a close; C2 on one turn and a close about `idle_s` after it. The result is written once and never rewritten (I11).
4. A failure refuses the cell, naming the harness version and the failed control. A run records `session.control` (the record's path and digest).
**Deliverables**: `crates/candler-core/src/resolve/steps/controls.rs`: `SessionControlsStep`, `ControlRecord`, `CONTROL_VERSION`; `crates/candler-core/tests/guard_controls.rs`: `controls_*`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^controls_/)'` (expected: PASS)
**Commit**: `feat(runner): refuse session mode on a harness whose controls fail`

## Step 2: Run the real controls
**Goal**: The controls pass end to end on the pinned harness.
**Implementation Logic**:
A container-e2e test runs `SessionControlsStep` for real against `assets/docker/base` and the fake model, asserts both controls pass and the record is written, and removes only what it created. Report the test names to the coordinator for CI's expected-guard list.
**Deliverables**: `crates/candler-core/tests/e2e/controls_e2e.rs`: `controls_e2e_pinned_harness_passes`
**Consistency Checks**: `cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'test(/^controls_e2e_/)'` (expected: PASS)
**Commit**: `test(runner): run the session controls on the pinned harness`
