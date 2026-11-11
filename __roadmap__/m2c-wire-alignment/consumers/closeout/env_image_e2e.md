# Env image e2e

**Goal**: A real env image built from an experiment's requirements file and prune list shows three things: the pruned file is absent, the recorded package's distribution and version are read from it, and an experiment with no requirements gets an env with no third-party package.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone after L1: `git merge-base --is-ancestor feat/m2c-wire-alignment HEAD && git log --first-parent --format=%s feat/m2c-wire-alignment | grep -qE '^merge\(.*\): land declared_inputs$'`
- [ ] [cmd] Docker answers and the network reaches PyPI: `docker info >/dev/null && curl -sfI https://pypi.org/simple/ >/dev/null`
- [ ] [nl] OWNED: `crates/candler-core/tests/e2e/env_image_e2e.rs` (new), its `#[path]` line in `crates/candler-core/tests/guard_container_e2e.rs`, and a fixture directory under `crates/candler-core/tests/fixtures/env_image/`. No `src/` change: a defect is reported
**Success Gates**:
- ⬜ [cmd] X1: `cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_container_e2e) & test(/env_image/)'`.
  - With a requirements file pinning one small pure-Python package that ships a wheel, and `isolation.build.prune` naming a file that package ships: the file is absent under `/opt/env`; the package read for its import name returns its distribution and pinned version; and `uv pip list` in the container shows the package.
  - With no requirements: `uv pip list` shows only the seed packages.
  - Oracle: the pin in the fixture, and `uv pip list`
- ⬜ [cmd] The e2e removes what it built: `docker images -q --filter label=candler.lock` and `docker ps -aq` list no image or container the test created after it ends. The test removes them by exact id and records the ids in the leaf report
- ⬜ [cmd] CI runs it. The `container-e2e` workflow's filter covers the new test: `grep -n 'guard_container_e2e' .github/workflows/container-e2e.yml` names the binary, and the leaf report shows the test in that job's list, or says why the workflow's filter needs the coordinator
- ⬜ [cmd] STD (`-p candler-core`), LINUX and SUBJ pass
- ⬜ [nl] The leaf report records X1's bite: the Dockerfile's prune argument ignored, which turns the absent-file assertion red; this is applied to a copy of the Dockerfile the test points at, never to `assets/`
**References**: [R10 guard X1, D6](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md); `crates/candler-core/tests/e2e/container_run_e2e.rs` for the e2e conventions; campaign README §Gotchas, "Safety"

## Step 1: The real env image
**Goal**: The e2e builds two env images through the core's image service and inspects them.
**Implementation Logic**:
1. Pick the fixture package: small, pure Python, with a wheel on PyPI, no dependencies, and shipping a file that is safe to prune.
2. Write the module under `tests/e2e/`, gated like the others (`#[ignore]`, the `container-e2e` test group), building through the image service with the fixture's requirements and prune list. Run the package read and `uv pip list` in a control container.
3. Remove every image and container it created, by exact id.
**Deliverables**: `crates/candler-core/tests/e2e/env_image_e2e.rs`: X1; `crates/candler-core/tests/guard_container_e2e.rs`: its `#[path]` line; `crates/candler-core/tests/fixtures/env_image/`
**Consistency Checks**: `cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_container_e2e) & test(/env_image/)'` (expected: PASS)
**Commit**: `test(isolation): build a real env image from an experiment's requirements`
