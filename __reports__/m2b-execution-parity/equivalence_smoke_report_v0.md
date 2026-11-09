# M2b execution parity — Equivalence smoke, findings (v0)

Date: 2026-11-09

---
type: findings
topic: m2b-execution-parity
date: 2026-11-09
version: v0
prior-version: none
key-metric: agent runs spent: 0 of 5 (prior: N/A, delta: N/A)
decision-required: intervene
---

## Headline Result

metric: smoke steps completed
value: 1 of 5 (step 2 stopped at the first run, before any agent container)
unit: steps
prior: N/A
direction: new

The first `candler run` of the valid one-cell experiment ends `job.ended(failed)` with `run.failed(driver_failed)`. The treatment archive of `htn-creator-eval@0.4.2` holds 9 symlinks, and the driver's `skill_files` refuses a link. No agent container started, so no token was spent. The leaf stops here as a Candler defect (addendum rule); nothing was retried and no code changed.

This report has no B-test and so no bite record. Agent runs spent: 0 of 5 (X32 is a finding, not a run). Model for the runs that did not happen: `RiVault/Reasoning-Small`, `--effort medium`. Cost reported by the endpoint: none (the sidecar's readiness probe only lists models).

## Results Tables

### Setup

| Item | Value |
|:---|:---|
| Branch / base | `m2b/equivalence_smoke` at 4ce3256 |
| Binary | worktree `target/debug/candler`, built with sccache, `CARGO_INCREMENTAL=0`, own `target/` (1.4 GB) |
| Pre-spawn check | `guard_signal` 9/9 PASS, before the X39 run and before the first run |
| Treatment clone | scratch clone of `mcp-python-ingestion` at `htn-creator-eval@0.4.2`, commit 28be2c158fbc6ee786a789242c2d8fd47bfde9a8 |
| Case | eval 1 `tier-a-domain-structure`; the 0.4.2 suite declares no prompt (`make_prompt` builds it), so the cell uses a short own template (a deviation from the suite's prompt) |
| Settings | `--set model.token_secret=RIVAULT_LLM_INFERENCE`, `--set request.treatment_source=<clone>`; store through `XDG_STATE_HOME` inside scratch |
| Endpoint | `https://api.class2.llm.ai.r-ccs.riken.jp` (host only; the gateway takes a host) |
| VPN / key | `rv_health.py`: network ok, LLM key ok, 11 llm models |
| T0 | 2026-11-09T03:23:51 |

### X39 (missing build input)

| Check | Result |
|:---|:---|
| Exit status | 2 (non-zero) PASS |
| Names the missing path | FAIL: the message is `preflight failed: 0 failed step report(s), 3 step error(s)` and names no path |
| No job dir, no run | PASS (`store/` holds `control`, `images`, `store.json` only) |
| No container since T0 labelled `preflight` | PASS (0) |
| Side effect | preflight built `candler-gateway:latest` and `candler-base:src-319fda768a95` before it failed (product images, kept) |

### Run 1 (job 01a1be57-2702-76a2-8c9c-0c6cfad15426)

| Fact | Observed |
|:---|:---|
| Launch | exit 0, job id printed; the env image `candler-env:8ff09ffe…` was built in preflight |
| Event kinds, `process.*` dropped | `job.started, run.failed, container.destroyed, job.ended` |
| Literal expected | `job.started, container.created, run.started, container.created, container.destroyed, run.ended, container.destroyed, job.ended` |
| `run.failed` | `reason: driver_failed`, 7 ms after the `skill-archive` child exited 0 |
| Sidecar | started and passed its readiness probe (`/v1/models` through the upstream, so TLS to RiVault worked for that call); destroyed as `gateway`, found `recorded` |
| `candler ps` | `ended (failed)`, 1/0, no `lost` job |
| Left behind for this job | containers 0, networks 0, processes carrying its job id 0 |

### Images and disk

| Image | Id | Size |
|:---|:---|:---|
| candler-gateway:latest | 94ead961fe45 | 91.7 MB |
| candler-base:src-319fda768a95 | 8e9b91c6b2e0 | 706 MB |
| candler-env:8ff09ffe94aa… | f97e8abe2a18 | 788 MB |

`df` free on /System/Volumes/Data: 189 GiB (80 % used). Fixtures `candler-gateway:test` and `candler-testagent:test` untouched.

## Observations

| Signal | Baseline / Expected | Observed | Interpretation |
|:---|:---|:---|:---|
| Skill archive | `skill_files` takes the treatment's skill tree | The archive is `git archive` of the whole commit: 1447 entries, 9 of them symlinks (`scripts/*.py -> ../.claude/skills/htn-creator/scripts/*.py`) [source: `git archive … \| tar -tv` in the scratch clone; `prepare.rs` `skill_files`] | `skill_files` returns `InvalidValue` on any link, which fits `driver_failed`. The cause is read from the code and the archive; the run records no detail, so it is not proven by an error text. |
| Failure detail | A failed run names its cause | `run.failed` carries only `driver_failed`; `supervisor.log` holds no detail | Owner: cli_run or run_exec (event payload). Without it the cause cannot be read from the job. |
| X39 message | Names the path (leaf gate) | Counts only | Owner: experiment_resolution (`preflight.rs:181`). |
| Gateway events | First `container.created` is the gateway | No `container.created` for the sidecar before `container.destroyed(gateway)` | Not yet decided: the run failed before `run.started`; re-check on the first good run. |
| A25 (CA bundle) | Open until the first run | The readiness probe against RiVault passed | Provisional only; the chat path is untested. |

## Contradictions & Surprises

- The treatment source is the whole repository, not the skill directory, so any tagged repository with a link in it is refused. The reference HTN tag is such a repository.
- A failing `candler run` is not clean of side effects: preflight builds product images first, and the env image is built before the treatment archive is read.

## Findings for the lead

| Id | Row | Fact | Owning leaf |
|:---|:---|:---|:---|
| F1 | (X21/X20 path: treatment) | `skill_files` refuses the 9 symlinks of `htn-creator-eval@0.4.2`; the first run ends `driver_failed`. Blocks every run on the reference treatment. | claude_code_driver (prepare.rs), with run_plan/run_exec for the archive scope |
| F2 | run evidence | `run.failed` has no cause text. | run_exec |
| F3 | X39 | Refusal does not name the path. | experiment_resolution |
| F4 | job_events | No `container.created` seen for the sidecar. Undecided. | job_orchestrate / gateway |
| F5 | cli_run gap | Settings and the treatment source need `--set`; the catalog is the empty-servers stub; `JobSpec.inputs` is empty, so X32 `input_leak` cannot be driven by a declared input and is not run. The guard that covers the classifier is B24. | cli_run |
| F6 | cli_run gap | `isolation.build.requirements` is joined to the caller's cwd (`ctx.config.cwd.join`); I used a copy in the work dir and saw it resolve. | experiment_resolution |

## Not reached

The 31 check scripts, the 65-row coverage table, provenance, event order, resume, the forced deviation, the canary scan and the intended-deviation table are not run. They need a run that seals. The six `intended_deviation` rows (X20, X22, X24, X34, X35, X40) and the X39 absent-image case are as in the feature map and unexercised here; the absent env image was built, not refused, as the leaf expects.

## Steering Questions

- `[now]` Fix F1 (or pick a skills-only treatment repo for the smoke)? A symlink-free scratch repo holding the same skill tree would let the smoke continue, but it is no longer the reference tag.
- `[now]` Is a counted-only X39 refusal acceptable, or does it go back to experiment_resolution?
- `[next run]` Should `run.failed` carry a cause?
- `[next run]` Confirm the sidecar `container.created` event after the first good run.

## Pointers

- Scratch (not committed): `/private/tmp/claude-501/m2b-smoke/` (job dir `state/candler/store/jobs/01a1be57-…`)
- Leaf: `__roadmap__/m2b-execution-parity/runs/jobs/cli/closeout/equivalence_smoke.md`
- Feature map: `__roadmap__/m2b-execution-parity/feature_map/`
