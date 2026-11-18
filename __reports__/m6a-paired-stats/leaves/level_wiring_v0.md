# level_wiring: report v0

## Commits
| sha | subject |
|:----|:--------|
| 00df25ba | test(jobs): guard per-cell treatment at run time |
| 3890a8cd | feat(jobs): run each cell with its own treatment level |
| 9200053b | test(jobs): guard that a one-level spec has no levels key |
| 7325dacf | fix(jobs): apply a one-level treatment factor and code plan refusals |
| (this report) | docs(jobs): record the bites and the verifier of the planning chain |

## Gates
| gate | result |
|:-----|:-------|
| guard_job_orchestrate, guard_job_spec_file, guard_image::t4_, guard_run_plan | PASS, 21 run |
| candler guard_cli_run, guard_cli_refusal | PASS, 22 run |
| DIGEST (numstat, guard_cell_digest 9, core set) | PASS |
| clippy -p candler-core -p candler --all-targets -D warnings; fmt --check | PASS |
| no `fn confirm_` | PASS |
| ownership check | FAIL by design: plan.rs, guard_run_plan.rs (ruling 2) and six test files, see Findings |
| `git diff --quiet $BASE_LEAF -- guard_job_spec_file.rs` | PASS |
| STD workspace, LINUX, SUBJ, GOLDEN | not run (coordinator) |

## Claims checked
- A one-treatment file resolves once and plans the bytes it planned: `t3_one_level_plans_the_old_bytes`, `p0_`, and DIGEST pass; `plan_job` clears `levels` below two resolutions and `t3_one_level_spec_has_no_levels_key` pins the key list.
- `cells_of` takes treatment source, sources and package names from the cell's own level, found by the cell's `treatment` reference (`level_of`). Harness and image come from the cell's comparable, built per level by `fresh_cells`.
- T3 failure: each distinct level is resolved with `?`, so a failing level fails `candler run` before any plan.
- The leaf says `JobSpec.levels` and `commands/supervise.rs`: `supervise.rs` needed no edit, it passes the spec through.
- The leaf's T4 premise (a new build rule) is false in effect: equal lock bytes already gave one build and different bytes two. The `t4_` rows are guards of that existing behaviour and were green on the base.

## Probe: commit in digest
Answer: the treatment commit IS part of the cell digest. Row `guard_run_plan::t3_probe_treatment_commit_in_digest`: two one-level plans differing only in `treatment.commit` give different cell digests (passes).
Code path: `plan.rs::fresh_cells` -> `Comparable::from_resolved` (`candler-schemas/src/execution.rs`), where the resolved path `treatment` is a reserved root read into the typed `TreatmentRef {name, tag, commit}` and hashed whole; `source` lives only in the `request.treatment` setting, which `fresh_cells` filters out (`is_setting`).
This matches `__design__/architecture.md` §Treatment and §Comparable and contradicts the L1 handover sentence "the treatment `commit` is not part of the cell digest". Finding for the coordinator; nothing changed in the digest or schemas.

## Ruling 2
Two declared levels whose resolved `TreatmentRef` (the typed root, as hashed) are equal fail the plan in `fresh_cells`, naming both level values. Error: `PlanError::Cell { ExecutionError::Document { path: "/factors" } }` (L1's existing code); no `codes.toml` row. Guard row: `guard_run_plan::t3_equal_levels_are_refused` (also shows levels differing in commit alone plan). Two checkouts of one name with no tag or commit resolve to equal references and are refused.

## Bites
| bite | injected | red row | message |
|:-----|:---------|:--------|:--------|
| every cell from the primary level | `level_of` always returns `spec.resolution` | `t3_each_cell_runs_its_own_treatment` | `the checkout of v2 / left: Some("/src/a") right: Some("/src/b")` |
| sources of the primary level | `sources_of(written, &spec.resolution, ..)` | same | `the sources of v2 hold its own environments` |
| serialise an empty `levels` | drop `skip_serializing_if` | `t3_one_level_spec_has_no_levels_key` (p0_ stays green: it compares a spec with its own bytes and cannot see it) | key list has `levels` |
| image keyed by something other than the lock bytes | cwd bytes appended to the lock in `ImageIdStep` (the named bite, keying by the level, is unreachable: `ensure` refuses a digest that is not the lock's sha256) | `t4_equal_locks_one_build`, `t4_distinct_locks_two_images` | `equal lock bytes give one lock and one id`; `the first lock's image left: 0 right: 1` |
| skip the equal-levels check | `equal_levels` result dropped | `t3_equal_levels_are_refused` | no refusal |
| per-level `record_packages` | not injected: `PackageReading` is `non_exhaustive`, tests cannot build readings | none | see Left |
| probe | not a guard to bite | | |

## Verifier
Round 1 (spec-verifier, Sonnet, 28 calls): VERIFY_DEVIATION, no path found where a cell mounts or records another level's treatment through `cells_of`, `level_of`, `check_facts` or `fresh_cells`.
| finding | disposition |
|:--------|:------------|
| F1 a one-level treatment factor was never applied (`run.rs` resolved the file's own `[treatment]`) | fixed in 7325dacf: every declared level is injected |
| F2 equal or unresolved level surfaced as a generic failure from `experiment_of` in `plan_job` | fixed in 7325dacf: mapped through `refuse_plan` |
| F3 ownership check prints plan.rs, guard_run_plan.rs and six literal files | accepted: ruling 2 owns the first two; the six are Finding 2 |
| F4 `sources_of` per level unguarded (mutant survived) | fixed: fake levels carry their own sources; mutant now red. `record_packages` stays unguarded (Left) |
| F5 `t4_` rows cannot go red on the named bite | recorded in Bites and Findings |
| F6 extras in `Levels` ignored; job-wide settings read from the primary level | no leak found; informational |
Re-check of F1, F2, F4 was requested from the same verifier; its answer was not back when this report was committed, so the verdict is open until the coordinator reads it (not fabricated here).

## Findings
1. Probe contradicts the L1 handover: the commit enters the digest (above). Handover and any note saying levels must differ in `tag` are wrong; commit alone suffices.
2. Ownership extensions the change forced, none altering a guard's meaning:
   - `runner/plan.rs`: `experiment_of` takes `impl Into<Levels>` (it built a single-level `Levels`, so any multi-level file failed with "no resolution" in `run_job` and `plan_job`); `treatment_levels` is `pub`; the ruling-2 check.
   - `levels: Vec::new()` added to the `JobSpec` literals of `guard_egress_setting.rs`, `guard_session.rs`, `guard_cli_kill_live.rs`, `guard_cli_observe.rs`, `support/job_orchestrate.rs`, `support/job_runtime.rs`. A new field on a non-`non_exhaustive` struct breaks every literal.
3. The leaf's T4 bite is unreachable and no T4 rule was missing: the `t4_` rows are green on the base. They guard the PATH-fake counts only.
4. `candler run` with several levels is not exercised through the CLI (no Docker allowed); `input_for_level` and the per-level resolve loop are covered by reading only. A level must carry its own `source`, since each level replaces the whole `request.treatment` setting.
5. `--set request.treatment=X` beats every level, so all levels resolve to X and ruling 2 refuses the plan (coded refusal after the fix).
6. Job-wide settings (`model.endpoint`, `model.token_secret`, harness choice in `supervise.rs`) are read from the primary level; none depends on the treatment.

## Left
- Per-level `record_packages` has no guard (readings cannot be built from tests); it is filtered in `cells_of` by the cell's own level's names.
- A CLI-level row for a two-level file needs Docker or a fake that `guard_cli_run` does not have.
- STD workspace, LINUX, SUBJ, GOLDEN for the coordinator.
