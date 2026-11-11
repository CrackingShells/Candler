# Equivalence smoke

**Goal**: M2b's equivalence smoke is re-run with the inputs declared in the experiment file, `prompt.values` and the treatment source written in the file, the HTN environment declared by the experiment, and no `--set`. Feature-map rows S2, X19 and X32 and the `layout` check pass. `planner_pin` and `provenance_fields` pass through the committed projection, and every moved live HTN cell digest is listed.
**Pre-conditions**:
- [ ] [cmd] The leaf branches off the milestone after L2: `git log --first-parent --format=%s feat/m2c-wire-alignment | grep -qE '^merge\(.*\): land (input_seam|layering_guard|env_image_e2e)$'` holds for all three
- [ ] [nl] Eliott gave his go for this leaf's token spend and chose the model. RiVault is used only after he was asked. The `using-rivault` skill's variable finder and its VPN and key check report the endpoint reachable and the key variable present, and the key's value is never printed
- [ ] [cmd] Docker answers: `docker info >/dev/null`
- [ ] [nl] OWNED:
  - `__reports__/m2b-execution-parity/equivalence_smoke_report_v2.md`;
  - `__reports__/m2b-execution-parity/smoke_v2/{commands.sh,projection.jq,experiment.toml}`, the experiment with the key's variable name only;
  - scratch under `/private/tmp/claude-501/m2c-smoke/`.

  No code. The feature-map checks are run as they are, never edited
**Success Gates**:
- ✅ [cmd] P4: before any token is spent, `projection.jq` applied to a manifest from a fake-docker run of the same experiment yields every field of `provenance_fields.txt` except the recorded deviations (`planner.runtime.*`, A59), and `planner_pin.sh`/`provenance_fields.sh` read it without a jq error
- ✅ [cmd] No `--set`: `! grep -n -- '--set' __reports__/m2b-execution-parity/smoke_v2/commands.sh`
- ✅ [cmd] The 31 checks run as committed, against the run's real artifacts, with S2, X19 and X32 now included: `for c in $(jq -r '.[]|select(.check!=null)|.check' __roadmap__/m2b-execution-parity/feature_map/feature_map.json | sort -u); do sh __roadmap__/m2b-execution-parity/feature_map/checks/$c.sh || echo "FAIL $c"; done`. The report lists every exit status. The only FAIL lines allowed are recorded deviations of M2b's KT, each named
- ⬜ [cmd] `layout`, `planner_pin` and `provenance_fields` exit 0, the last through `projection.jq` (its `sandbox_contract` clause is M2b's recorded deviation, reported as such)
- ✅ [cmd] X32: a run whose prompt makes the agent copy a declared input to an output path is sealed `invalid(input_leak)`
- ✅ [cmd] The canary scan of M2b's smoke (`canary_absent.sh`) exits 0 over every location it lists
- ✅ [cmd] Nothing is left: `docker ps -a` and `docker network ls`, filtered by each job's label, print nothing, and the images this smoke built are removed by id
- ⬜ [nl] The report:
  - lists every moved live HTN cell digest, M2b's against this run's, with its cause (the lock, from P3's answer; `prompt.values` now whole; inputs declared);
  - states the agent runs spent (at most 4), the model and the cost the endpoint reported;
  - is written with `writing-reports` (type findings).
**References**: [M2b's smoke leaf](../../../../m2b-execution-parity/runs/jobs/cli/closeout/equivalence_smoke.md) — the steps, the check inputs, the safety rules; [R06 smoke reports v0, v1 and the KT's deviations](../../../../../__reports__/m2b-execution-parity/04-knowledge_transfer_v0.md); [the feature map](../../../../m2b-execution-parity/feature_map/README.md); [R10 P4, P6, G6f](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md)

## Step 1: Probe P4 and prepare without tokens
**Goal**: The experiment file, the commands and the projection are ready and checked against a fake-docker run before any paid run.
**Implementation Logic**:
1. Under `/private/tmp/claude-501/m2c-smoke/`, clone the treatment repository at the skill tag M2b's smoke used (`htn-creator@X.Y.Z`, B38).
2. Write `experiment.toml`:
   - `[treatment] source`;
   - `[fixed.prompt.values]`;
   - `[[inputs]]` for the servers tree, `scripts/` and the bare `pyproject.toml` the reference places;
   - `isolation.build.requirements` naming a requirements file with the six packages;
   - `isolation.build.prune = ["domain.py", "problems.py"]`;
   - `isolation.record_packages = ["gtpyhop"]`;
   - the model fields and the key's variable name.
3. Write `projection.jq`, mapping the manifest to `provenance_fields.txt`'s names:
   - `skill_version` from `comparable.treatment.tag`, by the reference's `v` rule;
   - `eval_infra.*` and `sandbox_contract` from `measured[]`;
   - `planner.sandbox.*` from `provenance.packages.gtpyhop.*`.
4. Run the experiment once against the fake docker of the test supports, and check the projection on its manifest (P4).
**Deliverables**: `__reports__/m2b-execution-parity/smoke_v2/{experiment.toml,projection.jq,commands.sh}`; the report's P4 section
**Consistency Checks**: `! grep -n -- '--set' __reports__/m2b-execution-parity/smoke_v2/commands.sh && test -s __reports__/m2b-execution-parity/smoke_v2/projection.jq` (expected: PASS)
**Commit**: `reports(orchestrator): prepare the m2c smoke without tokens`

## Step 2: The paid runs and the report
**Goal**: The one-cell run, the leak run and the checks, inside the token budget Eliott approved.
**Implementation Logic**:
1. Run `commands.sh` from an unrelated directory, capturing the inputs of M2b's Step 2 for the checks.
2. Run every check, then the X32 leak run.
3. Run the canary scan and the leftover checks.
4. Stop after a Candler defect and report it; never retry more than twice without Eliott's go.
5. Remove only what the smoke created, by exact id.
6. Write the report.
**Deliverables**: `__reports__/m2b-execution-parity/equivalence_smoke_report_v2.md`
**Consistency Checks**: `test -s __reports__/m2b-execution-parity/equivalence_smoke_report_v2.md` (expected: PASS)
**Commit**: `reports(orchestrator): record the m2c re-validation smoke`
