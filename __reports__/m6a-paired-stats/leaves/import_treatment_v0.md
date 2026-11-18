# import_treatment: leaf report

## Probe
What the treatment rule can move, read before the change.
- **Declaration today.** `map_runs` in `import/manifest.rs` declared every value path that varies across the imported cells (`varying_paths`). On the mini-corpus that is `plan.model`, `plan.runs` and the three spellings of the skill version (`plan.skill_version`, `provenance.skill_version`, `resolved_skill_version`); `plan.out_dir` is filed under `import`, not declared.
- **Pinned factor lists.** One row pins an imported factor list: `guard_legacy_mapping::the_mapped_runs_are_the_oracle_runs` (A11 names it, and it is the first changed row below). `guard_comparability::imported_stratum_of_the_mini_corpus_is_comparable` declares the skill factors by hand and is the second (A11). `guard_import_mini` pins no factor list and no manifest bytes; its snapshot rows compare an import with another import of the same code.
- **Digests.** Factors are not a member of `Comparable`, so no cell digest reads them; `experiment_digest` is built from cell digests only. Nothing in `crates/candler-schemas/tests/vectors`, `crates/candler-core/tests/oracle/vectors` or `crates/candler/tests/oracle` changes: DIGEST's numstat check is empty and `guard_cell_digest`, `guard_import_mini`, `guard_legacy_mapping`, `guard_oracle_mini`, `guard_run_context`, `guard_engine`, `guard_seams` pass. No stop-and-report.

## Changed rows
| row | old | new | cause |
|---|---|---|---|
| `guard_legacy_mapping::the_mapped_runs_are_the_oracle_runs` (factor list) | `[plan.model, plan.runs, plan.skill_version, provenance.skill_version, resolved_skill_version]` | `[plan.model, plan.runs, request.treatment]` | The three cells' treatments differ and the three skill spellings map one to one onto them. Computed by the jq rule embedded in the row's doc comment over the three `experiment.json` files, not by the importer. |
| `guard_comparability::imported_stratum_of_the_mini_corpus_is_comparable` (`SKILL_FACTORS`) | `[inputs, plan.skill_version, provenance.skill_version, resolved_skill_version, treatment]` | `[request.treatment]` | The importer declares the treatment for the skill version; the verdict stays `Comparable` (A12 drops the three mapped paths over all arms). |

New A12 rows (no existing verdict row changed; `verdicts.json` and every other fixture are byte-identical): `live_stratum_tracking_the_treatment_is_refused`, `imported_stratum_with_a_path_that_does_not_map_one_to_one_is_refused`.

## Gates
- Targeted `guard_import_mini`, `guard_legacy_mapping`, `guard_comparability` (the verdict table holds the t5_ rows): PASS, 25 of 25.
- DIGEST: PASS (numstat empty; `guard_cell_digest` 9 of 9; the import and engine guards 47 of 47).
- Golden row `declared_golden_factors`: PASS (twice, last on the committed tree).
- clippy `-p candler-core -p candler --all-targets -D warnings`: PASS. fmt: PASS. Ownership check: prints nothing. No `fn confirm_`.

## Bites
Each injected, seen red, restored from a copy of the file.
- Importer keeps declaring the mapped value paths: red on `the_mapped_runs_are_the_oracle_runs` (got `[plan.model, plan.runs, plan.skill_version, provenance.skill_version, request.treatment, resolved_skill_version]`) and `it_two_versions_declare_treatment` (got `plan.model, plan.skill_version, provenance.skill_version, request.treatment, resolved_skill_version`).
- Declares the treatment on a one-version import: red on `it_one_version_unchanged` (got `[plan.model, request.treatment]`, want `[plan.model]`).
- Drops a path that does not map one to one (levels count removed from `maps_one_to_one`): red on `imported_stratum_with_a_path_that_does_not_map_one_to_one_is_refused` (got `Comparable`) and `the_mapped_runs_are_the_oracle_runs` (got `[request.treatment]`).
- A12: drop the all-imported condition (apply the rule to live arms): red on `live_stratum_tracking_the_treatment_is_refused` (the three skill paths vanish from the refusal).

## Verifier
`spec-verifier` (Sonnet, 19 calls): PASS with findings, none blocking.
- The two changed guards take their expectations from the jq oracle and the unchanged verdict, not from the code. The live branch, `residual`, `declared_covers`, `treatment_explains` and `TREATMENT_EXPLAINED` are untouched; the new retain runs only in the all-imported branch, before the import gate paths are added, over every arm.
- Finding 1 (low): with only two distinct treatments among the arms any differing undeclared value path maps one to one, so a hand-edited two-arm imported stratum that omits a factor is explained. Inherent in the A12 wording; the importer declares every non-mapping path over all cells, so importer-written manifests are unaffected. No two-arm row exists.
- Finding 2 (low): the importer keys levels with `Value::to_string`, the verdict with JCS; they differ only for numerically equal spellings (`1` and `1.0`). Golden paths are strings.
- Finding 3 (low): the importer keys the treatment as `<name>@<tag>`, the verdict as JCS of the typed record; they agree unless a name contains `@`.
- Deviation: `declared_golden_factors` reads the factors comparisons are made on, since the analyze report carries no manifest factor list; the manifest list is pinned by `the_mapped_runs_are_the_oracle_runs` on the mini corpus.
