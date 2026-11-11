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

## Notes
