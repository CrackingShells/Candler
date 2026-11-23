# Inner refusal codes: build record

## Probe
R16 says 14 inner codes are shadowed by `*.document_invalid` on the file-read path. On the tip the count is 8, not 14. Defect for the coordinator.

| Variant | Code | On the file-read path today | Pointer after the change |
|:--|:--|:--|:--|
| `ExecutionError::Zero` | `experiment.count_zero` | shadowed (raised in `ExperimentFile::new`, inside serde's `try_from`) | prints its code with a pointer: `/replicates` or the budget field |
| `NotPositive` | `experiment.cost_not_positive` | shadowed | prints its code with a pointer: the budget field (`/budget/per_run/max_cost_usd`) |
| `NoLevels` | `experiment.factor_without_levels` | shadowed | prints its code, no pointer (the old `Document` pointer was the root, so none is lost) |
| `EmptyTable` | `experiment.table_empty` | shadowed | prints its code, no pointer (root before) |
| `PathOverlap` | `experiment.path_overlap` | shadowed | prints its code, no pointer (root before) |
| `Duplicate` | `experiment.entry_repeated` | shadowed | prints its code, no pointer (root before) |
| `InputDeclaration` | `experiment.input_invalid` | shadowed, pointer came from serde's nested path | prints its code with a pointer: `/inputs/N` (the variant carries the entry index) |
| `ReservedRoot`, `ReservedValue`, `AbsolutePath` | `experiment.reserved_root_misused`, `reserved_value_malformed`, `absolute_path` | not on the file-read path: raised only by `Comparable::from_resolved` (cell building at resolve), already printing their own codes | no change |
| `EvaluationError::DuplicateId`, `UnknownDependency`, `Cycle` | `suite.id_repeated`, `dependency_unknown`, `dependency_cycle` | not shadowed: `SuiteFile::from_toml_str` reads the plain shape and then runs `SuiteFile::try_from`, so they already print | no change (rows kept in the test as regression rows) |
| `EvaluationError::EmptyList` (`applies_to = []`) | `suite.list_empty` | shadowed: raised by the entry's `try_from` nested in the list reader | prints its code with a pointer: `/expectations/N` (the variant carries the entry index; the suite's entries are validated after the parse) |

Answer 2a: variants raised in the file's outer validator never had a pointer (serde's path was the root, which the table prints as none); `Zero`, `NotPositive` and `InputDeclaration` carry one through `execution_path`. No refusal that had a pointer lost it; every refusal that had a pointer keeps it, including `input_invalid` at `/inputs/N` and `suite.list_empty` at `/expectations/N`.

Answer 2b: the `Deserialize` of `ExperimentFile` and `SuiteFile` still goes through `try_from`, so `read_value::<ExperimentFile>` and every JSON reader of the type still validate. Only `ExperimentFile::from_toml_str` changed: it reads `RawExperimentFile` and calls `ExperimentFile::try_from`, the way the suite reader already does. `RawExperimentFile.inputs` holds `RawExperimentInput` so the input validator runs after the parse; its schema type is stated as `Vec<ExperimentInput>` with `default = []`, and `schema_drift` shows no schema file moved. No reader loosened.

A format, value or canonical fault of the document (`Format`, `Values`, `Canonical` out of `new()`) is mapped back to `Document`, so the 25 `format.*`/`canonical.*` rows stay behind `document_invalid` as R16 says. Without the mapping they would print their own codes: a behaviour change beyond the leaf.

## Guards changed
| Test | Change | Reason |
|:--|:--|:--|
| `guard_experiment_file::overlapping_or_empty_experiments_are_refused` | `Document` becomes `PathOverlap` (3 cases) and `NoLevels` (1) | asserted the defect |
| `guard_experiment_file::table_value_empty_and_overlapping_tables_are_refused` | two `Document` become `PathOverlap` | asserted the defect |
| `guard_refusal::inner_codes_print_as_code` | new | confirm |
| `guard_refusal_codes` (candler-core) | two `EvaluationError::EmptyList` constructions gain `entry: None` | the variant gained a field; no expectation changed |

No test of `candler-core` or `candler` asserted `document_invalid` on a value-invalid file; the targeted tests of both crates pass unchanged, so there is no `test(schemas): expect the inner code ...` commit for them. The two Notes lines changed (`experiment-file.md`, `suite-file.md`, stamp `v@0.13.0`); no generated section moved.

## Bites
| Bite | Result |
|:--|:--|
| `NoLevels` mapped back to `Document` | red: `ExecutionError::NoLevels` left `experiment.document_invalid`, right `experiment.factor_without_levels` |
| `Zero` loses its pointer in `execution_path` | red: `pointer of ExecutionError::Zero` left `None`, right `Some("/replicates")` |
| registry key `experiment.factor_without_levels` renamed locally in `codes.toml` | red: left `experiment.factor_without_levels`, right `experiment.factor_without_levelz`; the expectation comes from the registry |

All three reverted. The hard-coded variant of the third bite was not run: a literal in the test would stay green, which is what reading the registry prevents.

## D3
`inner_codes_print_as_code`, role confirm.
1. Question: does a file with a factor without a level print `experiment.factor_without_levels` and a suite with a repeated id `suite.id_repeated`?
2. Why no existing test answers it: the existing tests asserted `Document` for these files (the defect).
3. Sustainable: one table-driven test, 11 rows, one registry read; no model, no Docker.
4. Scalable: a new inner code adds a row (enum, variant, file, pointer); the code and side come from the registry row of that variant.

Keep or cull (R14): keep. It names a relation the registry promises (code, side, pointer per variant) and stays valid when a variant is added. The three suite rows that were green on the tip are regression rows of the same relation.

## Verifier
One fresh Sonnet verifier, re-checked after the fixes of findings 1 and 2 (result below).

| # | Finding | Disposition |
|:--|:--|:--|
| 1 | `input_invalid` path coarsens from `/inputs/0` to `/inputs` | closed in the `fix(schemas):` commit that follows the breaking one: the variant carries the index; the row of the test asserts `/inputs/1` (red before, seen) |
| 2 | `suite.list_empty` stays `document_invalid`, no inner code in `causes[]` | closed in the same commit: the variant prints `suite.list_empty` at `/expectations/N`; the row asserts it (the row expected the new code before the fix; its red run was not captured separately) |
| 3 | R16's 14 is 8 on the tip | recorded in Probe and Follow-ups |
| 4 | `Format`/`Values`/`Canonical` remapped to `Document` with `detail` from `to_string()` | accepted: keeps the 25 rows behind `document_invalid` as ruled; no test pins `detail` |
| 5 | Bite 3's hard-coded half not run; lookup is by (enum, variant), not by key | accepted: the literal would stay green; the registry is still the source |

Re-check of the two fixes: findings 1 and 2 closed; five new findings.

| # | Finding | Disposition |
|:--|:--|:--|
| A | `e5d52d84` adds a field to two public variants but is `fix(schemas):`, not `!`; history does not bisect between it and the core test commit | accepted by the coordinator: the `!` on `327cb73f` already makes the schemas release breaking, `e5d52d84` ships in the same release and its body carries the migration; no history is rewritten; the one-commit compile gap between `e5d52d84` and `a7fddbed` is accepted |
| B | `suite-file.md:66` said an empty `applies_to` is `document_invalid` | closed in the `docs(schemas)` commit after the report: the line says `suite.list_empty` at `/expectations/0`; `contract_pages` rerun |
| C | a canonical fault of a JSON suite entry is rebuilt as `EvaluationError::Toml` (was `Json`) | closed in `bc096619`: `SuiteFile::from_json_str` rebuilds it as `Json`; the verifier re-checked it |
| D | no test row for the canonical-fault mapping or an index above 0 | closed in `bc096619`: a row for a JSON suite whose entry 1 has an out-of-range integer, expecting `suite.json_invalid` at `/expectations/1`, seen red before the fix |
| E | the suite row's red run before the fix was not captured | accepted, recorded |

Re-check of C and D: closed. Two minor gaps left open: the test's reader tag is a string whose fallback arm reads the experiment file, and the TOML counterpart of the canonical entry fault has no row.

Everything else the verifier attacked came back clean (commit shape, registry unchanged, one validator, JSON readers validating, no changed expectation that did not assert the defect, no generated section edited).

## Follow-ups
- R16's count (14) is 8 on the tip: three experiment variants are raised at resolve, three suite variants already print. R16's blast-radius text and the `codes_review` M6 table should say 8.
- The `fix(schemas):` commit adds a field to two public variants, so its migration note is in its body; the `fix(schemas)!` commit stays the one the one-crate gate finds. Between that commit and the `test(core)` commit that follows it the core guard does not compile.
- The 25 `format.*` and `canonical.*` rows stay behind `document_invalid`.
- `ReservedRoot`, `ReservedValue` and `AbsolutePath` have no test row: no file reaches them in `candler-schemas`.
