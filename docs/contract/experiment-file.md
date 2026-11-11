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

## Notes
