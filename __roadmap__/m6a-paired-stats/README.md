# m6a-paired-stats

## Context
Milestone M6a of `candler-v1`, the leaf `stats/m6a_paired_stats.md` (R02), amended by A9 of its parent README. It runs off `origin/main` on `feat/m6a-paired-stats`. Today `analyze` compares one-factor arm pairs with Fisher's exact test, Newcombe and Holm; it cannot compare two treatments in one experiment, and its tests were never checked against the design Candler's experiments have. M6a does both, and reports `instrument_error` per arm. Depth is ordering:
- **L0, this level:** `exact_tests`, `cmh`, `resampling` (the battery's engines); `treatment_setting` (T2), `treatment_verdict` (T5), `arm_counts` (E1 and per-run units), `execution_order` (B3).
- **L1, `resolve/`:** `level_resolve` (T3, T4), `blocking` (blocks and test selection).
- **L2, `resolve/wired/`:** `level_wiring` (T3 wiring), `analyze_output` (the comparisons rule in `analyze`).
- **L3, `resolve/wired/proven/`:** `golden_design`, `import_treatment`, `known_truth`, `design_record` (A8: `golden_blocked` is blocked, superseded by `golden_design`).
The analysis chain (cmh, blocking, analyze_output, proven) is the critical path; the planning chain (treatment_setting, level_resolve, level_wiring) runs beside it. `analyze_output` needs only `blocking` and L0; it sits at L2 for that reason. No gate needs Docker or a model call. DIGEST and GOLDEN must not move.
## Reference Documents
- [R01 Architecture](../../__design__/architecture.md) — §Analysis statistics, §Contracts & Invariants (I5, I7), §Future features
- [R02 M6a node](../candler-v1/contracts/parity/alignment/surfaces/isolation/m5b/stats/m6a_paired_stats.md) — the gates this campaign decides, as amended by A9 (read only)
- [R03 Approved plan](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md) — the statistical design, decisions T1-T5, N3, E1, B3, the leaf × file map, races, conditions, budget
- [R04 A7 findings](../../__reports__/candler-v1-review/03-findings_amendment_a7_v0.md) — treatment arms: what is a setting today and why a setting factor is refused
- [R05 Cost report v2](../../__reports__/agent-token-costs/03-findings_campaign_costs_v2.md) — the corrected list rates and per-call costs the budget uses
- [R06 M5b campaign](../m5b-egress-proxy/README.md) — the standard gate forms and the leaf-report convention
- [R07 CONTRIBUTING](../../CONTRIBUTING.md) — types, scopes, the schemas-release rule
- R08 `designing-tests` skill, science procedure — roles (guard, confirm, probe), validity evidence, known-truth runs, the false-safety catalog

## Goal
`analyze` answers "does B change the pass rate compared with A?" with a test chosen by the design before the data: one comparison per declared factor and level pair, blocked by the other declared factors, each test equal to a second engine and seen holding its nominal error rate on known truth. One experiment can declare two treatments; each cell resolves, mounts and records its own. Each arm reports `instrument_error`. No golden or parity digest moves.

## Pre-conditions
- [x] [nl] Eliott approved the plan (2026-11-18, round 4; all subagents on Sonnet; cap $105), and amendment A9 of R02's parent records the gate changes
- [ ] [cmd] The milestone branch exists and `$BASE` is its cut point: `BASE=$(git merge-base origin/main feat/m6a-paired-stats) && git merge-base --is-ancestor $BASE feat/m6a-paired-stats`; the L0 coordinator records `$BASE` in the Progress table
- [ ] [cmd] The engines are in the uv cache: GEN (below) prints six `rows match` lines and exits 0 on `$BASE`
- [ ] [nl] The L0 coordinator has landed the scaffold commit and answered the golden probe (Gotchas) before dispatching L0

## Success Gates
- ⬜ [cmd] G1 engines: every test equals its second engine, and the generator is deterministic and offline: GEN passes, and `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats_exact::/) | test(/^guard_stats_cmh::/) | test(/^guard_stats_resampling::/) | test(/^guard_stats::/)'`
- ⬜ [cmd] G2 golden design (A8): the HTN golden experiment (3 models × 4 htn-creator versions × 20 runs) is one experiment file that plans 12 cells, and on a store of that shape `analyze` gives 6 version comparisons blocked by model and 3 model comparisons blocked by version, each equal to an independent second engine within 1e-9: `cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_golden_design::/)' && (cd crates/candler/tests/oracle/golden_design && uv run --offline --no-project --with scipy==1.17.0 --with statsmodels==0.15.0 python second_engine.py --check)`
- ⬜ [cmd] G12 import (A8): an imported experiment declares the same factors as its native rerun: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_import_mini::/)' && CANDLER_GOLDEN_CORPUS=/Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'test(/^guard_analyze_golden::declared_/)'`
- ⬜ [cmd] G3 citations: every validity source is cited in R01: `for k in 10.1002/sim.3531 Mantel Fleiss Robins Phipson Efron; do sed -n '/^## Analysis statistics/,/^## /p' __design__/architecture.md | grep -q "$k" || exit 1; done`
- ⬜ [cmd] G4 E1: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_stats::t3_/) | test(/^guard_analysis_report::e1_/)'`
- ⬜ [cmd] G5 treatment plan and wiring: two levels plan one cell per level, each mounting its own sources, with one image per distinct lock: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_run_plan::t3_/) | test(/^guard_image::t4_/) | test(/^guard_job_orchestrate::t3_/)'`
- ⬜ [cmd] G6 I7: every other setting factor is still refused: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_settings_out_of_cell::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_cli_refusal::r[23]_/)'`
- ⬜ [cmd] G7 one factor (A6): a two-level one-factor store with binary expectations only gives today's output except the mid-p p-value: `cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_analyze::c6_one_factor_/)'`
- ⬜ [cmd] G8 known truth: under H0 the false-positive rate's Monte Carlo interval contains α, and the Simpson case holds nominal blocked and not pooled: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_known_truth::/)'`
- ⬜ [cmd] G9 digests: DIGEST and GOLDEN pass
- ⬜ [cmd] G10 hygiene: STD, LINUX and SUBJ pass; no schemas commit: `test -z "$(git log --no-merges --format=%s $BASE..HEAD -- crates/candler-schemas)"`; confirms are culled: `! git grep -n 'fn confirm_' -- ':(glob)crates/**'`
- ⬜ [nl] G11 reports: the 6 verifiers closed with no open finding; every Bite is recorded in the leaf reports `__reports__/m6a-paired-stats/leaves/<leaf>_v0.md`; every probe is answered in its report

## Gotchas
- **Statistical design** (R03 §Statistical design; Eliott's four decisions of round 2):
  - **Question.** For each comparison and each gate or quality expectation, does B change the pass rate compared with A? Two-sided. Estimand: B − A within blocks. H0: no difference in any block. No SESOI; every effect is shown with its interval.
  - **Design facts.** The unit is one run (isolated containers; B3 randomizes order). Runs are not paired (replicate r of A and of B share nothing), so McNemar is out. Replicates per arm are fixed, so one margin is fixed and Fisher's exact test is conservative: use its mid-p (Lydersen, Fagerland & Laake 2009, doi:10.1002/sim.3531). Blocks are the level combinations of the declared factors other than the compared one; on the golden design (A8), the model for a version comparison and the version for a model comparison; the eval task is not a block (golden probe, C5).
  - **C6, comparisons.** One comparison per (declared factor, pair of its levels), the other declared factors as blocks, each block's own risk difference shown. A 2×2 store gives 2 comparisons where today it gives 4 one-factor pairs, so the `a2_` guards change by design; `analyze_output` lists each changed guard with its reason. A one-factor store gives one comparison in one block: today's output with the mid-p p.
  - **Battery**, fixed by the experiment file before any data: binary in 1 block, Fisher mid-p with Newcombe; binary in ≥2 blocks, CMH χ² without continuity correction with the MH odds ratio and its RBG interval, the Mantel–Fleiss criterion computed per comparison, homogeneity shown and never a gatekeeper, and the exact conditional test (convolution of the blocks' hypergeometrics) as the named fallback; weighted (num/den), a stratified permutation test of the difference in Σnum/Σden, B = 10 000, p = (b+1)/(B+1), with a stratified BCa interval.
  - **C7, multiplicity.** Holm over one comparison's gate expectations, and separately over its quality expectations. Holm is the only method.
  - **Readings.** "effect" when the interval excludes 0, "inconclusive" when it spans 0. Never "no difference".
  - **I5.** Only gate and quality outcomes are tested. No model call.
- **Decisions** (R03 §Treatment arms): **T1** the factor is `request.treatment`. **T2** `is_setting` is unchanged; `fresh_cells` admits only this setting as a factor, since it resolves into the hashed typed root `treatment`; every other setting still raises `SettingAsFactor`. **T3** one preflight resolve per distinct level in declared order; a failing level fails the plan; each cell and `check_facts` use their own level; a one-treatment file resolves once. **T3 wiring** `JobSpec` keeps `resolution` as the primary level and gains `levels` with `skip_serializing_if = "Vec::is_empty"`; `cells_of` takes each cell's own level. **T4** equal lock bytes give one image through `ImageService::ensure`. **T5** `stratum_verdict` explains env_lock, image and build_inputs only when a declared `treatment` factor differs, mirroring `tools_unexplained`. **N3** is met; residuals go in the design record and `lock_split` gets a Future row. **E1** `ArmReport.instrument_error {runs, outcomes}`, other counts unchanged. **Units** `ExpectationTally` holds sums only; `arm_counts` adds per-run `(num, den)` in a deterministic order. **B3** `plan_runs` iterates one pure `run_order(cells, replicates, seed)` in new `runner/order.rs`; the old loop is removed, no duplicate left. No digest moves.
- **Critical conditions.** C1 `Stratum` and the test result signatures (fixed by the scaffold and the leaf specs); C2 GEN deterministic and offline; C3 per-cell treatment reaches `cells_of`, one-treatment digests and spec bytes unchanged; C4 per-run units exist; C5 the blocks are right (golden probe); C6 and C7 above.
- **Scaffold** (the L0 coordinator's first act, about 15 calls, one commit `test(stats): scaffold the m6a modules and vector sections`): empty `crates/candler-core/src/analysis/{midp,exact_strata,cmh,splitmix,permutation,bca,blocking}.rs` with their `mod` lines in `analysis/mod.rs`; empty `crates/candler-core/tests/it/guard_stats_{exact,cmh,resampling,blocking}.rs` and `guard_known_truth.rs` with their `mod` lines in `tests/it/main.rs`; one marked section per leaf in `gen_stats.py` (`# --- <leaf> ---` with `FILES |= {}`); the `Stratum` type in `analysis/blocking.rs` (`pub struct Stratum { pub key: Vec<(FieldPath, Value)>, pub a: (u64, u64), pub b: (u64, u64) }`, passes and runs per arm); and `SplitMix64` in `analysis/splitmix.rs` complete (`new`, `next_u64`, `below` by rejection, about 20 lines), since both `resampling` (which adds its twin and guard) and `execution_order` (which shuffles with it) need it at L0. After it, no L0 sibling edits a shared append-only line, and no L0 leaf needs a sibling's code.
- **Golden probe** (the L0 coordinator, read-only, about 10 calls, before dispatch): are the golden import's eval tasks separate cells? Read the import path (`import/read.rs`, `eval_dir`) and one imported run's cell. Record the answer in the Progress table and the `blocking` leaf's report; it sets C5.
- **Races.** No sibling pair needs a manual merge. `treatment_setting` (`fresh_cells`, near line 369) and `execution_order` (the `plan_runs` tail, near line 985) share `runner/plan.rs` in hunks about 600 lines apart; they auto-merge and each leaf names the other. `arm_counts` (L0) and `analyze_output` (L2) share `commands/analyze.rs`, sequentially.
- **Traps.** Do not edit `fisher.toml`; add `midp.toml` (the `t1_`, `a1_` and `a2_` oracles read `fisher.toml`). Keep `fisher_two_sided` until `analyze_output` swaps the call.
- **Budget.** Cap $105; the coordinator stops and reports to Eliott at $84 (80 %). Measured from transcripts at the list rates of R05 (Opus 5.5 $4/$20/$5 or $8/$0.20; Sonnet 5.5 $2/$10/$2.5 or $4/$0.10); `spend.py` and `m5b-spend.py` overstate cache reads and are not used. Implementers 1,140 calls × $0.041 = $46.7; verifiers 200 × $0.041 = $8.2; fix rounds 70 × $0.041 = $2.9; coordinators 305 × $0.063 = $19.2 (4 levels × 70, scaffold and probe 25); close $8; contingency 15 % $12.8. A coordinator at 200 calls per level adds about $33: it measures at each level merge and stops early if its own row is over. Each leaf's call estimate is in its Estimate line; at the cap the implementer stops, pushes what is green and lists what is left.
- **Execution model.** A fresh Opus coordinator per level dispatches every leaf of the level at once, each a Sonnet subagent with the model passed explicitly, in its own worktree. It integrates each leaf: rebase onto the feature tip, run the leaf's targeted `[cmd]` gates and ownership check (integration, not verification), `git merge --no-ff -F <file>`, confirm with `git log`. It reviews the seams after each level and handles escalations only; it never verifies its own dispatches. At each level merge it runs STD (workspace form), DIGEST and GOLDEN; at the L1 merge and at the end, LINUX and SUBJ. It batches `dirtree-rdm update` calls per level and re-reads `dirtree-rdm ls` at each level close.
- **Implementer owns its verify loop.** On a `[V]` leaf the implementer, once its gates pass, dispatches a fresh Sonnet verifier with the leaf's Verifier brief verbatim, adding only the branch name, fixes the findings, asks the same verifier to re-check through SendMessage, and reports once. A disputed finding, a spec defect or a second failed re-check goes to the coordinator. **Mechanism probe** (the L0 coordinator, first dispatch): if a Sonnet subagent cannot dispatch Agent, implementers run as Sonnet side sessions that each dispatch their verifier, and their turn-end note is the report. Verifiers (6): `level_wiring` (covering `level_resolve`), `analyze_output`, `resampling`, `cmh`, `blocking`, `import_treatment` (A8).
- **Shared machine.** Each leaf builds in `CARGO_TARGET_DIR=/private/tmp/claude-501/m6a-target-<leaf>` with `RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0`, exported per command, never in a config file; the coordinator deletes it after the leaf merges; at most three leaves build at once. `df -h /System/Volumes/Data` before the first build. No Docker. Never cancel CI. Never rewrite history on main.
- **colgrep.** Index the leaf's worktree and scope every `search` to that path. The search policy denies `grep -r` and `rg`; use `git grep`.
- **Schemas.** No leaf touches `crates/candler-schemas/`. A commit that would is scoped `schemas` and touches no other crate, and needs a coordinator ruling first.
- **Owned files.** Each leaf lists its OWNED files as a regex; its ownership check is `git diff --name-only $BASE_LEAF..HEAD | grep -vE '<owned regex>'`, which must print nothing (`$BASE_LEAF` is the leaf's branch point). No leaf edits `__roadmap__/`, `cog.toml`, `.config/` or `Cargo.lock`; the leaf report under `__reports__/m6a-paired-stats/leaves/` is always owned.
- **Every leaf.** Pre `[cmd]`: the worktree is the leaf's own checkout, `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints its path, and `git merge-base --is-ancestor $BASE_LEAF HEAD`. Post `[cmd]`: the ownership check and the leaf's targeted test. Post `[nl]`: the leaf report records each Bite (the plausible bug that turned each guard red).
- **Test design.** Each guard is written first and seen red; its doc carries a `Guards:` line and an `Oracle:` line. Expected values come from the second engine or from exact arithmetic, never from the code under test. Confirms are written to drive the work and culled before merge.
- **Citations before code.** Each test leaf's first step is a probe that cites the primary source of its method's validity conditions in the leaf report. A source that contradicts the battery is a finding for Eliott, not a change to implement.
- **Behaviour changes stop the implementer**, who reports them to the coordinator before the leaf merges: a changed existing guard (only `analyze_output` may, with reasons), a moved digest, a newly refused or newly admitted input beyond T2.
- **Standard gates** (`$BASE` from the Pre-conditions; `$BASE_LEAF` a leaf's branch point):
  - **STD**: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`; implementers run only their targeted test and `-p` clippy; the coordinator runs the workspace form once per level merge;
  - **LINUX**: `cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings`;
  - **SUBJ**: `cog check <base>..HEAD && git log --format=%s <base>..HEAD | awk 'length>72{b=1}END{exit b}'`;
  - **GEN**: `(cd crates/candler-core/tests/vectors/stats && uv run --offline --no-project --with scipy==1.17.0 --with statsmodels==0.15.0 python gen_stats.py --check)`, measured on 2026-11-18: six files, `rows match`, exit 0;
  - **GOLDEN**: `export CANDLER_GOLDEN_CORPUS=/Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train && cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'test(/guard_golden::/)' && cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'test(/guard_oracle_ac/)'` (read it, never write in it). The golden reference is htn-creator-eval 0.4.2's 240 runs; never `harness/evals/eval.json`, never RiVault Reasoning-Small runs;
  - **DIGEST**: `git diff --numstat $BASE -- crates/candler/tests/oracle crates/candler-core/tests/oracle/vectors crates/candler-schemas/tests/vectors ':(exclude)crates/candler-core/tests/oracle/vectors/field_paths.toml' | awk '$2!=0{b=1}END{exit b}' && cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/guard_cell_digest/)' && cargo nextest run -p candler-core --no-tests=fail -E 'test(/guard_import_mini|guard_legacy_mapping|guard_oracle_mini|guard_run_context|guard_engine|guard_seams/)'`.
- **Defects.** Verify each claim against the code before changing it. Report a spec defect as a finding; do not build on a false premise.

## Status
```mermaid
graph TD
    exact_tests[Exact tests]:::done
    cmh[CMH]:::done
    resampling[Resampling]:::done
    treatment_setting[Treatment setting]:::done
    treatment_verdict[Treatment verdict]:::done
    arm_counts[Arm counts]:::done
    execution_order[Execution order]:::done
    resolve[Resolve]:::inprogress
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `exact_tests.md` | 📄 Leaf Task | ✅ Done |
| `cmh.md` | 📄 Leaf Task | ✅ Done |
| `resampling.md` | 📄 Leaf Task | ✅ Done |
| `treatment_setting.md` | 📄 Leaf Task | ✅ Done |
| `treatment_verdict.md` | 📄 Leaf Task | ✅ Done |
| `arm_counts.md` | 📄 Leaf Task | ✅ Done |
| `execution_order.md` | 📄 Leaf Task | ✅ Done |
| `resolve/` | 📁 Directory | 🔄 In Progress |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A1 | 2026-11-18 | __reports__/m6a-paired-stats/leaves/treatment_setting_v0.md | [] | Supervisor ruling: fresh_cells drops setting paths, so treatment_setting is only the refusal change; the two-digest row moves to level_resolve |
| A2 | 2026-11-18 | __reports__/m6a-paired-stats/leaves/execution_order_v0.md | [] | Supervisor ruling: b27 and b34 pinned declaration order and were rewritten; OWNED gains guard_job_orchestrate.rs for b34; run_order and seed_of are public; the git grep gate uses ! git grep -q |
| A3 | 2026-11-18 | __reports__/m6a-paired-stats/leaves/treatment_setting_v0.md | [] | Scope plan is not in cog.toml: treatment_setting and level_resolve subjects use runner; treatment_verdict subjects cut to 72 characters |
| A4 | 2026-11-18 | __reports__/m6a-paired-stats/leaves/arm_counts_v0.md | [] | Merge pre-conditions test the landed leaf report instead of a merge title, since L0 merge titles name the change |
| A5 | 2026-11-18 | __reports__/m6a-paired-stats/leaves/level_resolve_v0.md | [] | Supervisor ruling: plan_runs has no image step (ensure runs per resolution in resolve/steps/image.rs), so T4 and its t4_ rows move from level_resolve to level_wiring, which owns guard_image.rs; plan_runs takes impl Into<Levels<'a>>, so no existing caller changes, and a level without a resolution fails the plan naming it |
| A6 | 2026-11-18 | __reports__/m6a-paired-stats/leaves/analyze_output_v0.md | [] | Supervisor rulings: G7 is reworded to a two-level one-factor store with binary expectations only, since C7 and the weighted battery change any other store; analyze_output may change a1_ and a2_ guards whose cause is C6, C7, mid-p, the weighted battery or the B − A sign; the treatment verdict keys on request.treatment only (t5_ fixtures respelled); every difference is B − A, None where no pooled interval |
| A7 | 2026-11-18 | __reports__/m6a-paired-stats/leaves/level_wiring_v0.md | [] | Supervisor rulings: a table level on request.treatment is that level's whole treatment, with only the fields name, tag, commit and source (newly admitted); two levels with equal resolved treatments are refused at plan time naming both (newly refused, existing code); the probe shows the treatment commit enters the cell digest, as R01 says |
| A8 | 2026-11-18 | m6a-l3-addendum.md (notes folder of session local_7f577b41), Eliott's approval of the L3 reshape | [ "golden_design", "import_treatment" ] | The 240 golden runs are the trace of a design to rerun, not numbers to refit: golden_blocked is blocked, superseded by golden_design (the design plans 12 cells and analyses against a second engine); import_treatment makes an import declare request.treatment like its native rerun; G2 becomes the golden design gate, G12 the import gate, 6 verifiers; known_truth runs the golden shape end to end; design_record documents the treatment table form and re-reads the offline citations |
| A11 | 2026-11-18 | __reports__/m6a-paired-stats/leaves/import_treatment_v0.md (probe) | [] | Milestone coordinator ruling: the A8 drop rule changes two existing guards and no digest; import_treatment owns guard_legacy_mapping's the_mapped_runs_are_the_oracle_runs (expected factors [plan.model, plan.runs, request.treatment] computed by the jq oracle applying the rule, never from the importer) and guard_comparability's imported_stratum_of_the_mini_corpus_is_comparable (SKILL_FACTORS respelled request.treatment, verdict stays comparable, a flip is a finding); the verifier attacks a changed guard whose expected value came from the code under test |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| `exact_tests.md` | m6a/exact_tests | 5 | merged 50482359. BASE = 9560cd3d6b7b63eca28dc2bc0322ae769bd0d2f2. Golden probe: eval-K dirs merge into one run of a cell, the task is the expectation id, so 12 cells of 20 runs and the golden blocks are the model only (C5). Mechanism probe: AGENT-OK. Degenerate mid-p returns 0.5 |
| `cmh.md` | m6a/cmh | 6 | merged 83f2c45d. Verifier F1-F4 fixed, re-check closed. OR is B over A |
| `resampling.md` | m6a/resampling | 7 | merged e0462cd0. Verifier F1-F6 fixed, re-check closed. BCa needs 2 runs per arm; multi-block BCa has only the twin |
| `treatment_setting.md` | m6a/treatment_setting | 3 | merged 02c50c30. Refusal only (A1); CLI no longer refuses a treatment factor and plans one collapsed cell until level_resolve |
| `treatment_verdict.md` | m6a/treatment_verdict | 3 | merged c1cf6cad. 89 verdict rows unchanged, 3 t5_ rows added |
| `arm_counts.md` | m6a/arm_counts | 3 | merged 45a91016. units are u64, not f64; the insertion-order bite cannot go red |
| `execution_order.md` | m6a/execution_order | 5 | merged e7ca6bc0. b27 and b34 rewritten under ruling (A2) |
