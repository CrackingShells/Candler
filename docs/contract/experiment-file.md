# Experiment file

## Surface

The experiment file an experimenter writes to fix the values, factors, replicates, budget, harness and treatment of a set of runs; Candler reads it to plan and run the cells. Instruction texts and hooks are tables whose keys are the harness's own words: `instructions.global` (harness id to text), `instructions.workspace` (a text), and `hooks` (harness id to `settings` and `files` for `claude-code`, or `plugins` for `opencode`), in `[fixed]` or as factor levels. A hook a consumer gives the agent may approve tool calls, which changes what `permission_mode` means for that cell.

## Version

<!-- generated:version -->
Wire tag: `experiment/1`.

<!-- versions -->
Wire line: `schemas@0.13.0`.
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
| `env` | `map of string` | optional | `{}` |
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
| `treatment` | `TreatmentSpec` | optional | - |
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
| `experiment.cell_value_invalid` | A cell value cannot be used by the harness driver. | The field of the value, such as `/mcp/servers/fs/command`. | experiment |
| `experiment.cell_value_missing` | A cell lacks a value that the harness driver needs, such as a hooks table that has no entry for the cell's harness. | The key of `[fixed]` that holds the value, such as `/fixed/hooks`. | experiment |
| `experiment.cost_not_positive` | A cost cap is not a finite number above 0. | - | experiment |
| `experiment.count_zero` | A count, duration or process id that must be at least 1 is 0. | The field when it is `replicates` or one under `budget`; none otherwise. | experiment |
| `experiment.document_invalid` | The TOML is not an experiment file: a field is missing, has the wrong type or breaks the format of its own. | The field at fault, or its parent table when a field is missing; none at the root. | experiment |
| `experiment.entry_repeated` | A list holds the same entry twice: a fixed path, a factor path or an input kind. | - | experiment |
| `experiment.env_name_invalid` | An entry of the `env` table has a name that is not a variable name: it must match `[A-Za-z_][A-Za-z0-9_]*`. | The key of the entry in the `env` table. | experiment |
| `experiment.env_reserved` | An entry of the `env` table names a variable Candler sets for the harness process itself. | The key of the entry in the `env` table. | experiment |
| `experiment.env_secret` | An entry of the `env` table names a secret the experiment declares; a secret's value reaches the gateway only. | The key of the entry in the `env` table. | experiment |
| `experiment.env_source` | A `[fixed]` key, a factor or a `--set` names the path `env`; the `[env]` table is its only source. | The path that was written. | experiment |
| `experiment.env_token_secret` | An entry of the `env` table is named like `model.token_secret`, the secret that holds the gateway token, whose value reaches the gateway only. | The key of the entry in the `env` table. | experiment |
| `experiment.factor_without_levels` | A factor has no level, so the experiment would have no cell. | - | experiment |
| `experiment.harness_rejected_input` | The harness's own check reports an error or a warning for a file Candler installed for the cell, such as a hook configuration the harness cannot read. | The setting in the experiment file that the file comes from, such as `/fixed/hooks`. | experiment |
| `experiment.input_invalid` | An input declaration uses the kind the wire reserves, or mounts under an empty, absolute or escaping directory. | `/inputs`, the list that holds the declaration. | experiment |
| `experiment.input_not_stored` | A declared input file or list could not be copied into the store. | The `inputs` array of the experiment file. | experiment |
| `experiment.input_unusable` | A declared input cannot be used as written, for example a missing file or a directory where a file is needed. | The `inputs` array of the experiment file. | experiment |
| `experiment.instructions_collide` | The workspace instruction text and a declared input mount a file at the same path, the harness's workspace instruction file; the text is refused, never shadowed. | The workspace instruction text in the experiment file, `/fixed/instructions.workspace` or the factor that sets it. | experiment |
| `experiment.mcp_server_prefix_ambiguous` | Two MCP servers of a cell have names that make one tool name read two ways: the name of one, then `_`, is a prefix of the other's name, so a tool of the longer-named server cannot be told from a tool of the shorter-named one. | The table of servers, `/mcp/servers`. | experiment |
| `experiment.name_collision` | Two names under one root map to the same deviation path, so a deviation there would be ambiguous. | - | experiment |
| `experiment.path_overlap` | Two paths set by the fixed values and the factors are the same, or one lies below the other. | - | experiment |
| `experiment.replicates_conflict` | `replicates` in `[fixed]` differs from the experiment file's own `replicates`; write the count once. | The `replicates` key. | experiment |
| `experiment.replicates_not_count` | `replicates` is not a whole number from 1 to 4294967295, such as a negative number, a fraction, a count beyond 32 bits or a string. | The `replicates` key. | experiment |
| `experiment.replicates_path` | A `[fixed]` key or a `--set` names a path below `replicates`; `replicates` is one count and has no fields. | The path that was written. | experiment |
| `experiment.reserved_root_misused` | A value at a reserved root is not the root alone, or has no value. | - | experiment |
| `experiment.reserved_value_malformed` | The value of a reserved root is not the shape the root takes. | - | experiment |
| `experiment.session_refused` | A cell's session settings are refused: `runner.mode` is not `headless` or `session`, `runner.idle_s` is not whole seconds or `none`, `runner.idle_s` is set on a cell that is not a session, or the harness's driver cannot run a session. | - | experiment |
| `experiment.setting_as_factor` | A factor varies a setting, which no cell can vary; the setting belongs in `[fixed]`. | The `factors` array of the experiment file; a verb that has read the file narrows it to the `path` of the factor. | experiment |
| `experiment.setting_unsupported` | The experiment sets a setting that the harness has no way to honour, such as a turn cap for a harness with no turn limit; the run is refused rather than the setting dropped. | The setting in the experiment file: a field under `/budget`, or the key of `[fixed]` that holds it, such as `/fixed/model.effort`. | experiment |
| `experiment.table_empty` | A table among the fixed values or the levels holds no value, so it fills no path. | - | experiment |
| `experiment.toml_invalid` | The experiment file is not valid TOML. | - | experiment |
| `experiment.unknown_field` | The experiment file holds a key its type does not have, such as a misspelt option. | The unknown key itself. | experiment |
| `experiment.unknown_harness` | `request.harness`, written as `name` under `[harness]` or given to `--set`, names a harness that is not registered in this binary; the refusal lists the registered names. | - | experiment |
| `experiment.unknown_path` | A dotted path under `[fixed]`, in a factor or in `--set` is none that the experiment surface has, such as a misspelt `model.idd`. | The key under `/fixed`, the `path` of the factor, or the key of the level's table that holds the path; none for `--set`. | experiment |
| `experiment.unknown_request_key` | `--set` names a key under `request` that is none of the request settings, such as a misspelt `request.harnes`. | - | experiment |
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

- The document has no `free` fields. An unknown key outside `[fixed]` and the `levels` of a factor is `experiment.unknown_field`; a key under `[fixed]` or in a level table that is no setting path is `experiment.unknown_path`. [checked: v@0.7.0]
- A TOML date or time is refused with `experiment.value_unrepresentable`. [checked: v@0.7.0]
- A name, secret name or `schema` marker of the wrong form, an integer out of range and a value that breaks its field's format print `experiment.document_invalid`, never `format.*`. Any other value-invalid file prints its own code. [checked: v@0.13.0]
- `canonical.integer_out_of_range` and `values.null` print only with `--set`. [checked: v@0.7.0]
- `mcp.servers.<key>.name` names a server (default the key); a driver refuses a name with `__` or one two servers share. [checked: v@0.7.0]
- `experiment.schema.json` does not check the entries of `mcp.servers`; `mcp-server.schema.json` checks one entry. [checked: schemas@0.12.0]
