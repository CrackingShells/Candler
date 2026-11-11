# M2c re-validation smoke: findings (v2)

Date: 2026-11-11

---
type: findings
topic: m2b-execution-parity
date: 2026-11-11
version: v2
prior-version: v1 (equivalence_smoke_report_v1.md)
key-metric: feature-map checks passing on a real RiVault run, 30 of 31 (prior: 29 of 31)
decision-required: intervene
---

## Headline Result

metric: feature-map checks passing (31 scripts, one valid real run, no `--set`)
value: 30 of 31 (1 fail: `provenance_fields`, on two named M2b deviations; `manifest_comparable` failed on the run and passes since the check was fixed on the tip)
unit: check scripts
prior: 29 of 31
direction: improved

S2, X19 (`layout`) and X32 now hold on a real job: `layout` exits 0 in the live agent container and the leak run is sealed `invalid(input_leak)`. `layout` and `planner_pin` are no longer failing. A new check failed on the run, `manifest_comparable` (finding N3); it was fixed on the tip afterwards (8e4d883, d4bc588) and now exits 0 on both stored runs, which makes 30 of 31 pass. `provenance_fields` exits 1 on exactly the two recorded deviations, and `skill_version` now reads `v0.2.7` (M2b's N1 shape bug is gone). Agent runs spent: 2 of 4. Model `RiVault/Reasoning-Small`, medium. Cost: the endpoint reported no price; client-side `total_cost_usd` 0.2576 + 0.0521 = 0.3097 USD.

## P4: the projection against a fake-docker run (pre-token evidence, Step 1 at 066f72e)

**Question.** Does `smoke_v2/projection.jq`, applied to a manifest of this experiment, give every name of `provenance_fields.txt` except the recorded deviations, and do `planner_pin.sh` and `provenance_fields.sh` read it without a jq error?

**Method.** The binary of this branch at 066f72e (clean checkout, own target dir, removed afterwards). The PATH-fake docker of `crates/candler/tests/support/cli_run.rs`, copied by hand into scratch and given two more branches: the package probe prints one reading, and the lock compile prints a short lock. `commands.sh stage`, then `commands.sh run` and `commands.sh leak` from an unrelated directory, with a fixture value in the key variable. Real docker was not called: `docker images -q` and `docker ps -aq` equal their before-lists.

**Result: the ten names**

| Name | Source in the manifest | P4 |
|:--|:--|:--|
| `skill_version` | `comparable.treatment.tag` = `htn-creator@0.2.7`, single-`v` rule | present, `v0.2.7` |
| `eval_infra.tag`, `.commits_ahead`, `.commit`, `.dirty` | `measured[]` `provenance.eval_infra.*` | present (`v0.5.0`, 61, the full commit, `false`) |
| `sandbox_contract` | `measured[]` `provenance.sandbox_contract` | present, `candler-container@1` |
| `planner.sandbox.distribution`, `.version` | `measured[]` `provenance.packages.gtpyhop.*` | present, but the values are my fake's reading (`gtpyhop-core`, `2.0.2`): only the path and shape are proved |
| `planner.runtime.distribution`, `.version` | none | absent, recorded deviation (A59); not invented |

The rule vectors of `skill_version` hold: `htn-creator@0.2.1` gives `v0.2.1`, `htn-creator@v0.2.3` gives `v0.2.3`, and `htn-creator-eval@0.4.2` or any non-skill tag leaves the name out.

**Scripts.**

| Script | Exit | Reading |
|:--|:--|:--|
| `planner_pin` | 0 (`OK planner_pin (gtpyhop-core)`) | only with `WORKSPACE_DIR` holding the real `scripts/pyproject.toml`. On the fake run's own workspace it exits 1: the input mount points are empty stubs, filled only by a real docker mount (P2). Step 2 reads the real workspace. |
| `provenance_fields` | 1, no jq error | fails on exactly two clauses: `planner.runtime.*` absent, and `sandbox_contract` is `candler-container@1`, not `v<n>`. With a stand-in `planner.runtime` of nulls and `sandbox_contract = "v1"` it exits 0, so no other clause fails. Both are recorded deviations; the gate line "exit 0 through projection.jq" cannot hold as written (question 2). |
| `layout` | 0, with the inputs overlaid by hand on the run's workspace | the declared layout satisfies S2 and X19. The real check runs against the live container in Step 2. |

**What the fake route showed about the experiment.** `candler run` resolved the file with no `--set`. The docker log shows one read-only file mount per input file: 6 servers at `/work/servers/rikyu_mcp/`, 23 scripts files at `/work/scripts/`, and `/work/pyproject.toml`. `comparable.inputs` holds `project`, `scripts` and `servers`. The prompt values arrive as one table in `comparable`. The fake runs seal `invalid(reader_error)` (no stream), as expected; the X32 verdict needs a real run.

**Finding P4-F1: file-level settings enter the cell.** With the settings in `[fixed]`, `comparable.values` holds `isolation.build.requirements`, `isolation.build.prune`, `isolation.record_packages` and `model.token_secret`. R01 §configuration surface classes all four as settings, "dropped from `Comparable`", and A2 says `record_packages` is "never in the cell". Cause: `runner/plan.rs::fresh_cells` takes every `[fixed]` path of `file.cells()` as a cell value and filters settings only for explicit (`--set`) paths. This is M2b's F7 settings leak, still open. The run works, and the leaked values are all constants of the experiment, but the cell digest carries them. The only no-`--set` route to a clean cell would be another layer, and `candler.toml` holds `store.root` and `plugins.dirs` only. Fixed on the tip (`fresh_cells`) before Step 2; the real manifest confirms it (question 1 below).

## Prepared files

The real manifest of Step 2 gives the same shape: 8 of 10 names, the two `planner.runtime.*` names absent.

## Setup

| Item | Value |
|:--|:--|
| Branch, binary | `m2c/equivalence_smoke` at 3b53511 (full sha 3b53511ec411780a93492b9cefa021482e0fc759), `git status --short` empty; `cargo build -p candler`, own target dir; binary `/private/tmp/claude-501/m2c-target-equivalence_smoke/debug/candler` |
| Commit the manifest carries | `provenance.eval_infra.commit` = 3b53511ec411780a93492b9cefa021482e0fc759, `dirty` false, `tag` v0.5.0, `commits_ahead` 72 |
| Treatment | `htn-creator@0.2.7` = 6af06f8, source written in the file; servers from `htn-creator-eval@0.4.2` = 28be2c1 |
| RiVault | `rv_health.py`: network, `llm` key (`RIVAULT_LLM_INFERENCE`) present and accepted, `RiVault/Reasoning-Small` in the live list. The key's value was never printed or written |
| Invocation | `commands.sh run` and `leak`, from `/private/tmp/claude-501/m2c-smoke/elsewhere`, `env -i`, no `--set` (`! grep -- '--set' commands.sh` holds) |
| Jobs | run `01a1c857-1be6-7c0d-86a8-63581087f2c7` (cell `89b02aec…`), leak `01a1c85c-0a0c-7f68-b41b-4f67a93c6f20` (cell `82e5809a…`) |
| Event literal | `job.started,container.created,run.started,container.created,container.destroyed,run.ended,container.destroyed,job.ended`, both jobs |

## Runs, model, cost

| Run | Verdict | Turns | Duration | Client-side cost |
|:--|:--|:--:|:--|:--|
| 1: the cell | valid, `completed`, exit 0 | 15 | 36.9 s | 0.2576 USD |
| 2: X32 leak | `invalid(input_leak)` | 2 | n/a | 0.0521 USD |

Agent runs spent: 2 of the 4 approved, no retry. Model `RiVault/Reasoning-Small` only (init event `model` equals it, 0 MCP servers). The endpoint returned no price field; the figures are the client's `costUSD` with `costBasis: unknown`, as in M2b.

## The 31 checks

The gate's literal loop over `feature_map.json` yields 30 scripts: `non_root.sh` is listed in `checks/README.md` but mapped to no row. I ran 31, `non_root` included, and the table has 31 lines. Provisioning checks (`EXEC`) ran against the live agent container mid-run (container `17df20d67802`); their results are the `mid-*.out` files in the scratch. The checks that read the run's store or stream ran after the seal, over the real artifacts. `settings_sources_isolated` and `stream_invocation` need `EXEC` and a complete stream: they ran after the run with a stand-in `EXEC` (`pwd -P` answered from the inspect, `sh -c` run with `HOME` set to the run's kept home), as in v1. Not a live-container result. The committed `commands.sh checks` as first run did not complete: `manifest_outside` through a stand-in `EXEC` greps the host root and never returned, so I stopped it and ran each check singly. `checks1.tsv` and `checks2.tsv` in the scratch hold 25 rows in all, including stale FAILs of the aborted loop (`env_scrubbed`, `layout` through the post-run stand-in); the `mid-*.out` files are the live results and this table is the record. `commands.sh checks` is now split into `live` and `post` (N5).

What the stand-in `EXEC` did not test: the image's own `$HOME` for `settings_sources_isolated` (it used the run's kept home on the host), and `pwd -P` symlink resolution for `stream_invocation` (it answered from the inspect's `WorkingDir`).

`layout` and `env_scrubbed` passed live only; both fail on the host copy or through a stand-in (`layout`: the host `pyproject.toml` is a 0-byte stub; `env_scrubbed`: `cannot run env`).

| Check | Exit | First line | Where run | Cause if failing |
|:--|:--:|:--|:--|:--|
| benign_devices | 0 | OK benign_devices | live | |
| canary_absent | 0 | OK canary_absent | post | |
| env_scrubbed | 0 | OK env_scrubbed | live | |
| host_isolation | 0 | OK host_isolation | inspect | |
| integrity_verdict | 0 | OK integrity_verdict | post | |
| job_events | 0 | OK job_events | post | |
| layout | 0 | OK layout | live | |
| manifest_comparable | 1, then 0 | FAIL manifest_comparable: lists not canonical or a value has no source (at the run); `OK manifest_comparable` after the fix | post | finding N3, fixed on the tip (8e4d883, d4bc588); re-run on both stored runs, exit 0 |
| manifest_fields | 0 | OK manifest_fields | post | |
| manifest_outside | 0 | OK manifest_outside | live | |
| no_abs_paths | 0 | OK no_abs_paths | post | |
| no_bytecode | 0 | OK no_bytecode | post | |
| no_reference_answers | 0 | OK no_reference_answers | live | |
| non_root | 0 | OK non_root (uid 10001) | live | |
| outside_path_denied | 0 | OK outside_path_denied (mounts within the declared set; 0 Read/Write/Edit call(s) named an in-image path outside /work) | post | |
| planner_pin | 0 | OK planner_pin (gtpyhop-core) | post-run reconstruction with the declared blob, see note | |
| provenance_fields | 1 | FAIL provenance_fields: missing/ill-shaped field(s) | post | the two M2b deviations below |
| resume_runs | 0 | OK resume_runs | post | |
| run_dir_per_key | 0 | OK run_dir_per_key (1 runs) | post | |
| run_measured | 0 | OK run_measured | post | |
| run_sealed | 0 | OK run_sealed | post | |
| settings_sources_isolated | 0 | OK settings_sources_isolated | post, stand-in EXEC | |
| skill_discovery | 0 | OK skill_discovery (listed=true) | post | |
| stderr_kept | 0 | OK stderr_kept | post | |
| stream_invocation | 0 | OK stream_invocation | post, stand-in EXEC | |
| stream_wellformed | 0 | OK stream_wellformed | post | |
| tree_inventory | 0 | OK tree_inventory | post | |
| venv_contents | 0 | OK venv_contents | live | |
| venv_not_hardlinked | 0 | OK venv_not_hardlinked | live | |
| venv_scoping | 0 | OK venv_scoping | live | |
| workspace_no_pyvenv | 0 | OK workspace_no_pyvenv | post | |

`planner_pin` note: the mid-run attempt errored (`PROVENANCE_JSON: parameter null or not set`, the manifest was not yet projected); the OK is a post-run reconstruction. `WORKSPACE_DIR` on the host holds empty stubs for mounted input files (`scripts/pyproject.toml` is 0 bytes; Docker fills it only inside the container, P2/P4). On the host workspace the script exits 1 (`skill pins zero or several planner distributions:`). With `WORKSPACE_DIR` holding the declared `scripts/pyproject.toml` (the store blob that was mounted) it exits 0. The reported exit is the second, and that is the real file the agent saw. `layout` ran live, so the stub does not affect it.

### Fail causes against M2b's KT deviations

| Fail | Cause | Named M2b deviation? |
|:--|:--|:--|
| `provenance_fields` | `planner.runtime.distribution` and `planner.runtime.version` absent from the projection (confirmed: the only two missing names) | Yes: `planner.runtime.*`, A59 (KT intended-deviation table). The KT's later note that M2c withdraws this (amendment A2) concerns a grader runtime; no field exists to project, and the gate was not amended, per the coordinator |
| `provenance_fields` | `sandbox_contract` is `candler-container@1`, which fails `^v[0-9]+$` | Yes: the `sandbox_contract` shape (KT, 2026-11-09 rename) |
| `manifest_comparable` | `comparable.inputs` is sorted by `kind` (`project, scripts, servers`), as R01 says ("sorted by kind"). The check's `canon` is `. == sort and . == unique`; jq orders objects by the value of their first key in key order, so `digest` decides before `kind`, and `unique` sorts the same way. `.comparable.values` paths are sorted and every one has a `sources[]` entry (0 missing) | No. Finding N3, fixed on the tip (8e4d883, d4bc588) |

N3 stayed hidden in M2b: its runs had `inputs: []`, which any sort satisfies. On the real manifest's `comparable.inputs` (the verifier's table):

| Test | Result |
|:--|:--|
| `. == sort` | false |
| `. == unique` | false |
| `. == sort_by(.kind)` | true |
| `. == sort_by(.kind, .digest)` | true |

N3 is fixed on the tip (8e4d883 `docs(design)`, d4bc588 `rdm(orchestrator)`). Cause: the check's jq object order against the reader's key order. The old `canon` (`. == sort and . == unique`) ordered objects by their sorted key names, so `digest` came before `kind` (and lock digest first for environments); R01 orders `inputs` by kind then digest. R01 now names each list's key (`values` by path, `snapshots` and `inputs` by kind then digest, `environments` by plug-in then lock digest), and the check tests that each list rises strictly by that key. This check was edited with Eliott's approval, because the failure was the check's, not the product's. The table above is the evidence. After the rebase onto d4bc588 I re-ran `manifest_comparable.sh` with `RUN_DIR` set to each stored run dir: the cell run `89b02aec…-r1-a1` exits 0 and the leak run `82e5809a…-r1-a1` exits 0.

## Other gates

| Gate | Result | Last relevant output |
|:--|:--|:--|
| P4 on the real manifest | PASS | projection of the real manifest gives 8 of 10 names, the two absent are `planner.runtime.*`; `planner_pin` reads it with no jq error |
| No `--set` | PASS | `! grep -n -- '--set' commands.sh` exits 0 |
| `layout`, `planner_pin` exit 0 | PASS | `OK layout` (live), `OK planner_pin (gtpyhop-core)` (declared blob) |
| `provenance_fields` exit 0 through the projection | FAIL, named | exit 1, causes in the table above |
| 31 checks, only M2b deviations may fail | PASS | 30 of 31 exit 0; the one FAIL is `provenance_fields`, on M2b's two recorded deviations. `manifest_comparable` failed on the run (N3) and passes with the check fixed on the tip with Eliott's approval (8e4d883, d4bc588) |
| X32 | PASS | run `82e5809a…-r1-a1` `integrity {"kind":"invalid","reason":"input_leak"}` |
| Canary scan | PASS | `OK canary_absent`; also absent in the leak run's workspace, home and job dir, the gateway's `docker inspect` and `docker logs`, the agent inspect, the stream, `exp/`, `elsewhere/`, the stdout and stderr files, and inside every `.gz` of the store |
| Nothing left | PASS | `docker ps -a` and `docker network ls` filtered by each job's label print nothing; no `candler` network; no process carries `CANDLER_RUN` |
| Images by id | PASS | removed `2f95c23fe99a` (candler-env:b214811c…), `a370af116a43` (candler-base:src-319fda768a95), `309f5726f608` (candler-gateway:latest); `docker images -q` and `docker ps -aq` equal the before3 lists |

The before-snapshots hold bare ids, not tags. The three images were absent from all three before-lists, and the env and base tags were set in this smoke's first seconds (02:01:46 to 02:01:48 UTC; the store holds locks for those two). That `candler-gateway:latest` was absent before the smoke is inferred: `ensure_sidecar` (`crates/candler/src/commands/run.rs:275-292`) builds it only when absent, and no such image remains now. It is not read from a tag. No container was left, so nothing was removed there.

## Moved live HTN cell digests

**Completeness cannot be shown.** M2b's store did not survive, and its reports recorded no cell digest. They recorded the `comparable` sha256 (v1) and the lock, and none of `prompt.template`, `budget.per_run.*`, `model.endpoint`, `model.effort` or `permission_mode`. M2b's cell digest is not recoverable, and `7dad90cf…` is a sha of `comparable`, not a cell digest, so the two columns below are different digests. The table lists what M2b recorded against this run, and nothing else can be claimed to have moved or not moved.

| | M2b (v1) | This run |
|:--|:--|:--|
| Digest recorded | `comparable` sha256 `7dad90cff9dd30df06a1bba0eb8657c88682fe26eac024305d659c32bffa9053` (both replicates) | cell `sha256:89b02aecc7ce4c4b37e987746a9c33524e3a7239fb9062ffdbe9139237ba511d` (leak cell `82e5809a…`) |
| `isolation.env_lock` | `sha256:8ff09ffe…` | `sha256:01e3e6b265ac9cd156329c54bd94f9628c9071340edb60783c12997f5f613e3f` |
| `isolation.image` | not recorded | `sha256:2f95c23fe99a…` (removed) |
| Treatment | replicates: `htn-creator-eval@0.4.2`, 28be2c15…; spare third run: `htn-creator@0.2.7` (v1, "Spare run"), commit not recorded | `htn-creator@0.2.7`, 6af06f8f… |
| `inputs` | none (`--set` workarounds) | `project cd1011fa…`, `scripts 2439109a…`, `servers bdf19441…` |
| Settings in `comparable.values` | `isolation.build.requirements ["requirements.in"]` among them (v1 step 1) | none |

Causes of what moved, from what M2b recorded:

1. **The lock.** M2b also built from `requirements.in` (v1 step 1). What changed is that `stable.in` left the lock: P3's answer is that the lock differs only in `# via` lines, so the lock digest, `isolation.image` and the cell move.
2. **Inputs are declared.** `comparable.inputs` holds three entries where M2b's was empty.
3. **Settings leave the cell** (P4-F1 fixed on the tip): M2b's values carried `isolation.build.requirements`.
4. **Treatment.** The tag the replicates used was `htn-creator-eval@0.4.2`; the spare run already used `htn-creator@0.2.7`. Against the replicates the tag and commit differ; against the spare run only the commit is unknown to the record.

Not a cause: `prompt.values`. M2b's `--set` JSON also gave a whole object (M2b v0 F7), and nothing M2b recorded shows the value moved. Whether the template, the budget, the endpoint, the effort and the permission mode moved is unknown.

## Answers to the Step 1 questions

1. **P4-F1, answered.** The tip's `fresh_cells` keeps `[fixed]` settings out. On the real manifest `comparable.values` holds 11 paths: `budget.per_run.{max_cost_usd,turns,wall_s}`, `isolation.env_lock`, `isolation.image`, `model.{effort,endpoint,id}`, `permission_mode`, `prompt.template`, `prompt.values`. No `isolation.build.*`, no `isolation.record_packages`, no `model.token_secret`.
2. **`provenance_fields` exit 1**, reported as above with its two causes; gate and check not amended.
3. Variable name, endpoint and model came from M2b's report and were confirmed by the skill's checks.
4. Servers from `htn-creator-eval@0.4.2`, skill from `htn-creator@0.2.7`, unchanged.
5. The binary was rebuilt at 3b53511 and carries it, `dirty` false.

## Findings

| Id | Finding | Owner |
|:--|:--|:--|
| N3 | `manifest_comparable` failed once `comparable.inputs` was non-empty: the product orders by kind (R01), the check's jq order put digest first. Fixed on the tip (8e4d883, d4bc588); re-run exit 0 on both runs | closed |
| N4 | On the host, mounted input files appear as 0-byte stubs in the run's `workspace/`; `planner_pin` and the post-run `layout` cannot read the real files from the store. `planner_pin` needs the declared blob; `layout` needs the live container | check inputs (known from P2/P4) |
| N5 | `commands.sh checks` could not run all 31 with one `EXEC`: `manifest_outside` needs a live `EXEC`, or it scans the host. Fixed in `commands.sh`: `checks live` (needs `EXEC=docker exec -w /work <agent container id>`, runs the nine `LIVE_CHECKS` while the job is up) and `checks post` (the sealed-artifact checks); the list is the feature map's plus `non_root`. Tested on `post` against this run's artifacts, with no `--set` and no key value | commands.sh (done) |
| N6 | The `isolation.image` id is in `comparable.values`; it is the image id of a rebuilt layer stack, so it is stable only while the image is kept. I removed it by the leaf's gate | design question, not a defect observed |

No Candler defect stopped the run; both runs were clean. Not done: a control run that keeps the leak cell with one changed byte (the leaf asks for it only if cheap; no second valid output was needed).

## Scratch

`/private/tmp/claude-501/m2c-smoke/` left in place: `state/` (the store), `cap/` (inspects, gateway log, stream, projection, shim), `exp/`, `check-*.out`, `checks1.tsv`, `checks2.tsv`, `docker_{images,ps}_{before3,after3}.txt`.
