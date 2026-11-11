# M2c wire alignment — Knowledge Transfer (v0)

Date: 2026-11-11

## Executive Summary

**What shipped:** PR CrackingShells/Candler#14, merged as 9c18ed3. It holds 75 commits in 16 leaf merges, each `--no-ff`.
- **Declared inputs.** `[[inputs]] kind, path, at?, audience`.
  - Agent inputs enter the cell as `comparable.inputs[] {kind, digest}` and are mounted read-only, file by file.
  - Evaluator-only inputs go to `evaluation_inputs[]`, outside the hash, and the container never sees them (I8).
  - An evaluator's `needs.inputs[]` is served from either list.
- **Table values.** A table at or below `prompt.values` reaches the cell as one value.
- **The treatment source.** `[treatment] source` is recorded in `sources[].from`, never in the cell digest. `request.treatment_source` is retired.
- **The domain is out of the core, the binary and `assets/` (A2).**
  - The agent environment comes from the experiment (`isolation.build.requirements`, `isolation.build.prune`, `isolation.record_packages`).
  - The planner axis, the probe and `stable.in` are gone.
  - Import entry params come from the rules (`entry_params`).
  - The I1 guard matches identifier segments over every crate's src, the schema, `assets/` and the helper, and the tree holds 0 hits.
- **Build identity (E4).** A build outside a git checkout records what it can prove. A stratum is gated on a recorded and equal `eval_infra.commit` and `dirty`, and on `sandbox_contract`.
- **Amendment A1** (`settings_out_of_cell`). A setting in `[fixed]` stays out of the cell. A factor that varies a setting is refused (`SettingAsFactor`).
- R01 records every decision: D1–D9, plus the key each `comparable` list rises by.

**Primary outcomes:**
- **0 digest moves** on the golden import (240 runs, 10,320 records) and the mini import (6 runs, 136 records):
  - run keys, context digests and suite-entry digests are all unchanged;
  - the manifests differ in `import.rules` alone, by construction, because the rules file must gain `entry_params` ([digest_invariance_v1.md](digest_invariance_v1.md)).
- **The re-validation smoke** ([equivalence_smoke_report_v2.md](../m2b-execution-parity/equivalence_smoke_report_v2.md)):
  - one cell on RiVault/Reasoning-Small, from the experiment file alone, with no `--set`;
  - 2 agent runs, 0.31 USD client-side;
  - X32 sealed `invalid(input_leak)`;
  - 30 of 31 feature-map checks pass.
- **Gates.**
  - On the PR head: fmt, clippy, nextest 421/421, doc, Linux clippy, cog, GOLDEN 2/2 and `oracle_ac` 1/1. CI `Container e2e` passed, so X1 is green.
  - Not met as written: G6f is partial (below). The release check (`schemas@0.6.0`, `v0.6.0`, nothing else) is with the coordination session.

## Gates not met as written

| Gate | State | Why |
|---|---|---|
| G6f `provenance_fields` | exits 1 | on exactly M2b's two recorded deviations: `planner.runtime.*` is absent (A59), and `sandbox_contract` is `candler-container@1`, not `v<n>`. Neither the check nor the gate was amended. |
| G6f `planner_pin` | passes on the declared blob only | after a run, the host copy of a mounted input is a 0-byte stub (N4), so the check reads the stored blob |
| PR title | 72 characters with ` (#14)`; the leaf's line would be 74 | recorded deviation |

## Behaviour changes without a breaking marker

The versions are the same as with a `!`; only the changelog's BREAKING label is missing. Ruling (coordination session, after Eliott): no history rewrite. Items 1 and 2 are recorded here and in the PR body; item 3 goes to M4.

1. **0bc7668 `feat(schemas)`:**
   - an empty `prompt.values` table is refused with `EmptyTable`, also in hand-written JSON, which b017d31 accepted;
   - a table under `prompt.values` is one cell value, where it used to be one value per leaf.
2. **129e2a7 `fix(experiment)`:**
   - a factor that varies a setting is refused, where on b017d31 its levels were silently lost;
   - a stored experiment that wrote a setting in `[fixed]` (M2b wrote `isolation.build.requirements` there) gets new cell digests.
3. **716f079 `feat(manifest)`:** an unrecorded `eval_infra.commit` or `sandbox_contract` refuses a stratum. See "For M4", item 1.

## Wins

- **Gates as commands, measured on main before dispatch (P0).** Today's I1 guard passed on main with 82 segment hits it could not see. That finding sized the `layering_guard` leaf before any code was written.
- **One deriver per path, settled by ownership.** `declared_inputs` owned the single `inputs` deriver, so the merge produced no `DerivedTwice`.
- **Adversarial verifiers found real defects:**
  - parent-table factors evaded `SettingAsFactor`;
  - a bare `budget` factor was falsely refused through `is_setting`'s prefix match;
  - the ambient `CANDLER_EVAL_INFRA_*` leak at build time (fixed in 975048e);
  - the `cli_run` test world never sealed a run (input_seam F1).
- **The canary scans proved I8 end to end** (S1, I2). The smoke found no canary anywhere it looked.
- **Coordinator workspace runs after each merge caught a cross-crate seam** that a leaf's own gates could not see: core's `NUISANCE_PATHS` lacked `evaluation_inputs`.

## Pain Points and Root Causes

| Pain | Root cause | Next-cycle change |
|---|---|---|
| Gate results from before 6ba21c7 are unconfirmed (agent_env F1) | The plan briefed one `CARGO_TARGET_DIR` for every worktree. Cargo names a member's artifacts by its path relative to the workspace root, so one worktree linked a sibling's code. | One target dir per worktree, with sccache and `CARGO_INCREMENTAL=0` (the memory "leaf-build-footprint"). The coordinator's clean tip runs are the record. |
| A schemas-only leaf passed its gates and broke the core | Core mirrors `hashed_paths.toml` in code, and a `-p candler-schemas` gate cannot see it. | Run the workspace suite on the rebased leaf branch before merging any leaf that changes a wire type. |
| A guard asserted about a run the test world could never seal (input_seam F1) | The skill was at `demo/`, not `.claude/skills/demo/`, and there was no `permission_mode`. The CLI guards only waited for `job.ended`. | A guard that reads a sealed run asserts the seal first. |
| Two spec defects in the A1 leaf | `request.inputs` moves the digest by D2. A factor-level setting never reaches the resolution, so its levels collapsed silently. | An amendment leaf names the digest effect of every path it moves, and probes the factor route as well as `[fixed]`. |
| A planned bite could not hold (wire F1) | "Remove `skip_serializing_if` turns `guard_cell_digest` red", but the new fields sit outside `comparable`. | Check a bite against the guard's oracle at planning time; the two wire-shape guards cover it. |
| Leaf pre-conditions broke mid-campaign (design_record F1) | `merge-base --is-ancestor feat/m2c-wire-alignment HEAD` fails once the coordinator commits rdm updates. | A pre-condition names the merge subject, not ancestry of a moving branch. |
| A feature-map check refused every live run with inputs | `manifest_comparable.sh` used jq's `sort`, which orders objects by their key names (digest before kind). M2b's runs had `inputs: []`, which hid it. | Fixed with Eliott's approval (d4bc588): each list rises strictly by its R01 key (8e4d883). Checks over lists need one non-empty fixture. |
| The smoke report overstated the causes of the moved digests | The cause was inferred from the diff, not from M2b's runs: `prompt.values`, and the treatment tag M2b's spare run already used. | A moved-digest cause is listed only with its evidence, or not at all. |
| Four unmarked behaviour changes | Leaf briefs named `!` only for removed surfaces, not for newly refused input. | A brief says that a newly refused input, or a moved stored digest, is a behaviour change, and sends it to the supervisor for a ruling (memory "no-external-users-escalate-compat"). |
| RiVault outage (502 on every family) | Maintenance. The stopped agent could not be resumed. | Health check first (order step 1); re-dispatch fresh and re-check what the stopped agent left. |

## Flakes seen (no change in M2c)

- **Under load (load average ~20, three builds at once):**
  - `guard_protocol_client` (`every_failure_path…`, `a_deadline_beats…`) failed once, then passed 3 times out of 3 on its own;
  - `guard_container_record::stopped_run_has_its_agent_recorded…` failed once.
- **CI on PR 14:** `guard_job_runtime::b33_setsid_descendant_dies` failed once ("the sweep did not find the detached process"), and the re-run passed (run 42949488202, attempt 2).
  - **Root cause:** `started_after` (`supervise/signal.rs:222`) requires a Linux start strictly later in ticks (CLK_TCK=100) than the supervisor's own. A grandchild started in its ancestor's tick 274 times out of 300 in a probe.
  - **Where the rule is pinned:** R01 ("later than the supervisor's own") and the "the same ticks → refused" row of `guard_signal`, both from M1 (4fb787f).
  - **Effect:** the sweep can leave a real orphan alive.
  - **Next step:** a separate session is proposed; it needs Eliott's ruling on I22.

## For M4

1. **Imported strata are refused under D8.**
   - `stratum_verdict` (`manifest/compare.rs:396`) refuses an unrecorded `eval_infra.commit` or `sandbox_contract`.
   - Imported manifests record neither: their provenance holds only `planner.sandbox.*` from the legacy rules. So all 240 golden runs are refused as one stratum today.
   - Nothing in src calls `stratum_verdict` yet.
   - The coordination session proposed an M4 gate to Eliott: decide the rule for imported strata before M4's analysis surface calls the verdict.
2. **`sandbox_contract` unrecorded in every arm counts as equal** (an open question from the verdict_gating verifier). D8 says "equal". Decide whether a real manifest can lack it, and refuse it if not.
3. **Comparability still has no CLI** (M2b's KT, item 8). M4's analysis surface adds the verb that calls the verdict.
4. M2b's KT item 3 (`planner.runtime.*`) stays withdrawn. A grader's packages are reported through `describe`'s `environment.packages{}`.

## Follow-ups (not in M2c)

| Item | Owner |
|---|---|
| `candler run` reports a factor refusal as `Failed` where other planning errors are `Refused`. It does so only after resolving the image and starting the sidecar. | cli follow-up |
| The `TreatmentSpec.source` doc in candler-schemas is stale. It should say file-relative, or cwd-relative for `--set`, recorded absolute and normalised. | `docs(schemas)` |
| `--set` accepts unknown `request.*` keys | cli follow-up |
| The sweep refuses an orphan started in the supervisor's tick | separate session; Eliott's ruling on I22 |
| The packaged `eval_infra` origin is guarded but not exercised end to end: P1 could not package `candler` (`publish = false`, unversioned path deps). It matters once Candler is published to crates.io. | the first crates.io release |

## Decisions for Eliott

- The plug-in's `import/htn-results-rules.toml` needs `entry_params`, and `guard_golden_parity` pins its sha256, which moves with it.
- A treatment with a name and a source but no tag or commit runs and records no commit, as on b017d31: a no-skill control arm. Keep it, or require a tag or commit?
- `isolation.image` sits in `comparable.values` and names an image that may later be removed (N6).
- Which model runs the first scored experiment? Reasoning-Small is not comparable with the 240 golden runs.

## Artifacts to Preserve

- Gap analyses: [gap_analysis_v0.md](gap_analysis_v0.md) (A1) and [gap_analysis_planner_v0.md](gap_analysis_planner_v0.md) (A2).
- [digest_invariance_v1.md](digest_invariance_v1.md): the main-against-branch method for the golden and mini imports.
- Leaf reports, with each guard's bite: [leaves/](leaves/).
- The smoke: [equivalence_smoke_report_v2.md](../m2b-execution-parity/equivalence_smoke_report_v2.md), and `smoke_v2/` (experiment, command lines, `jq` projection).
- The campaign and its gate commands: `__roadmap__/m2c-wire-alignment/`.
