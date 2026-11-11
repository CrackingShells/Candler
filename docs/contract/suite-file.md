# Suite file

## Surface

The suite file an evaluation author writes to list the expectations a run is graded against; Candler reads it to choose and run the evaluators.

## Version

<!-- generated:version -->
Wire tag: `suite/1`.

<!-- versions -->
Wire line: `schemas@0.6.0`.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
| Path | Type | Class | Default |
|---|---|---|---|
| `expectations` | `array of SuiteEntry` | required | - |
| `expectations[].applies_to` | `array of Condition` | optional | - |
| `expectations[].applies_to[].any_of` | `array of any` | required | - |
| `expectations[].applies_to[].path` | `FieldPath` | required | - |
| `expectations[].deadline_ms` | `integer` | optional | - |
| `expectations[].depends_on` | `array of ExpectationId` | required | - |
| `expectations[].evaluator` | `EvaluatorRef` | required | - |
| `expectations[].evaluator.name` | `Name` | required | - |
| `expectations[].evaluator.range` | `string` | required | - |
| `expectations[].group` | `Name` | free | - |
| `expectations[].id` | `ExpectationId` | required | - |
| `expectations[].kind` | `Name` | required | - |
| `expectations[].params` | `map` | required | - |
| `expectations[].role` | `"gate" \| "quality" \| "descriptive"` | required | - |
| `schema` | `"suite/1"` | required | - |
| `source` | `map` | free | - |

<!-- /generated:fields -->

## Refusals

<!-- generated:refusals -->
| Code | When | Path at fault | Side to fix |
|---|---|---|---|
| `suite.dependency_cycle` | Expectations depend on each other in a loop. | - | suite |
| `suite.dependency_unknown` | An expectation depends on an id that no expectation has. | - | suite |
| `suite.document_invalid` | A suite is not valid TOML, or does not have the shape of a suite. | The field at fault, or its parent table when a field is missing; none at the root. | suite |
| `suite.equivalence_invalid` | The equivalence file is not a list of rows of a run and an expectation. | - | suite |
| `suite.equivalence_row_repeated` | The equivalence file lists one row of a run and an expectation twice. | - | suite |
| `suite.equivalence_run_invalid` | A row of the equivalence file names a run that is not a run key. | - | suite |
| `suite.equivalence_unreadable` | The equivalence file does not exist or cannot be read. | - | suite |
| `suite.evaluator_unknown` | An entry of the suite names an evaluator for which no plug-in manifest was loaded. | - | suite |
| `suite.evaluator_version_mismatch` | A plug-in describes a version outside the range the suite entry asks for. | - | suite |
| `suite.id_repeated` | Two expectations of a suite share an id. | - | suite |
| `suite.json_invalid` | A suite is not valid JSON, or does not have the shape of a suite. | The field at fault, or its parent object when a field is missing; none at the root. | suite |
| `suite.kind_unknown` | An entry of the suite names a kind that its evaluator does not declare. | - | suite |
| `suite.list_empty` | A list of a suite entry that needs at least one member is empty, such as `any_of` or `applies_to`. | - | suite |
| `suite.params_invalid` | The params of a suite entry do not satisfy the params schema of its kind. | The JSON pointer, inside the `params` of the entry, of the value the schema refuses; none when the whole of `params` is refused. | suite |
| `suite.unknown_field` | The suite holds a key its type does not have, such as a misspelt option. | The unknown key itself. | suite |
<!-- /generated:refusals -->

## Notes

- `source` is an open table of class `free` that records where a translated suite came from; Candler stores it and never reads it. [checked: schemas@0.6.0]
- Any other unknown key is refused with `suite.unknown_field`, and its path is the key. [checked: schemas@0.6.0]
- A value of the wrong form, such as an id with a space or an empty `any_of`, is refused with `suite.document_invalid`, its `path` pointing at the field. [checked: v@0.6.0]
- A suite is TOML, or JSON when its file name ends in `.json`. [checked: v@0.6.0]
- Expectations form a graph through `depends_on`: a loop is `suite.dependency_cycle`, an id no entry has is `suite.dependency_unknown`, and an id used twice is `suite.id_repeated`. [checked: v@0.6.0]
- The suite is checked before any plug-in is asked to describe itself. [checked: v@0.6.0]
