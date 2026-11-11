# Wire leaf: report v0

The wire leaf of M2c is implemented on `m2c/wire` (cut from `feat/m2c-wire-alignment` at 2343ecd). This is the version after fix round 1. Only `crates/candler-schemas/` changed. `candler-core` and `candler` compile untouched. No existing digest moved and no existing vector file has a `-` line. One piece of the spec cannot hold as written (the `guard_cell_digest` bite, finding F1).

## 1. Commits

| sha | subject |
|:----|:--------|
| 89aa1e2 | `test(schemas): pin the cells of experiment files with no new field` |
| c085ddb | `feat(schemas): declare inputs, table values and a treatment source` |
| 62251d8 | `test(schemas): keep the schemas' unit tests free of domain names` |

The baseline commit touches no `src/` file, so the recorded digests come from the b017d31 code.

## 2. Gates

| gate | result | last relevant line |
|:-----|:-------|:-------------------|
| only the schemas crate changed (OWNED) | PASS | `git diff --name-only` lists no path outside `crates/candler-schemas/` |
| three commits in order | PASS | `test(schemas),feat(schemas),test(schemas)` |
| workspace builds unchanged | PASS (check only) | `cargo check --workspace --all-targets`: Finished |
| workspace `nextest` | BLOCKED | not run: the brief leaves `--workspace` to the coordinator, and SPAWN was not run |
| W1 `table_value_` | PASS | 4 passed |
| W2 `unchanged_cells_` | PASS | 1 passed |
| W3 `input_list_digest_` and `gen_input_list.py --check` | PASS | 2 passed; exit 0 |
| existing digest binaries | PASS | 26 passed (`guard_cell_digest`, `guard_execution_wire`, `guard_experiment_file`) |
| no `-` line in existing vectors | PASS | awk exits 0; the only edit to an existing vector adds 4 lines |
| DRIFT | PASS | bless run, `git status` clean after the commit |
| evaluator helper models | PASS | `gen_models.py` leaves `src` clean |
| no domain name in schemas' `src` | PASS | the grep finds nothing |
| STD, `-p candler-schemas` | PASS | fmt, clippy `-D warnings`, 198 tests, doc `-D warnings` |
| LINUX | PASS | clippy for `x86_64-unknown-linux-gnu`, workspace tests: Finished |
| SUBJ | PASS | `cog check`: No errored commits; every subject at most 72 characters |
| `Cargo.lock` | PASS | unchanged |
| GOLDEN, SPAWN | not run | no core `src/` change, no process spawned |

## 3. Guards

All were written first and seen red. W3 was red as a compile error (the API did not exist). W1 was red on the assertions. W2 is a baseline guard, green by construction before the change, so its bite is the evidence.

| id | Guards line | oracle | bite and the failure seen |
|:---|:------------|:-------|:--------------------------|
| W1 `table_value_fixed_folds_into_one_object` | a table at or below `prompt.values` reaches the cell as one object, nested or quoted | the cell written by hand from the TOML | no table-valued path: `left: None, right: Some({"a":1,"b":{"c":"x"}})` |
| W1 `table_value_factor_levels_fold_per_cell` | factor levels at or below it fold per cell, merged with `[fixed]` | the four cells written by hand | no table-valued path: `left: [None, None, None, None]` |
| W1 `table_value_undeclared_paths_still_flatten` | any other table still flattens | the cell written by hand | folding at undeclared paths (the const lists `plan.values` and `prompt.other`): `table_value_undeclared_paths_still_flatten` fails, `left: None, right: Some(1)` |
| W1 `table_value_empty_and_overlapping_tables_are_refused` | empty table, duplicate leaf and overlapping leaf refused; disjoint leaves merge | the prefix rule, worked by hand | no table-valued path: the merge assertion fails (`right: Some({"a":1,"b":1})`) |
| W2 `unchanged_cells_keep_their_digests` | a file with no new field lists the cells it listed before | digests recorded from the b017d31 code before the change | factor product reordered: `cont_train: the cells moved`. Folding at undeclared `plan`: same message |
| W3 `input_list_digest_matches_the_second_engine` | the input digest is the sha256 of the sorted RFC 8785 list | `gen_input_list.py`, a Python engine running no Candler code | sort removed: `several_files_out_of_order`, `left: sha256:d6ec3a1f…`, `right: sha256:024bf0ec…`. Sort by UTF-16 code units: `code_point_order_differs_from_utf16_order`, `left: sha256:ff7d572b…`, `right: sha256:14bd9c7c…` |
| W3 `input_list_digest_refuses_empty_and_repeated_lists` | an empty input and a repeated path have no digest | the `refused` cases of the generator | not bitten separately (no mutation of the refusals was run) |

Extra guards, written for the new wire, each with a hand-written oracle:

| id | what it holds | bite seen |
|:---|:--------------|:----------|
| `declared_inputs_are_read_and_the_forbidden_ones_refused` | defaults, evaluators, six refusals | not bitten |
| `treatment_source_is_read_and_left_out_of_the_reference` | `reference()` drops `source` | not bitten |
| `evaluation_inputs_are_optional_and_sorted` | absent when empty, unsorted or repeated refused | `skip_serializing_if` removed: `assertion left == right failed` (guard_execution_wire.rs:705) |
| `derived_source_carries_an_optional_location` | `from` round-trips, absent when `None`, `null` refused | `skip_serializing_if` removed: `assertion left == right failed` (guard_wire_shapes.rs:217) |
| `object_not_list` rows for `TreatmentSpec`, `ExperimentInput`, `InputEntry` | map-only wire | not bitten |

The "no digest moves" bite (F1) is the one the leaf names differently.

## 4. Probe answers

None in this leaf.

## 5. Findings

| id | kind | finding |
|:---|:-----|:--------|
| F1 | spec defect | The leaf says removing `skip_serializing_if` from `evaluation_inputs` or from `Derived.from` "turns `guard_cell_digest` red". It cannot: both fields lie outside `comparable`, the only part the cell digest hashes. I ran both mutations. `guard_cell_digest` stayed green, and the guards that went red are the two new wire guards in the table above. The no-move property of the digest is instead held by W2 and the existing digest binaries; the manifest bytes can change without a digest moving. I did not write a guard that pretends otherwise. |
| F2 | deviation | The fold into one object lives in `ExperimentFile::new` (`fold_fixed`) and in the per-level fill (`fold_tables`), not in `flatten_fixed`/`flatten_into`. `flatten_into` still lists leaf keys. Reason: a file read from JSON, and a serialised file read back (the job spec carries one), must fold the same way as one read from TOML. The observable behavior is the leaf's. The overlap footprint is the set of leaves, so a factor at `prompt.values.a` merges with `[fixed]` at `prompt.values.b`, while one leaf given twice is refused. |
| F3 | spec gap | The leaf names no file for the `evaluation_inputs` row of the cell-digest classification. `guard_cell_digest` refuses an unclassified manifest path, so I added `[[path]] name = "evaluation_inputs", class = "nuisance"` to `tests/vectors/manifest/hashed_paths.toml` (4 added lines, no `-` line). It states that the list is outside the hash. |
| F4 | spec gap | `crates/candler-core/tests/support/job_orchestrate.rs` holds no inline experiment text (it calls `run_plan::experiment`), and the text of `guard_cli_kill_live.rs` equals `run_plan`'s one-model text. The baseline holds four entries: `run_plan_one_model`, `run_plan_two_models`, `cli_run_fixture` and `cont_train` (as a file). Their 16 cell digests are recorded. |
| F5 | shape of the API | `ExperimentInput` has private fields: read `kind()`, `path()`, `at()`, `audience()`, and build with `ExperimentInput::new(kind, path, at, audience)`, which refuses the forbidden `at` and kind. `ExperimentFile::new` keeps its arity and takes `impl Into<TreatmentSpec>` (`TreatmentRef` converts); inputs go in with `with_inputs`. The manifest builder method is `evaluation_input(DigestRef)` (sorted at `build`). `TreatmentSpec` keeps public fields. Names in the shared constraints (`TABLE_VALUED_PATHS`, `TreatmentSpec`, `[[inputs]]` keys, `evaluation_inputs[] {kind, digest}`, `Derived.from`) are exact. |
| F6 | notice for `treatment_source` | `ExperimentFile::treatment()` now returns `&TreatmentSpec`. The binary's `json_of(file.treatment())` is generic and compiles, but once a file sets `source`, `request.treatment` carries it. The identity step must read a `TreatmentSpec` and call `reference()`. |
| F7 | request | An input's `path` is not validated (an empty or absolute `path` is accepted); the leaf asks only for `at`. The reader of the declared inputs must refuse them, or Eliott can ask for a wire rule. |
| F8 | note | A scalar written at `prompt.values` itself (not a table) stays a plain value. Folding at an undeclared path went unseen by W2 until the bite used `plan`, because a lone scalar at its own path does not fold. |
| F9 | not run | GOLDEN, SPAWN and the workspace `nextest` were not run; no `src/` of another crate changed and the coordinator runs the workspace form per merge. The wire verifier is not part of this report. |
| F10 | fix round 1 | The W3 vectors gained a case with a non-BMP path (U+1F600) beside a high-BMP one (U+FF5E), where code point order and UTF-16 order differ; it is folded into the `feat(schemas)` commit. Every gate was re-run in the target dir `/private/tmp/claude-501/m2c-target-wire`; the earlier results were not trusted. |
| F11 | accepted, no change | A hand-written JSON `fixed: {"prompt.values": {}}`, valid at b017d31, is now refused (`EmptyTable`). The coordinator accepts it and records it in R01 through `design_record`. |
