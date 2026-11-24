# Leaf P8 declaration: every per-harness list in one shape

Base `bf1f03ea` (`origin/feat/integrity`), tip `3ffbefb8`. Commits: `7b0e80fa` (MCP transports answered per driver), `af34f725` (one shape), `8318f2dd` (first report), `3ffbefb8` (one reader in `set_by`). The first implementer ran over its call cap after the second commit; a second round finished the leaf.

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| `SettingRow`, `settings_table`, `Capabilities`, builder and `BudgetCaps` gone, no second table | PASS | tree builds; `AgentSetting::ALL` and `DriverFact::ALL` are the only lists, read through `answer` and `fact` |
| Guards fail on the base | PASS | `conformance_claude_code_fixtures`: "the driver answers no row for model.id"; the walk listed 20 unanswered (driver, path) pairs; a `Capabilities::builder()` driver with no `.session(..)` passed conformance with the default `false` (39/39) |
| One variant each in `AgentSetting` and `DriverFact` fails to compile in both drivers | PASS | scratch worktree, nothing committed. `claude-code/src/driver.rs:203 error[E0004]: non-exhaustive patterns: AgentSetting::BiteSetting not covered`; `claude-code/src/driver.rs:276 ... DriverFact::BiteFact not covered`; `opencode/src/driver.rs:163 ... DriverFact::BiteFact not covered`; `opencode/src/prepare.rs:112 ... AgentSetting::BiteSetting not covered` |
| Wildcard denied in both drivers | PASS | `clippy::wildcard_enum_match_arm = "deny"` is workspace-wide; clippy clean |
| OpenCode cost cap refused through `Refused` arm; hand-written lists gone | PASS | bite (a): the cost arm made `Mapped` fails `guard_opencode_driver::opencode_command_line_maps_or_refuses_every_setting`: `budget.per_run.max_cost_usd *: the case and the answer disagree`, left `true`, right `false` (`guard_opencode_driver.rs:446`) |
| `PathRow.reach` and the walk | PASS | bite (b): `model.endpoint` moved to `Reach::CandlerOnly` fails `guard_driver_answers::every_agent_setting_is_reached_by_a_path_or_has_no_path`: `reached by no path: [ModelEndpoint]` (`guard_driver_answers.rs:32`) |
| `guard_silent_acceptance.rs` rows read `answer` | DEVIATION | The file holds no driver rows (resolve, plan and MCP-reader refusals only), so there is nothing to read from `answer`. The driver-versus-answer row is `guard_opencode_driver`'s case loop, which reads `answer` (bite (a) shows it). Two rows were added for the one reader: an effort and a permission mode that name nothing are refused with `experiment.cell_value_invalid` at `/model/effort` and `/permission_mode` |
| Hooks and instruction files not `AgentSetting` variants | PASS | none added |
| "What it leaves" at three levels | PASS | below |
| Verifier (Step 3) | MISSING | not run here; the lead dispatches it |
| fmt, clippy `-D warnings`, doc `-D warnings` | PASS | all clean on `3ffbefb8` |
| nextest workspace | PASS | `756 tests run: 755 passed, 1 failed, 31 skipped`. The one failure, `guard_protocol_client::every_failure_path_of_an_evaluator_is_accounted_for` (`cancel.sh`: A1 `NotMeasured("deadline")` where `Reported` was expected), is a deadline race under load unrelated to the change: it passed in the first full run and in three reruns alone (8.7 s each). The 31 skipped are the container-gated tests, as on the base |

## What changed in the second round

- **One reader of the servers.** `AgentSetting::set_by` re-read `mcp.servers` from raw JSON with `.ok()` and guessed the transport. It now reads each entry through `mcp::read`, the reader behind `mcp::servers`, and propagates its error. `read` is `pub(crate)`; `by_name` is not applied, because `name` is what `McpName` reports.
- **One check of effort and permission mode.** `set_by` refuses text no `EffortLevel` or `PermissionMode` names with `DriverError::InvalidValue` naming the path and the accepted texts. OpenCode's two duplicate checks (`EFFORT_LEVELS`, `OTHER_MODE`, `EFFORT_LEVEL`) are deleted, with the two table cases that tested them.
- **Right error variant.** Cell reads no longer map to `VersionOutput`: `Cell::text` and `Cell::value` give `InvalidValue`, and no path text is parsed at run time.
- **Honest arm.** The Claude Code driver passes the level to `--effort` unchanged, so `Effort(Ultracode)` says `--effort ultracode`, which Claude Code reads as `xhigh` with its ultracode mode on, apart from the other levels.

## Behaviour change

Claude Code used to pass any effort or permission-mode text through to the command line. It now refuses text no level or mode names, before any job starts. Claude Code is the reference harness, but passing unknown text through was a defect, so both drivers carry the fix. OpenCode's refusal of an unknown mode or level changes code, from `experiment.setting_unsupported` to `experiment.cell_value_invalid`, because the value is invalid rather than unsupported by one harness.

## Design decisions

- **`Reach::Agent` takes a slice.** One path can reach several keys: `mcp` reaches transport, `codemode` and `name`. A row per key would repeat the path and the walk would count one table three times.
- **`set_by` does not read `SessionMode`.** The session refusal stays `experiment.session_refused`, raised from the plan's session, and conformance holds the driver's answer to `plan.session`. Reading it from a cell would add a second place that decides it.
- **The per-tool-name rows are gone.** The old OpenCode table listed each tool name as a row. A tool name is a value of `tools.allowed`, not a key; name validation stays where the values are read.
- **`clippy::wildcard_enum_match_arm` is denied workspace-wide.** A `_ =>` arm is how a new key goes unanswered; denying it in the workspace lets the compiler say what the review would have to.

## What it leaves

- **How it was developed.** One build slot (`l3-1`) and one scratch build for the three bites, removed with its worktree, branch and target dir. The first round used its whole call cap on Steps 1 and 2 and stopped before the bites and the report; the second round kept to reading through colgrep and to three full workspace runs. The bites ran on a scratch worktree and nothing of them is committed.
- **What it leaves in the code.** One shape per list: a key enum with `ALL`, a `Copy` answer of `&'static str`, one exhaustive `match` per driver. One reader of `mcp.servers`, one check of effort and mode, for both drivers. Nothing replaces the old structs. Open points: `NO_PATH` holds only `Skills`; `Env`, `Inputs` and `WallBudget` are stated as Candler's without a check against the core; `mcp::read` repeats the transport default (`url` means remote) that `read` uses to validate.
- **How it is consumed.** The experimenter reads one refusal for an unknown effort or mode, naming the accepted texts, whichever harness the cell names. Refusal texts of the arms are unchanged. The capability page reads `answer` and `fact`, so a new key reaches the page when a driver answers it.

## Not run

The Step 3 verifier (a fresh agent over the leaf, this report and the diff).
