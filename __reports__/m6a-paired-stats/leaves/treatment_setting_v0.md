# Treatment setting: leaf report

Leaf `treatment_setting`, branch `m6a/treatment_setting`, branch point `c39c26fc`. The leaf is narrowed
to the refusal change (see Findings).

## Commits
| sha | subject |
|:----|:--------|
| cff48395 | test(runner): guard treatment as the one setting factor |
| 06a44f69 | feat(runner): admit the treatment as a declared factor |
| report commit (branch tip) | docs(runner): record the bites of the treatment factor |

## Refusal rows
Settings the existing refusal rows use, listed before the edit:
| row | setting |
|:----|:--------|
| `guard_settings_out_of_cell::settings_out_of_cell_setting_as_factor_is_refused` | `isolation.build.prune` (value, parent `isolation.build`, parent `isolation`), `model.token_secret` (parent `model`), `budget.max_runs` (parent `budget`) |
| `guard_cli_refusal::r2_a_setting_as_factor_prints_as_a_record` | `slots.count` (through `vary_a_setting`) |
| `guard_cli_refusal::r3_factor_refusal_precedes_docker` | `slots.count` (through `vary_a_setting`) |

No existing row uses a `request.*` setting as a factor. New rows: `t2_other_settings_stay_refused` covers
`request.harness`, `request.inputs`, `replicates`, `budget.max_runs`, `store.root`, `plugins.dirs`,
`slots.count`, `slots.evaluators`, `isolation.record_packages`, `model.token_secret`,
`isolation.build.prune`, and the `request` parent table filling `request.treatment`.

## Bites
| bite | red on | message |
|:-----|:-------|:--------|
| leave the refusal in place (before Step 2) | `t2_treatment_is_the_one_admitted_setting` | `request.treatment` given as a factor was refused as a setting: Some(SettingAsFactor { path: "request.treatment" }) |
| admit every `request.*` path | `t2_other_settings_stay_refused` | `request.harness` given as a factor: ... was planned instead of refused |
| admit by prefix `request.treat` | none | see below |
| a bite that turns `t2_one_treatment_digest_unchanged` red | none plausible | see below |

- Prefix bite: `is_setting` names no path that starts with `request.treat` other than `request.treatment`
  itself, and `request.treat` / `request.treatmentx` are not settings, so `fresh_cells` never refused them. A
  lookalike row cannot be red for a refusal that does not exist, and the prefix change alters no outcome.
  The coordinator's lookalike row was therefore not written (its premise is false).
- Digest bite: the one-treatment file declares no treatment factor, and the change reads only the factors, so
  no plausible edit of `setting_of_factor` moves its digest. The literal `sha256:f726a430...b255` was computed
  on `$BASE_LEAF`'s code (run twice in different temp dirs; the first run printed it) and the row passes
  before and after Step 2.

## CLI preflight
`commands/preflight.rs::check_factors` has no list of its own: it calls `experiment_of`, so it reaches
`fresh_cells` and `setting_of_factor`. It therefore **no longer refuses** a treatment factor at the CLI,
which differs from the leaf's premise that it still would. Interim state, closed by `level_resolve` and with no
guard here: a library (or now CLI) caller of `plan_runs` with two treatment levels gets one collapsed cell
(a throwaway probe printed `cells=1`), because the level never reaches the cell. The CLI refusal rows
`r2_` and `r3_` pass: they use `slots.count`.

## Gates
| gate | result |
|:-----|:-------|
| targeted `guard_settings_out_of_cell::` + `guard_run_plan::` | PASS: 8 passed |
| `guard_cli_refusal::r[23]_` | PASS: 15 passed |
| `t2_` rows | PASS: 3 passed |
| DIGEST (numstat, `guard_cell_digest`, import/oracle/engine/seams) | PASS: 0 changed lines, 9 passed, 45 passed |
| clippy `-p candler-core --all-targets -D warnings`, `cargo fmt --check` | PASS |
| ownership check | PASS: prints nothing |
| no `fn confirm_` | PASS |

GEN not run (no vectors touched).

## Claims checked
- "`fresh_cells` places the level under the typed root `treatment`": false (see Findings).
- "`is_setting` is not touched": true.
- "A factor on the `request` parent table is still refused": true for a table level that fills
  `request.treatment` with a plain value; a level that fills `request.harness` as a table is not reached by
  `setting_in` today (its leaf is `request.harness.name`, not a setting), unchanged here.
- "`check_factors` still refuses a treatment factor": false.

## Findings
1. Spec defect, ruled by the milestone coordinator: the leaf's commit subjects use scope `plan`, which is not in
   `cog.toml`. Scope `runner` is used for all three, the rest of each subject unchanged.
2. Spec defect, ruled by the milestone coordinator (option 2, narrowing): `fresh_cells` drops every `is_setting`
   path from `effective`, so the level of a treatment factor cannot reach the cell at this leaf, and the
   per-level cell row (differing `treatment` roots) moves to `level_resolve`. This leaf is only the refusal
   change.
3. The lookalike row and the prefix bite cannot be red (see Bites).
4. `check_factors` admits a treatment factor now (see CLI preflight).
5. Machine load was 23 when the first build of the gate batch started, above the brief's 12; the earlier builds
   ran at about 6. No effect on results.
