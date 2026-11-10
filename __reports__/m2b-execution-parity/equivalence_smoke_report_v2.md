# M2c re-validation smoke: findings (v2, in progress)

Date: 2026-11-10

---
type: findings
topic: m2b-execution-parity
date: 2026-11-10
version: v2
prior-version: v1 (equivalence_smoke_report_v1.md)
key-metric: provenance_fields.txt names present through projection.jq on a fake-docker manifest, 8 of 10 (the 2 absent are `planner.runtime.*`, A59)
decision-required: intervene
---

Step 1 only. No token was spent: no agent run, no RiVault call, no gateway, no model endpoint. Steps 2 onward (the paid runs, the 31 checks, the moved digests, the cost) are written after Eliott's go.

## P4: the projection against a fake-docker run

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

**Finding P4-F1: file-level settings enter the cell.** With the settings in `[fixed]`, `comparable.values` holds `isolation.build.requirements`, `isolation.build.prune`, `isolation.record_packages` and `model.token_secret`. R01 §configuration surface classes all four as settings, "dropped from `Comparable`", and A2 says `record_packages` is "never in the cell". Cause: `runner/plan.rs::fresh_cells` takes every `[fixed]` path of `file.cells()` as a cell value and filters settings only for explicit (`--set`) paths. This is M2b's F7 settings leak, still open. The run works, and the leaked values are all constants of the experiment, but the cell digest carries them. The only no-`--set` route to a clean cell would be another layer, and `candler.toml` holds `store.root` and `plugins.dirs` only. Decision for Eliott before Step 2 (question 1).

## Prepared files

| File | Purpose |
|:--|:--|
| `smoke_v2/experiment.toml` | one cell, RiVault/Reasoning-Small, medium; treatment `htn-creator@0.2.7` from the local clone; prompt values, three declared inputs, requirements, prune, `record_packages = ["gtpyhop"]`; the key by variable name only |
| `smoke_v2/commands.sh` | the Step 2 command lines: `stage`, `run`, `leak`, `project`, `checks`, `leftovers`; carries no `--set` |
| `smoke_v2/projection.jq` | manifest to `PROVENANCE_JSON`, the ten names nested |

Layout choices, all from the reference: servers mounted at `servers/rikyu_mcp` (the reference's `servers/<dirname>` form for an absolute servers path; under `.claude/` the `layout` check would not see them); scripts are the skill's own at the tag; the pyproject is `write_sandbox_pyproject` byte for byte; the six requirements are `stable.in` at b017d31.

## Scratch left for Step 2

All under `/private/tmp/claude-501/m2c-smoke/`: `treatment/` (clone of PCfVW/mcp-python-ingestion, detached at `htn-creator@0.2.7` = 6af06f8f5761a4f22cc7c3e91397d83e840b378f; `htn-creator-eval@0.4.2` = 28be2c158fbc6ee786a789242c2d8fd47bfde9a8 is the servers' source), `fakebin/` and `p4/` (the fake docker and the P4 store), `docker_images_before.txt` (8 ids) and `docker_ps_before.txt` (3 ids). Nothing was created in docker.

## Open questions for Step 2

1. P4-F1: accept the settings in the cell for this smoke (the digests then carry them), or fix `fresh_cells` first (outside this leaf's OWNED files)?
2. The gate "`provenance_fields` exits 0 through the projection" cannot hold while `planner.runtime.*` is absent and `sandbox_contract` is `candler-container@1`: report the exit 1 with those two causes, or amend the gate?
3. The key's variable name (`RIVAULT_LLM_INFERENCE`), the endpoint and the model id come from M2b's report. The `using-rivault` finder and the VPN and key check are Step 2's, after Eliott's go.
4. The servers come from `htn-creator-eval@0.4.2` and the skill from `htn-creator@0.2.7`, because the skill tag holds no servers tree.
5. Rebuild the binary at the tip Step 2 runs on, so `eval_infra.commit` is that commit.
