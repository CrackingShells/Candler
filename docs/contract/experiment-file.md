# Experiment file

## Surface

The experiment file an experimenter writes to fix the values, factors, replicates, budget, harness and treatment of a set of runs; Candler reads it to plan and run the cells.

## Version

<!-- generated:version -->
Wire tag: `experiment/1`.

<!-- versions -->
Wire line: `schemas@0.6.0`.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
| Path | Type | Class | Default |
|---|---|---|---|
| `budget` | `Budget` | required | - |
| `budget.max_cost_usd` | `number` | optional | - |
| `budget.max_parallel` | `integer` | optional | `1` |
| `budget.max_runs` | `integer` | optional | - |
| `budget.max_tokens` | `integer` | optional | - |
| `budget.per_run` | `PerRunCaps` | required | - |
| `budget.per_run.max_cost_usd` | `number` | optional | - |
| `budget.per_run.turns` | `integer` | optional | - |
| `budget.per_run.wall_s` | `integer` | required | - |
| `budget.wall_s` | `integer` | optional | - |
| `factors` | `array of Factor` | optional | `[]` |
| `factors[].levels` | `array of any` | required | - |
| `factors[].path` | `FieldPath` | required | - |
| `fixed` | `map` | optional | `{}` |
| `harness` | `HarnessRequest` | required | - |
| `harness.name` | `Name` | required | - |
| `harness.version` | `string` | optional | - |
| `inputs` | `array of ExperimentInput` | optional | `[]` |
| `inputs[].at` | `string` | optional | - |
| `inputs[].audience` | `"agent" \| "evaluators"` | optional | `"agent"` |
| `inputs[].kind` | `Name` | required | - |
| `inputs[].path` | `string` | required | - |
| `replicates` | `integer` | required | - |
| `schema` | `"experiment/1"` | required | - |
| `secrets` | `array of SecretName` | optional | `[]` |
| `treatment` | `TreatmentSpec` | required | - |
| `treatment.commit` | `GitCommit` | optional | - |
| `treatment.name` | `Name` | required | - |
| `treatment.source` | `string` | optional | - |
| `treatment.tag` | `string` | optional | - |

<!-- /generated:fields -->

## Refusals

<!-- generated:refusals -->
| Code | When | Path at fault | Side to fix |
|---|---|---|---|
| `canonical.integer_out_of_range` | An integer lies outside the range JSON numbers hold exactly, plus or minus 2^53 - 1. | - | experiment |
| `canonical.key_repeated` | A JSON object repeats a key. | - | experiment |
| `canonical.lone_surrogate` | A JSON text holds an unpaired surrogate escape. | - | experiment |
| `canonical.number_not_finite` | A number is not finite. | - | experiment |
| `canonical.syntax_invalid` | A text is not valid JSON. | - | experiment |
| `experiment.absolute_path` | A value holds an absolute path as a string, and a file enters a cell as an input digest. | - | experiment |
| `experiment.cell_value_invalid` | A cell value cannot be used by the harness driver. | - | experiment |
| `experiment.cell_value_missing` | A cell lacks a value that the harness driver needs. | - | experiment |
| `experiment.cost_not_positive` | A cost cap is not a finite number above 0. | The cap under `/budget`. | experiment |
| `experiment.count_zero` | A count, duration or process id that must be at least 1 is 0. | The field when it is `replicates` or one under `budget`; none otherwise. | experiment |
| `experiment.document_invalid` | The TOML is not an experiment file: a field is missing, has the wrong type or breaks the format of its own. | The field at fault, or its parent table when a field is missing; none at the root. | experiment |
| `experiment.entry_repeated` | A list holds the same entry twice: a fixed path, a factor path or an input kind. | - | experiment |
| `experiment.factor_without_levels` | A factor has no level, so the experiment would have no cell. | - | experiment |
| `experiment.input_invalid` | An input declaration uses the kind the wire reserves, or mounts under an empty, absolute or escaping directory. | `/inputs`, the list that holds the declaration. | experiment |
| `experiment.input_not_stored` | A declared input file or list could not be copied into the store. | The `inputs` array of the experiment file. | experiment |
| `experiment.input_unusable` | A declared input cannot be used as written, for example a missing file or a directory where a file is needed. | The `inputs` array of the experiment file. | experiment |
| `experiment.name_collision` | Two names under one root map to the same deviation path, so a deviation there would be ambiguous. | - | experiment |
| `experiment.path_overlap` | Two paths set by the fixed values and the factors are the same, or one lies below the other. | - | experiment |
| `experiment.reserved_root_misused` | A value at a reserved root is not the root alone, or has no value. | - | experiment |
| `experiment.reserved_value_malformed` | The value of a reserved root is not the shape the root takes. | - | experiment |
| `experiment.setting_absolute_path` | A setting holds an absolute path where a path relative to the project is required. | - | experiment |
| `experiment.setting_as_factor` | A factor varies a setting, which no cell can vary; the setting belongs in `[fixed]`. | The `factors` array of the experiment file; a verb that has read the file narrows it to the `path` of the factor. | experiment |
| `experiment.setting_missing` | A setting the harness needs has no value in the cell. | - | experiment |
| `experiment.setting_under_reserved_root` | A setting is written below a reserved root, which takes one value. | - | experiment |
| `experiment.setting_value_invalid` | A setting holds a value that has the wrong form for what it names, such as a name with a space. | - | experiment |
| `experiment.setting_wrong_type` | A setting holds a JSON value of a type its path does not take. | - | experiment |
| `experiment.table_empty` | A table among the fixed values or the levels holds no value, so it fills no path. | - | experiment |
| `experiment.toml_invalid` | The experiment file is not valid TOML. | - | experiment |
| `experiment.unknown_field` | The experiment file holds a key its type does not have, such as a misspelt option. | The unknown key itself. | experiment |
| `experiment.value_invalid` | A value of an experiment record has the wrong form for what it names, such as a name with a space. | - | experiment |
| `experiment.value_unrepresentable` | A TOML value is a date, a time or a number that JSON cannot state. | - | experiment |
| `format.evidence_ref_invalid` | An evidence reference does not fit the format of a reference. | - | config |
| `format.expectation_id_invalid` | An expectation id does not fit the format of an id. | - | config |
| `format.field_path_invalid` | A field path is not segments of lowercase letters, digits and underscores joined by dots. | - | config |
| `format.marker_invalid` | A marker does not fit the format of a marker. | - | config |
| `format.name_invalid` | A name holds a character a name may not hold, or is empty. | - | config |
| `format.name_version_invalid` | A `name@version` text does not hold a name and a SemVer version. | - | config |
| `format.reason_code_invalid` | A reason code does not fit the format of a reason code. | - | config |
| `format.rel_path_invalid` | A relative path is empty, absolute, or holds a `.` or `..` segment. | - | config |
| `format.schema_tag_mismatch` | The `schema` field of a document holds another version marker than its type's. | `/schema` | config |
| `format.secret_name_invalid` | A secret name does not fit the format of a secret name. | - | config |
| `values.null` | A setting is null, where an absent value is omitted. | - | config |
<!-- /generated:refusals -->

## Notes
