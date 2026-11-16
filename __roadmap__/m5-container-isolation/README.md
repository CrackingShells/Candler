# m5-container-isolation

## Context
Milestone M5 of `candler-v1`, as cut by amendment A6. It runs off `main` at 18142ec, on
`feat/m5-container-isolation`. It makes the agent's container closed to everything but an
allowlist of hostnames and ports and the model gateway. It lets a plug-in declare the agent's
environment: build, MCP servers, inputs and egress. It proves I8 inside a real container. It builds
the Docker plumbing once (audit A5, with A1 and A8). No golden or parity cell digest may move.
Depth is ordering:
- **L0, this level:** `wire_shapes`, the one `feat(schemas)` commit plus the core's admission of
  the new paths.
- **L1, `client/`:** the one Docker client and control builder (`docker_client`), and the
  `isolation.egress` setting (`egress_setting`).
- **L2, `client/seams/`:** the destroy guard's reuse (`guard_reuse`, A1), `docker create` (A8,
  `container_create`), and the plug-in agent environment (`plugin_environment`).
- **L3, `…/egress/`:** the egress proxy (`egress_proxy`) and the I8 refusal
  (`ground_truth_refusal`).
- **L4, `…/proof/`:** the adversarial I8 real-image e2e (`ground_truth_e2e`) and the R01 record
  (`design_record`).
The coordinator runs P0, the digest checks at each level merge, and one live smoke after L3. No
leaf does these.

## Reference Documents
- [R01 Architecture](../../__design__/architecture.md) — §Process supervision, §Agent environment and secrets, §Plug-in manifest, §Contracts & Invariants (I8, I14, I22), §Future features
- [R02 Meta leaf](../candler-v1/contracts/parity/alignment/surfaces/isolation/m5_container_isolation.md) — the gates this campaign decides
- [R03 Amendment A6](../../__reports__/candler-v1-review/02-findings_amendment_a6_v0.md) — what M5 is and is not
- [R04 Interlude audit](../../__reports__/interlude-audit/00-findings_audit_v0.md) — A1, A5, A8; [results](../../__reports__/interlude-audit/01-results_v0.md) — costs, corrected claims
- [R05 M4 knowledge transfer](../../__reports__/m4-session-runner/knowledge_transfer_v0.md) — real-image guards, cold daemon, early push
- [R06 Approved plan](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md) — decisions D1–D9, the file layout of D6, the race rules, the budget
- [R07 HTN agent-runtime needs](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/htn-agent-runtime-needs-for-m5.md) — N1–N6, the live-MCP-server need
- [R08 CONTRIBUTING](../../CONTRIBUTING.md) — types, scopes, the schemas-release rule
- [R09 M4 campaign](../m4-session-runner/README.md) — the standard gate forms reused here

## Goal
An agent's container reaches only its listed hosts and the model gateway. A plug-in declares the agent's environment and the core provisions it. A real container is shown never to hold ground truth. One Docker client and one control builder serve every caller. No golden or parity cell digest moves.

## Pre-conditions
- [x] [nl] Eliott approved the plan (2026-11-16, round 3; all subagents on Sonnet; cap $60)
- [x] [cmd] The milestone branch starts at `main`: `git merge-base --is-ancestor 18142ec feat/m5-container-isolation`
- [x] [cmd] The golden corpus is in place: `test -d /Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`
- [x] [cmd] Docker Engine is 25 or newer (29.9.2, 2026-11-16): `test "$(docker version --format '{{.Server.Version}}' | cut -d. -f1)" -ge 25`
- [x] [cmd] P0 on 000f8f1 (18142ec plus the roadmap, 2026-11-16): fmt, clippy, SPAWN 9/9, nextest 655/655 (24 skipped), doc, LINUX, DIGEST 9 + 45, DRIFT 2/2, GOLDEN 2 + 1, E2E 13/13; every gate filter selects tests
- [x] [cmd] P3 (PASS: `--with-stream_ssl_preread_module`): the gateway base has the stream SNI module: `docker run --rm --entrypoint nginx "$(sed -n 's/^FROM //p' assets/docker/gateway/Dockerfile | head -1)" -V 2>&1 | grep -q ssl_preread`
- [x] [cmd] P1 (here: SERVFAIL, exit 1, so no external name resolves; `--dns` not needed locally): whether an `--internal` network resolves external names here, from `docker network create --internal <tmp>` and `docker run --rm --network <tmp> <base> getent hosts example.com`, network removed by name after. The exit status is recorded here and decides D2's `--dns`; CI's answer comes from the first push.

## Success Gates
- ⬜ [cmd] G1 egress: `egress_proxy`'s adversarial e2e passes locally and in CI's `container-e2e` job: a listed host and port is reachable, everything else fails, and the model gateway still works
- ⬜ [cmd] G2 plug-in environment: `guard_container_e2e::plugin_environment_serves_live_mcp_tools` passes; no hand-edited sandbox contract is needed
- ⬜ [cmd] G3 I8: `guard_cli_e2e::i8_*` and `ground_truth_refusal`'s table pass
- ⬜ [cmd] G4 one Docker client: `test -z "$(git grep -l 'SpawnRequest::builder' -- crates/candler-core/src/isolation crates/candler-core/src/runner/evidence | grep -v isolation/docker.rs)"`, and `test -z "$(git grep -l '"docker"' -- crates/candler-core/src | grep -v isolation/docker.rs)"`, and `! git grep -n 'cidfile' -- crates/candler-core/src`
- ⬜ [cmd] G5 digests: DIGEST and GOLDEN pass on the milestone tip
- ⬜ [cmd] G6 schemas: exactly one release-triggering commit touches the schemas crate, it is `feat(schemas)`, and it touches no other crate: `test "$(git log --no-merges --format=%s 18142ec..HEAD -- crates/candler-schemas | grep -cE '^(feat|fix|perf|refactor|style|revert)')" -eq 1 && git log --no-merges --format=%s 18142ec..HEAD -- crates/candler-schemas | grep -qE '^feat\(schemas\): '`; DRIFT holds
- ⬜ [cmd] G7 no further Python: `b=$(git grep -ciE "$PYRE" 18142ec -- crates/candler-core/src | awk -F: '{s+=$NF}END{print s}'); t=$(git grep -ciE "$PYRE" HEAD -- crates/candler-core/src | awk -F: '{s+=$NF}END{print s}'); test "$t" -le "$b"`, with `PYRE='python|venv|pyvenv|pip |uv |pyproject|site-packages|VIRTUAL_ENV|importlib'`
- ⬜ [cmd] G8 no MCP knowledge in the core: `! git diff 18142ec -- crates/candler-core/src | grep '^+' | grep -qiE 'tools/list|jsonrpc'`
- ⬜ [cmd] G9 frozen assets and no raw image build in CI: `git diff --quiet 18142ec -- assets/docker/base assets/docker/env && ! grep -q 'docker build' .github/workflows/container-e2e.yml`
- ⬜ [cmd] STD and LINUX pass on `feat/m5-container-isolation`; SUBJ passes over `18142ec..feat/m5-container-isolation`
- ⬜ [cmd] No confirm is left: `! git grep -n 'fn confirm_' -- ':(glob)crates/**'`
- ⬜ [cmd] Every new e2e is listed in CI's expected-guard list: for each name in the leaf reports, `grep -q <name> .github/workflows/container-e2e.yml`
- ⬜ [nl] The live smoke after L3 is recorded: a run with `isolation.egress = ["pypi.org:443"]` fetched pypi.org, failed on example.com, and its model calls went through the gateway. The run id is checkable with `candler show`
- ⬜ [nl] Each leaf report records the bite of every guard it introduced, and every verifier named in a leaf closed with no open finding
- ⬜ [nl] The rulings are recorded: `harness_baseline/argv.json` (A8) and D9's newly refused input
- ⬜ [cmd] The PR to `main` is titled `merge(isolation): land container isolation` (at most 72 characters with ` (#N)`), and its CI is green: `gh pr checks <N>`

## Gotchas
- **Critical conditions.** If any of these is wrong, many leaves go wrong silently:
  1. P0's green baseline;
  2. DIGEST with `argv.json` pinned until `container_create`;
  3. the `DockerDriver` additions and their fakes at `docker_client` step 1;
  4. the egress e2e's oracle being the target's own connection log;
  5. P1 and P3 deciding D1 and D2;
  6. D5's two commits;
  7. the plug-in fragment entering below the experiment file with its source recorded.
- **Condition tags.** Every Pre-condition and Success Gate line starts with `[cmd]` (decided by a command's exit status) or `[nl]` (a judgement). Every nextest call carries `--no-tests=fail`.
- **Standard gates**, named in every leaf:
  - **STD**: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`. Implementers run the `-p` forms of their crates; the coordinator runs the workspace form once per merge;
  - **LINUX**: `cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings`;
  - **DRIFT**: `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/schema_drift/)' && test -z "$(git status --porcelain crates/candler-schemas/schema)"`;
  - **SUBJ**: `cog check <base>..HEAD && git log --format=%s <base>..HEAD | awk 'length>72{b=1}END{exit b}'`, `<base>` being the leaf's branch point;
  - **SPAWN**, before any test run that may spawn a process: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core --no-tests=fail -E 'test(/guard_signal/)'`;
  - **GOLDEN**: `export CANDLER_GOLDEN_CORPUS="$GOLDEN" && cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'test(/guard_golden::/)' && cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'test(/guard_oracle_ac/)'`, with `$GOLDEN=/Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`. Read it; never write in it;
  - **DIGEST**: `git diff --numstat 18142ec -- crates/candler/tests/oracle crates/candler-core/tests/oracle/vectors crates/candler-schemas/tests/vectors ':(exclude)crates/candler-core/tests/oracle/vectors/field_paths.toml' | awk '$2!=0{b=1}END{exit b}' && ! git diff 18142ec -- crates/candler-core/tests/oracle/vectors/field_paths.toml | grep -q '^-[^-]' && cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/guard_cell_digest/)' && cargo nextest run -p candler-core --no-tests=fail -E 'test(/guard_import_mini|guard_legacy_mapping|guard_oracle_mini|guard_run_context|guard_engine|guard_seams/)'`;
  - **E2E**: `cargo nextest run -p candler-core -p candler --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::/) | test(/^guard_cli_e2e::/)'`, with Docker up.
  - The tests live in one integration binary per crate (`tests/it/main.rs`), so the M4 forms `binary(guard_x)` became `test(/guard_x/)`. The coordinator checks each filter at P0 with `cargo nextest list`: a filter that lists nothing fails under `--no-tests=fail`.
- **Test design** (`designing-tests`):
  - **Only guards stay in the suite.** Every test a leaf names is a guard. Confirms and probe scripts are discarded before the leaf merges, and none is committed.
  - Each guard is written first and seen red. Its doc carries a `Guards:` line and an `Oracle:` line naming an expectation derived without Candler's code, and its bite is recorded in the leaf report.
  - A test-name filter passes whatever the test asserts. It gates only beside the `Guards:`/`Oracle:` lines and the recorded bite.
- **Real-image rule.** A guard that needs an image builds or finds it through the core:
  - `DockerImageService` (`base()`, `ensure`, the gateway's production path);
  - test images (testagent, the fake model) through `docker_client`'s shared support function, which runs the one client's `DockerDriver::build` keyed by the context digest.
  It removes what it built, by id. Every new e2e sits under `guard_container_e2e::` or `guard_cli_e2e::`, and its name goes to the coordinator for CI's expected list.
- **Build footprint (shared machine).**
  - Export `RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/private/tmp/claude-501/m5-target-<leaf>` for every cargo command: one target dir per worktree. The coordinator's tip runs use `/private/tmp/claude-501/m5-target`.
  - Check `uptime` and `df -h /System/Volumes/Data` before building. At most two leaves build at once; serialise when the load is high.
  - Each leaf deletes its target dir before its last report. The coordinator removes a leaf's worktree, branch and target dir as soon as it is merged.
- **Safety.**
  - Signals go through the I22 guard only. Containers and networks are destroyed only through `ContainerGuard`.
  - Never `kill`, `pkill`, `killall`, `docker rm -f $(…)`, `docker … prune`, nor a `docker stop`/`rm` of an id the test did not create.
  - A real-daemon test removes only what it created, by exact id; record `docker images -q` before and compare after.
  - If Docker is down, `docker desktop start` and wait for `docker info`; never stop it.
  - e2e and live scratch roots go under `$HOME`, a default Docker Desktop share, with a `--mount type=bind` check on a fresh subdirectory first.
  - Never `git rebase -x` with the gates: git exports `GIT_DIR`, and the test helpers' `git init` would turn the shared repository bare.
  - Run `cog install-hook --all` in each new worktree.
- **Tokens.** One live smoke after L3, on RiVault Reasoning-Small; a second run only on an infrastructure failure. RiVault runs need no ask, but each must be needed.
- **Digests.**
  - Cell digests come from `comparable`. Nothing M5 adds may enter an existing cell: `isolation.egress` is absent unless set, `isolation.build.plugin` is a setting, and a plug-in fragment enters only the cells that select it.
  - A digest that moves is a defect to report, never a value to bless; an expected digest is never edited "until green".
  - `crates/candler-core/tests/fixtures/harness_baseline/argv.json` is byte-identical until `container_create`, which changes it only after a ruling.
- **Schema commits.** Only `wire_shapes` touches `crates/candler-schemas/`, in exactly one commit, `feat(schemas): …`, which touches no other crate. A wire defect found later goes to Eliott before any second schemas commit.
- **Behaviour changes.** These stop the implementer, who reports them; the coordinator sends them to the coordination session for a ruling before the leaf merges:
  - a newly refused input;
  - a moved stored digest;
  - a changed recorded data shape;
  - a change to `docs/contract/codes.toml` other than appending.
- **Shared names** (one owner each; a leaf that needs to change one reports it instead):
  - **`wire_shapes`:** the paths `isolation.egress` (cell value) and `isolation.build.plugin` (setting), `PluginManifest.agent_environments`, the container-record role `egress`, the allowed fragment paths, and the code `plugin.environment_path_unsupported` (the `<surface>.<reason>` form; an unknown key inside an environment is `plugin.unknown_field`);
  - **`docker_client`:** `DockerDriver::{inspect_many, create}`, `isolation::hardening::{hardening, Role}`, `isolation::control::ControlSpec`, and the shared test-image support function;
  - **`egress_setting`:** `isolation.egress_invalid` and `isolation.egress_with_bridge`;
  - **`plugin_environment`:** the preflight step `plugin_environment` and `isolation.plugin_environment_missing`;
  - **`egress_proxy`:** the allow network name `candler-<job12>-allow-<digest8>` and the gateway image's `egress` mode;
  - **`ground_truth_refusal`:** `isolation.ground_truth_exposed`.
- **Owned files and races.** Each leaf lists its OWNED paths. Implementers never edit `__design__/` (except `design_record`), `__roadmap__/`, another leaf's files, `cog.toml`, `.github/`, `.config/` or `Cargo.lock`. The coordinator resolves the append-only hunks: `docs/contract/codes.toml`, the `mod` lines of `tests/it/main.rs` and its e2e module files, the `resolve/steps/mod.rs` registration, and CI's expected list.
- **Merges.**
  - The coordinator lands each leaf with `git merge --no-ff -F <file>` after rebasing it onto the milestone tip, subject `merge(<scope>): land <leaf>`. Scopes: `wire_shapes` → `schemas`; `design_record` → `design`; every other leaf → `isolation`.
  - Level pre-conditions key on these subjects.
  - The coordinator pushes the milestone branch after each level, so CI's cold daemon runs the container e2e early.
- **Leaf reports.** Each leaf writes `__reports__/m5-container-isolation/leaves/<leaf>_v0.md` (`writing-reports`, type findings, no code dump). It records the guards and their bites, the claims checked against the code, the deviations and the spec defects found.
- **Briefs.** Every brief says:
  - "verify each claim against the code before changing it";
  - "report a spec defect instead of implementing it";
  - the stop-and-report rule above;
  - the colgrep init and path scoping for the leaf's worktree.

## Status
```mermaid
graph TD
    wire_shapes[Wire shapes]:::done
    client[Client]:::inprogress
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `wire_shapes.md` | 📄 Leaf Task | ✅ Done |
| `client/` | 📁 Directory | 🔄 In Progress |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
