# M2b execution parity: re-validation smoke, findings (v1)

Date: 2026-11-09

---
type: findings
topic: m2b-execution-parity
date: 2026-11-09
version: v1
prior-version: v0 (equivalence_smoke_report_v0.md)
key-metric: feature-map checks passing, 29 of 31 (prior: 24, delta: +5)
decision-required: intervene
---

## Headline Result

metric: feature-map checks passing (31 scripts, both valid runs)
value: 29 of 31 (2 fail: `layout`, carved out to M2c; `provenance_fields`, one new product finding plus one intended deviation)
unit: check scripts
prior: 24 of 31
direction: improved

F4, F6, F8, F10, F11, F12 and F13 hold on a real job. F9 holds except for the shape of `skill_version`: it reads `vhtn-creator-eval@0.4.2`, not `v0.4.2` (new finding N1, owner run_evidence). The event literal matches exactly. Agent runs spent: 2 of 3 (one job, two replicates). Model `RiVault/Reasoning-Small`, medium. Cost: the endpoint reported none; client-side `total_cost_usd` 0.2185 per run, 0.4370 USD in all. Replicate 2 replayed replicate 1 (N2, outside Candler).

## Results Tables

### Setup

| Item | Value |
|:---|:---|
| Branch | `m2b/equivalence_smoke`, fast-forwarded to c78dc4b, clean tree before the build |
| Binary | `target/debug/candler` 0.2.0, sccache, `CARGO_INCREMENTAL=0`, no `CARGO_TARGET_DIR`; no diff/verdict command (`run`, `ps`, `kill`, `watch` only) |
| Pre-spawn | `cargo clean -p candler-core` and `guard_signal` 9 of 9 PASS before the job |
| Job | `01a1beb5-7d4a-7cd6-8cdc-41b54399e4f1`, one cell, `replicates = 2`, fresh store `state2/`, ended (completed) 2/2, replicates ran in sequence |
| Treatment | scratch clone (the v0 clone was gone and was re-cloned), tag `htn-creator-eval@0.4.2`, commit 28be2c158fbc6ee786a789242c2d8fd47bfde9a8 |
| RiVault | `rv_health.py` exit 0 before the run |

A first `candler run` attempt was refused in preflight because the scratch clone did not exist yet (no job, no container, no tokens).

### Step 1: run from an unrelated directory (F6)

Both runs started from `elsewhere/` with the experiment named by absolute path, `isolation.build.requirements = ["requirements.in"]` left relative. The resolve succeeded and the cell records `isolation.build.requirements ["requirements.in"]`, `isolation.env_lock sha256:8ff09ffe…`, the same env lock as v0 (built from `work/requirements.in`); `venv_contents` passes. `elsewhere/` held no `requirements.in` (a cwd join would have failed as in v0) and is still empty (0 entries). F6 fixed.

### Step 2: event literal (F4)

Quoted (`process.*` dropped):

`job.started,container.created,run.started,container.created,container.destroyed,run.ended,run.started,container.created,container.destroyed,run.ended,container.destroyed,job.ended`

This is the leaf literal, `[run.started, container.created, container.destroyed, run.ended]` per replicate. The two replicates ran in sequence (not parallel), so the events do not interleave; the outer `container.created`/`container.destroyed` (gateway, run null) bracket both. F4 fixed.

### Step 3: `measured[]` (F13, F9)

Identical paths in both runs (values of r1; r2 differs only in `run.duration_ms`):

| Path | Value (r1) |
|:---|:---|
| `census.tools` | list of 23 names |
| `census.mcp_servers` | `[]` |
| `census.init_tools` | list of 23 names |
| `run.ended_by` | `{"kind":"completed"}` |
| `run.exit` | `0` |
| `run.duration_ms` | `40435` (r2: `5630`) |
| `run.usage` | keys: cache_creation, cache_creation_input_tokens, cache_read_input_tokens, fallback_credit, inference_geo, input_tokens, iterations, output_tokens, output_tokens_details, server_tool_use, service_tier, speed |
| `run.cost_usd` | `0.2184856` |
| `run.num_turns` | `16` |
| `run.tool_calls` | `{"Bash":13,"Edit":1,"Write":1}` |
| `provenance.skill_version` | `vhtn-creator-eval@0.4.2` (expected `v0.4.2`; N1) |
| `provenance.eval_infra.tag` | `v0.2.0` |
| `provenance.eval_infra.commits_ahead` | `201` |
| `provenance.eval_infra.commit` | `c78dc4b` |
| `provenance.eval_infra.dirty` | `false` |
| `provenance.sandbox_contract` | `v6` |
| `provenance.planner.sandbox.distribution` | `gtpyhop-core` |
| `provenance.planner.sandbox.version` | `2.0.2` |

`planner.runtime.*` is absent (A59, intended). Both manifests are `integrity valid` with `deviations []`.

### Step 4: the 31 checks

`PROVENANCE_JSON` was built from `measured[] provenance.*`; `outside_path_denied` got the agent's captured `docker inspect`. Results are identical for r1 and r2, except where noted. Exec-dependent checks (`benign_devices`, `env_scrubbed`, `layout`, `manifest_outside`, `no_reference_answers`, `non_root`, `venv_*`) ran mid-run in the live agent container of each replicate. Output of every PASS is `OK <name>`.

| Check | Exit | First line | Cause if failing |
|:---|:---|:---|:---|
| layout | 1 | FAIL layout: pyproject.toml missing, no [project], or has [build-system] | carved out, M2c (A58) |
| provenance_fields | 1 | FAIL provenance_fields: missing/ill-shaped field(s) | product: `skill_version` shape (N1); intended: `planner.runtime.*` absent (A59) |
| other 29 | 0 | OK | |

Notes on method, not on the product:
- `settings_sources_isolated` and `stream_invocation` need a live container and a complete stream. My probe loop reached no sealed stream in r1, and in r2 the container had ended. I ran them after the run with `EXEC` emulated by `exec_shim.sh` (`pwd -P` answered from the inspect's WorkingDir, `sh -c` run with `HOME` set to the run's kept HOME bind mount) and the run's own stream. Both exit 0. Not a live-container result.
- r2 `venv_scoping`: the first call printed OK; my second call (for the exit code) ran after the container had ended and returned 1. Recorded as exit 0 on the evidence of the OK line; a capture race of mine, not a finding. r1 gave exit 0 on both calls.
- `outside_path_denied` exit 0: mounts within the declared set; 0 Read/Write/Edit calls named an in-image outside path.
- `planner_pin` exit 0 (`gtpyhop-core`); `tree_inventory` exit 0 (`domain.py` only); `run_measured` exit 0; `job_events` exit 0.

### Step 5: coverage of the 65 rows (only what moved)

| Row | v0 | v1 | Cause |
|:---|:---|:---|:---|
| S3.R1, S3.R2, S3.R3 | not driven (tree_inventory FAIL) | PASS (tree_inventory); guard:B23 not re-run | F10 fixed |
| X35 | FAIL | PASS | F10 |
| S3.R14, X14 | FAIL | PASS | F11 |
| X2 | FAIL | PASS (EXEC emulated) | F12 |
| S3.R19 | FAIL | PASS | F9 (`planner.sandbox.*`) |
| X29 | FAIL | PASS | F13 |
| X5 | PASS, literal not met | PASS, literal met | F4 |
| X22 | deviation seen | refusal now prints `{path, stored, observed}`; forced run not re-driven | F8 |
| X1, X15 | PASS (stream of run 3) | PASS (run's own stream, EXEC emulated) | method |

Still failing, 6 rows: S1, S5, X25, X26 (provenance_fields: N1 and A59), S2, X19 (layout, M2c). X32 stays not driven (M2c). All other rows are as in v0. X39 was not re-run.

### Step 6: comparability verdict

The binary has no diff or verdict command, so none was run. Side by side, the two valid runs:

| | r1-a1 | r2-a1 |
|:---|:---|:---|
| `comparable` sha256 (canonical JSON) | 7dad90cff9dd30df06a1bba0eb8657c88682fe26eac024305d659c32bffa9053 | identical |
| `provenance.*` | the table in step 3 | identical, every path and value |
| `eval_infra.dirty` | false | false |

Treatment `{name htn-creator, tag htn-creator-eval@0.4.2, commit 28be2c15…}` is the same in both. By the rule in A55 (same `eval_infra.*` and `sandbox_contract`, not dirty) the stratum would be comparable. This is a reading of the digests, not a verdict from the binary; B03 covers the rule.

### Step 7: F8 refusal (no agent container)

Tag moved to 7fa72057… in the scratch clone only, `candler run` without `--force`, exit 2, no job, container set unchanged. Quoted:

`candler: refused: a re-checked fact differs from the stored cell: treatment {stored: {"commit":"28be2c158fbc6ee786a789242c2d8fd47bfde9a8","name":"htn-creator","tag":"htn-creator-eval@0.4.2"}, observed: {"commit":"7fa720577d458f10adad62ea6a2a4d5a0cf2ca59","name":"htn-creator","tag":"htn-creator-eval@0.4.2"}}`

It names path, stored and observed. The tag was restored (28be2c15…). F8 fixed.

### Step 8 and 9: canary, leftovers

| Item | Result |
|:---|:---|
| Canary scan: new store, job dir, events, supervisor log, raw streams (gunzipped), inspects, container logs, work dir, `elsewhere/`, the top-level stdout/stderr files | absent (186 files in the roots, plus 10 top-level files) |
| Containers labelled for my job | 0 (only `61ef24622a00` and `3bd621eae7a6`, not mine; container set equals the set before) |
| Networks named `candler*` | 0 |
| `candler ps` | one job, ended (completed) 2/2; no `lost` job |
| Stops used | none; no `candler kill`, no hand docker stop/rm/kill/prune |
| Processes 13271, 84145, 89489 | still alive, untouched |
| Images (kept) | `candler-env:8ff09ffe…` 788 MB, `candler-gateway:latest` 91.7 MB, `candler-base:src-319fda768a95` 706 MB; fixtures untouched |
| Disk | see the final report |

## Observations

| Signal | Observed | Interpretation |
|:---|:---|:---|
| r2 against r1 | same upstream message ids (`msg_1bac607b-…`), same 2664 stream lines, same tokens, cost, turns and tool counts; r2 took 5.6 s against 40 s | RiVault replays a cached completion for an identical request. Candler's gateway has no `proxy_cache`. The replicates are not independent samples (N2). |
| Agent init | 3 builtin plugins, 0 MCP servers, 23 tools | as v0, accepted by F12 |

## Findings for the lead

| Id | Fact | Owner | State |
|:---|:---|:---|:---|
| F1, F2 | skill scope, `run.failed` detail | claude_code_driver, run_exec | Fixed (v0) |
| F3 | preflight names failing step and path | experiment_resolution | Not re-run; F6 shows the path resolves |
| F4 | `container.created` events | job_orchestrate | Fixed: literal matches |
| F5 | no declared inputs; S2, X19, X32, `layout` | M2c (A58) | Open, carved out |
| F6 | build input relative to the experiment file | experiment_resolution | Fixed |
| F7 | `[fixed]` table flattening; `prompt.values` via `--set` JSON | M2c | Open, workaround kept |
| F8 | refusal prints `{path, stored, observed}` | cli_run | Fixed |
| F9 | provenance in `measured[]` | run_evidence | Fixed except N1; `planner.runtime.*` absent by A59 |
| F10, F11, F12 | check adaptations | feature_map | Fixed: all three PASS |
| F13 | run end measured | run_evidence | Fixed |
| N1 | `provenance.skill_version` = `vhtn-creator-eval@0.4.2`: `skill_version(name, tag)` strips `<treatment.name>@`, and the tag prefix is `htn-creator-eval@`, not `htn-creator@`. `provenance_fields` wants `v0.4.2`. | run_evidence (`runner/evidence/provenance.rs`) | New, open. Not fixed here. |
| N2 | RiVault replays identical requests, so replicate 2 equals replicate 1 | outside Candler | New. For the KT: replicates need a request that differs, or a cache bypass. |
| N3 | `candler ps` and the store need `XDG_STATE_HOME`/`CANDLER_STORE_ROOT` in every shell; without them `ps` lists nothing | cli_run | Observation |

## What the smoke decides for R02

| R02 gate | Smoke result |
|:---|:---|
| Sandbox features and provenance for the same cell | Met apart from `layout` (M2c, carved out) and `provenance_fields`: N1 needs a fix, `planner.runtime` is the A59 deviation |
| First event set | Met: the literal matches |
| Resume refusal prints the stored and observed values | Met (F8) |
| Canary nowhere | Met |
| Comparability diff returns a stratum verdict | Not decided: the binary has no diff command; the two runs' digests are identical |
| Kill the supervisor mid-run, no leftover | Not run (B36); no kill was used |

## Flakes and deviations

- No flake in the Candler runs. Both replicates sealed valid.
- Deviations from the brief: the scratch clone was re-created; `EXEC` was emulated for two checks (step 4); the plain resume and forced run were not repeated (not asked); the job ran both replicates in one `candler run`, so two of the three runs were spent.

## Pointers

- Scratch (not committed): `/private/tmp/claude-501/m2b-smoke/cap2/` (captures, per-run check TSVs, streams); the store `state2/` was removed after the report.
- v0: `equivalence_smoke_report_v0.md`
