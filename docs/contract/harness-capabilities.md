# Harness capabilities

## Surface

What a run can expect of each registered harness: what the harness does itself, what Candler supplies in its place, and what resolving refuses.

## Version

<!-- generated:version -->
<!-- versions -->
Core line: `v0.8.5`; the `candler-harness` crate has the same version.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
| Capability | `claude-code` | `opencode` |
|---|---|---|
| gateway route | Candler: gateway, address in `ANTHROPIC_BASE_URL` | Candler: gateway, address in `CANDLER_GATEWAY_URL` |
| census source | yes | Candler: gateway, first request with tools |
| session mode | yes | no (refused) |
| turn cap | yes | no (refused) |
| cost cap | yes | no (refused) |
| stop | yes | Candler: interrupt command in the container, then SIGTERM |
| authoritative trace | yes | Candler: collects `export` after the run |
| skills layout | yes | Candler: places the tree at `.opencode/skills/<name>` |
<!-- /generated:fields -->

## Refusals

<!-- generated:refusals -->
| Code | When | Path at fault | Side to fix |
|---|---|---|---|
| `experiment.session_refused` | A cell's session settings are refused: `runner.mode` is not `headless` or `session`, `runner.idle_s` is not whole seconds or `none`, `runner.idle_s` is set on a cell that is not a session, or the harness's driver cannot run a session. | - | experiment |
| `experiment.setting_unsupported` | The experiment sets a setting that the harness has no way to honour, such as a turn cap for a harness with no turn limit; the run is refused rather than the setting dropped. | The setting in the experiment file: a field under `/budget`, or the key of `[fixed]` that holds it, such as `/fixed/model.effort`. | experiment |
<!-- /generated:refusals -->

## Notes

- `no (refused)` is a refusal with a code: `experiment.session_refused` while resolving, `experiment.setting_unsupported` when the cells are planned, before a job starts. [checked: v@0.7.0]
- In session mode Candler also counts turns across the session and ends the run when they exceed `budget.per_run.turns`. [checked: v@0.7.0]
- The settings a driver maps or refuses, such as permission modes, effort levels and tool names, are rows of its `settings_table()`. [checked: v@0.7.0]
