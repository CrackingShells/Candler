# setting_unsupported: one refusal for a setting a harness cannot honour

Leaf of `m4-session-runner`, branch `m4/setting_unsupported`, cut from 9bdfc1c. `DriverError::Unsupported` is shared by every driver, coded `experiment.setting_unsupported` (side experiment), and carries the pointer of the setting. The OpenCode driver uses it for six refusals. `candler-harness-claude-code`, `candler-core` and `candler` are untouched. No Docker, no model.

## 1. Commits

| sha | subject |
|:--|:--|
| b358906 | `feat(runner): refuse a setting a harness cannot honour, naming its path` |
| 421564c | `feat(adapter-opencode): refuse unsupported settings with their path` |

The report commit `reports(runner): record the unsupported-setting refusal` follows.

## 2. Matched path format

The path is an RFC 6901 pointer into the experiment file, built with `candler_schemas::refusal::pointer_of`. Two existing spellings are composed, none is new:

| setting | pointer | matches |
|:--|:--|:--|
| a field under `budget` | `/budget/per_run/turns` | `file_field` in `candler-schemas/src/refusal_impls.rs` (`ExecutionError::Zero`, `NotPositive`) |
| any other setting | `/fixed/model.effort` (the dotted path is one key) | the `[fixed]` arm of `unknown_path_pointer` in `candler/src/commands/refusal.rs` (`experiment.unknown_path`) |

`DriverError::unsupported(setting, harness, version, detail)` in `candler-harness` does the mapping, so no driver spells a pointer.

## 3. Guards and bites

| guard | Guards | oracle | bite (failing message seen) |
|:--|:--|:--|:--|
| `candler-harness::guard_unsupported_setting::an_unsupported_setting_is_one_coded_refusal_at_its_pointer` | a driver's refusal reaches `Refusal` with the shared code, side experiment, the pointer, and a text naming harness, version and setting | pointers by hand from the two functions above | path dropped in `driver_error_path`: `left: None`, `right: Some("/budget/per_run/turns")`; budget treated as a `[fixed]` key: `left: Some("/fixed/budget.per_run.turns")` |
| `opencode_refuses_model_effort_as_unsupported` | `model.effort` is refused, never dropped, at `/fixed/model.effort` | `opencode run --help`, `opencode_facts` row 2 | `InvalidValue` kept: `left: "experiment.cell_value_invalid"`, `right: "experiment.setting_unsupported"`; path dropped: `left: None` |
| `opencode_refuses_a_turn_cap_as_unsupported` | `/budget/per_run/turns` | same | `InvalidValue` kept; path dropped; budget under `/fixed`: `left: Some("/fixed/budget.per_run.turns")` |
| `opencode_refuses_a_cost_cap_as_unsupported` | `/budget/per_run/max_cost_usd` | same | as turns |
| `opencode_refuses_allowed_tools_as_unsupported` | `/fixed/tools.allowed` | tool names are Claude Code's | `InvalidValue` kept; path dropped |
| `opencode_refuses_disallowed_tools_as_unsupported` | `/fixed/tools.disallowed` | same | `InvalidValue` kept; path dropped |
| `opencode_refuses_another_permission_mode_as_unsupported` | `/fixed/permission_mode`, message names `bypassPermissions` | `--auto` is the one mode | `InvalidValue` kept for the mode only: `left: "experiment.cell_value_invalid"` |

Each OpenCode guard also asserts the side, and that the message holds the pointer, `OpenCode` and `2.2.7`. All guards were red before the code (the harness guard did not compile; the six others failed on the code). The old `opencode_command_refuses_values_it_cannot_honour` is replaced by the six.

## 4. Gates

| gate | result |
|:--|:--|
| `-p candler-harness -p candler-harness-opencode` nextest | PASS: 50 run, 50 passed, 1 skipped (the Docker test) |
| `guard_refusal_codes` | PASS: 4/4 |
| `contract_pages`, `guard_cli_surface` | PASS: 18/18 with the pages blessed; `contract_pages` 7/7 unblessed |
| SPAWN | PASS: `guard_signal` 9/9 |
| STD, `-p` form: fmt, clippy `-D warnings` all targets, doc `-D warnings`, nextest | PASS |
| LINUX | PASS: clippy `x86_64-unknown-linux-gnu --workspace --tests -D warnings` finished clean |
| DIGEST | PASS: `guard_cell_digest` 9/9, the six core binaries 45/45 |
| `--workspace` nextest | PASS: 657 run, 657 passed, 22 skipped |
| SUBJ | PASS |
| OWNED check | PASS: only the harness and OpenCode crates, `codes.toml`, `experiment-file.md`, this report |

## 5. Findings

1. **Deviation: a `detail` field.** The variant is `Unsupported { path, harness, version, detail }`. The spec names three fields and asks the permission-mode refusal to say which mode is accepted "in the message or a detail". The text is `OpenCode 2.2.7 cannot honour /fixed/permission_mode: ...reason`. The other five carry their old reasons.
2. **`path` is a `String` pointer, `harness` a display string.** `harness` holds `OpenCode` (the driver's `DISPLAY_NAME`), not the wire name `opencode`, because the message must read `OpenCode 2.2.7`. A second driver passes its own spelling.
3. **Pointer ambiguity for non-budget settings (for the coordinator).** A driver sees a cell, not where its value was written. A setting that came from a factor level or `--set` sits at `/factors/<i>/...`, or nowhere. `/fixed/<dotted>` is right when the value is a `[fixed]` key, the common case. A verb that has read the file may narrow it, as `experiment.setting_as_factor` does. Not done here.
4. **No exhaustive `match` broke.** `DriverError` is `#[non_exhaustive]`; `candler-core` and `candler` compile and `guard_refusal_codes` passes with no arm added.
5. **Contract pages.** Only `experiment-file.md` changed (one row). `codes.toml` has one appended row under `[harness]`. The first blessed `contract_pages` run failed `d3_every_author_code_has_a_page` while writing; the unblessed rerun passes 7/7.
6. **Behaviour change (small).** The six refusals change code from `experiment.cell_value_invalid` to `experiment.setting_unsupported` and gain a path. No stored digest moves.
7. **Process note.** I once lost uncommitted edits with `git checkout --` while biting before committing, and redid them; nothing outside OWNED was touched.
