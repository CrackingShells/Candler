# import_treatment: leaf report

## Probe
What the treatment rule can move, read before the change.
- **Declaration today.** `map_runs` in `import/manifest.rs` declared every value path that varies across the imported cells (`varying_paths`). On the mini-corpus that is `plan.model`, `plan.runs` and the three spellings of the skill version (`plan.skill_version`, `provenance.skill_version`, `resolved_skill_version`); `plan.out_dir` is filed under `import`, not declared.
- **Pinned factor lists.** One row pins an imported factor list: `guard_legacy_mapping::the_mapped_runs_are_the_oracle_runs` (A11 names it, and it is the first changed row below). `guard_comparability::imported_stratum_of_the_mini_corpus_is_comparable` declares the skill factors by hand and is the second (A11). `guard_import_mini` pins no factor list and no manifest bytes; its snapshot rows compare an import with another import of the same code.
- **Digests.** Factors are not a member of `Comparable`, so no cell digest reads them; `experiment_digest` is built from cell digests only. Nothing in `crates/candler-schemas/tests/vectors`, `crates/candler-core/tests/oracle/vectors` or `crates/candler/tests/oracle` changes: DIGEST's numstat check is empty and `guard_cell_digest`, `guard_import_mini`, `guard_legacy_mapping`, `guard_oracle_mini`, `guard_run_context`, `guard_engine`, `guard_seams` pass. No stop-and-report.
