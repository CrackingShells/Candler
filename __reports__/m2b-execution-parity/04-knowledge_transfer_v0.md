# M2b execution parity — Knowledge Transfer (v0)

Date: 2026-11-09

## Executive Summary

**What shipped** on `feat/m2b-execution-parity` (draft PR #4: 211 commits and 27 level merges over `origin/main`):
- `candler run` executes an experiment from any directory, with one Docker container per run. The run goes through a per-job internal network and an nginx gateway sidecar, so the provider token never enters the agent container.
- `candler ps`, `kill` and `watch --json`.
- Resume replays stored values; a forced resume records deviations.
- The stratum verdict.
- Every process and container is destroyed through the I22 guard.

**Primary outcomes:**
- Eliott's ruling of 2026-11-06 moved the Docker backend from M5 into M2b. The contract v5 features are kept as features (the feature map), not as mechanisms.
- The re-validation smoke passes 29 of 31 feature-map checks on real RiVault runs. The two failures are the carve-out `layout` (M2c) and `provenance_fields`. After the spare run, `provenance_fields` failed only on `planner.runtime`, an intended deviation (A59). Since `sandbox_contract` was renamed to `candler-container@1` (Eliott, 2026-11-09), it also fails on that check's `v<n>` shape, a second intended deviation.
- Gates on fe77f9f:
  - STD, LINUX, DRIFT and SUBJ;
  - macOS e2e 8 of 8 (run at 3330360; later commits add a host test and change comments only);
  - Linux CI `Container e2e` 9 of 9, b09 and b36 included;
  - 40 new guards, the ceiling.
- The canary is absent everywhere the smoke looked.

**What did not ship** and has an owner:
- declared inputs, `[fixed]` flattening and the treatment source belong to M2c;
- live metering and the grader's `planner.runtime` belong to M4;
- injection, the whole-container census, egress and plug-in environments belong to M5.

## Wins

- **The docker CLI as the supervised child.** M1's `Supervisor`, framed output and observer worked unchanged. There was no API client, and stdout is the stream-json.
- **I22 extended to containers** (`ContainerGuard`, a split `DestroyDriver`, the B16 source scan). The rule "recording fakes for every destroy bite, plus a test-side allowlist for e2e" held for the whole milestone. No container of Eliott's was touched.
- **The feature-map oracle**, derived from the reference only, turned "parity" into 31 scripts and 65 rows. Every smoke round was a table diff, not an opinion.
- **Verifiers found real defects:**
  - the A27 lock proof;
  - the A50 tick race;
  - the A52 self-view refusal, which no fake-only guard could have seen.
- **Rulings in one note** (A1 to A62) kept parallel leaves consistent across context resets.

## Pain Points and Root Causes

| Pain | Root cause | Next-cycle change |
|---|---|---|
| B34 failed only under full-workspace load (A51) | A product race: a run admitted just before a cap missed the stop. On top of it, a wall-clock cap in the test. | `LiveRuns` remembers the stop cause, and the test injects a `Clock`. Prove under full load ×3, never by retry. |
| Lost-job recovery refused every container in production (A52) | The refusal table was built only from foreign-job views, so the self-view clause was never exercised. | Each guard over a view needs one e2e that composes the production wiring. B36 is that e2e. |
| The first smoke found F1 to F13 | §14 of the plan left provenance ungrounded, so no leaf owned it (F9). The run end was unmeasured (F13), and the skill archive was unscoped (F1). | Every reference field named in the plan needs an owning leaf before dispatch. |
| F5 and F7 were "deferred", but were really a planning gap (A56) | No leaf owned the declaration surface, and `ExperimentFile` is shared with M2a. | Shared wire types get an alignment node (M2c), not a leaf workaround. |
| The smoke pinned the treatment at `htn-creator-eval@0.4.2` (N1) | My L3 addendum and leaf line 32 said "the HTN treatment at the cloned tag". The reference derives `skill_version` only from `htn-creator@…`. | A smoke treatment is always a skill tag (`htn-creator@X.Y.Z`). B38 pins the rule. |
| Replicate 2 equalled replicate 1 (N2) | RiVault caches identical requests. | None in Candler: a known provider limit (Eliott, 2026-11-09). See "For M4". |
| Disk pressure from parallel leaves | One `target/` per worktree, plus `cargo clean -p` for the I22 check. | `RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0`, never `CARGO_TARGET_DIR`, at most 6 implementers, and remove merged worktrees right away. |
| The guard-count gate read 43 | The grep counted `Guards: none` re-exec lines and M1's rewritten I14 line. | Ruling A62: the gate nets out removed lines and `none`. The real count is 40, B38 included. |

## Intended deviations from the reference (`run-config`, contract v5)

| Field or behaviour | Reference (v5, host sandbox) | Candler (`candler-container@1`) |
|---|---|---|
| `sandbox_contract` | `v5` (both `htn-creator-eval@0.4.2` and `htn-creator@0.2.7`) | `"candler-container@1"`: Candler's own constant, named by Eliott on 2026-11-09 (overruling A55's `"v6"`). It delivers the v5 feature set, as the feature map checks, through the container rather than the confinement hook. It is a `name@version` of Candler's mechanism, apart from the HTN repo's `v<n>` line, so a future HTN contract v6 cannot be mistaken for it; it is bumped only when the container changes what the agent gets. Pooling with the 240 golden runs is refused on `provenance.eval_infra.*`, not on this string. That is by design. |
| The `sandbox_contract` shape | the feature-map check `provenance_fields.sh:12` expects `^v[0-9]+$` | `candler-container@1` fails that clause. The check stays unedited, as the independent reference derivation. The row's observable, that each run's provenance names the contract version it ran under, is still met. |
| Workspace path | a per-run temp sandbox dir | `/work` inside the container. The sealed run dir keeps `workspace/` and `home/` (A28). |
| HOME and env | the user's HOME with `--setting-sources`, plus env scrubbing | `/home/agent`, a per-run host dir; the env is built from nothing (`-e` only); the flag is kept |
| Planner env | the agent's venv, seeded by pip | one uv env baked into an image layer at `/opt/env`, keyed by lock digest |
| Confinement | PreToolUse confine hook | the mounts are the boundary: read-only root, tmpfs, cap-drop ALL, no-new-privileges |
| `planner.runtime.*` | filled at plan time | absent (A59): no grader runs, and a null cannot be written |
| `eval_infra.dirty` | the whole tree | the watched source paths only, a narrower scope (A59) |
| `skill_version` | derived from `htn-creator@…`; raises otherwise | the same derivation; a foreign tag leaves it unrecorded (B38) |
| Init plugins | none | 3 builtin plugins ship with Claude Code 2.1.311: `cc-plugin-agents-md`, `cc-plugin-telemetry`, `cc-plugin-plugin-authoring`. They are allowed by F12 (`path = "builtin"`, source `…@builtin`). |
| Feature map | — | still contract v5 (reference `htn-creator-eval@0.4.2`) |

**The accepted macOS gap.** Host descendants that call setsid have no subreaper on macOS. What bounds it:
- the only host children are the docker client and the first-party catalog command, and neither calls setsid;
- containers carry labels and expire on their own (`timeout` wrapper, plus a 60 s margin);
- B36 checks the recorded pids' `CANDLER_RUN` marker;
- Linux CI decides the subreaper path (B33, B36).

## For M4

1. **Live metering (A51).** The job meter is fed only after the seal (`runner/exec`, around line 535). So a tokens or cost cap only sees ended runs, and a live run can overshoot it. M4's budget work needs per-frame usage fed to the meter. A cap crossed at setup ends the job with zero runs, which is correct.
2. **A known provider limit: RiVault may serve identical requests from its cache (N2).** Cost is reported on cache hits too, so `run.cost_usd` counts a cached reply as a paid one. Eliott ruled on 2026-11-09 that this is RiVault's limit and that Candler does nothing about it: turning the cache off would be far too expensive, and Candler must not over-specify for one provider. Do not build a workaround. The evidence is the probe of 2026-11-09, which sent 4 calls to `/v1/messages` at temperature 1:
   - the identical request was a cache hit;
   - the `Cache-Control: no-cache` header was ignored;
   - only the body field `"cache": {"no-cache": true}` bypassed the cache;
   - no hit header is exposed, and `x-litellm-response-cost` is reported on hits too.
3. **`planner.runtime.*`** stays absent until a grader runtime fills it (A59). The provenance verdict exempts it.
4. **The agent container has no `children/container-*.json` record** (A59); only the gateway has one. Recovery of an agent container relies on its labels and on `docker inspect` re-verification.
5. **Store identity.** Labels do not name the store. A copied `jobs/` dir of a live job, recovered with its own free lock, would read as Lost and sweep the live job's containers. Add a store id label before stores can be copied or shared.
6. **ZombieTick start is unguarded.** B33 starts the real tick itself, so deleting `ZombieTick::start` from `serve` (`jobs/runtime/session.rs:283`) leaves every test green.
7. **`census.init_tools`** is in `measured[]`, and the verdict refuses arms that differ in it. That is M4's hook for tool-surface factors.
8. **Comparability has no CLI.** The verdict lives in `manifest/compare.rs` (B03). The binary has no diff or verdict verb, so M4's analysis surface must add one.

## Owned elsewhere

| Item | Owner |
|---|---|
| F5 (declared inputs), F7 (`[fixed]` flattening, so `prompt.values` stops needing `--set` JSON), the treatment source setting. The feature-map rows S2, X19 and X32 and the `layout` check fail until then. | M2c wire alignment (A56, A58) |
| N3: `candler ps` needs `XDG_STATE_HOME` or `CANDLER_STORE_ROOT` (or a project) to find the store | cli follow-up |
| A36: `store_root` moves into `commands/common.rs` when M2a lands. M2a's step `environment-<plugin>@1` derives no path, so composing it with M2b's steps cannot give DerivedTwice (checked against `feat/m2a-evaluation-parity` 69997c7). | whichever of M2a or M2b lands second |
| A skill stored outside `.claude/skills/<name>` (`treatment.path`, a schema change) | M5 |
| Injection, whole-container census, egress allowlist, plug-in environments | M5 |

## Artifacts to Preserve

- Architecture: [00-architecture_v0.md](00-architecture_v0.md). Test definition: [01-test_definition_v0.md](01-test_definition_v0.md).
- Spikes: [02-gateway_spike_v0.md](02-gateway_spike_v0.md) and [03-image_probe_v0.md](03-image_probe_v0.md).
- Smokes: [equivalence_smoke_report_v0.md](equivalence_smoke_report_v0.md) and [equivalence_smoke_report_v1.md](equivalence_smoke_report_v1.md). Supervision gates: [supervision_gates_report_v0.md](supervision_gates_report_v0.md).
- The feature-map oracle and its check scripts: `__roadmap__/m2b-execution-parity/feature_map/`.
- The testagent image and the recording fakes (`DockerDriver`, `DestroyDriver`, `Signaller`): `crates/candler-core/tests/support/`.

## Open Questions

- Which model runs the first scored experiment? Reasoning-Small has never run the golden set, so its statistics are not comparable with the 240 earlier runs.
