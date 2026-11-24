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

## Verifier round

| Finding | Result |
|---|---|
| B1 stale prose (`settings_table`, `SettingRow`, `Capabilities`, `BudgetCaps`) | Fixed in the two contract-page notes (the pages carry their Notes over, so the page text is the source), the harness crate README, the OpenCode guard's doc comment and the architecture document, which now name `answer`, `fact` and `SessionMode`. History under `__reports__/` and `__roadmap__/` is untouched. |
| B2 path-to-key mapping stated three times | Handled in a separate round that reshapes `reach`, `AgentSetting::path()` and `set_by`. Only the two small parts of it that touch no `reach` data were done here: `set_by` reads `mcp.servers` through `mcp::SERVERS`, and the repeated "a `url` means `Remote`" default is gone; `mcp::kind` is the one place that resolves a server's transport, used by `mcp::read` and by `set_by`. |
| N1 one check before any job | Claude Code's `check_settings` now refuses a bad turn or cost cap as `plan` does, and `plan` runs `check_settings` first, so `check_answers` runs in both drivers' plans. No new validator. |
| N2 value typing of the budgets | Left: it is the resolve step's. |
| N3 OpenCode effort answers say `Mapped` unconditionally | The `Mapped` construct now states the condition (a Claude opus, sonnet, fable or mythos of 4.6 or later, `xhigh` from 4.7, refused for other models). `effort_variant` no longer re-reads effort as text: `set_by` has already validated it. |
| N4 census wording on the page | Left: goes to the pages leaf. |
| N5 OpenCode case table | Left: a deliberate cross-check. |
| N6 OpenCode prose | The doc comment above `FAMILIES` now describes the families; the `ULTRACODE` text and the doc above it no longer say what the Claude Code driver does. |

## B2 round

Base `e8786036`. Finding B2: each setting's path was stated in `PathRow.reach`, `AgentSetting::path()` and `AgentSetting::set_by`, and effort and permission mode were parsed in `set_by` and again in the drivers.

| Gate | Verdict | Evidence |
|---|---|---|
| G-B2a only `CONFIGURATION_PATHS` maps a path to a setting | PASS | `AgentSetting::path`, `AgentSetting::set_by` and their path lists are deleted; `git grep -E "fn set_by\|AgentSetting::set_by\|fn path\(self\)" -- crates/candler-harness*/src` returns nothing. The walk is `resolve::paths::settings_set_by`, over the rows whose reach is `Agent(..)` |
| G-B2b unknown effort and mode text give one `cell_value_invalid` at the field, both drivers | PASS | `guard_session::an_unnamed_effort_or_mode_is_refused_at_its_field_for_each_driver` goes through the real `resolve` and `plan_runs` for Claude Code and OpenCode, and checks `experiment.cell_value_invalid` at `/model/effort` and `/permission_mode`. The two `guard_silent_acceptance` rows read `settings_set_by` and still pass |
| G-B2c a refused setting is refused once, from core, before `plan` | PASS | `refuse_unanswered` is called in `fresh_cells` beside `check_settings`; `check_answers` is deleted with its two driver calls (`git grep check_answers` finds only the unrelated conformance function). `guard_session::session_plan_refuses_a_setting_the_driver_cannot_honour` stays green. `guard_opencode_driver`'s case loop now checks the driver's answer and reason for the cases the core refuses, and still checks the plan for the value refusals the driver keeps (an effort for a model that takes none, a tool name) |
| G-B2d completeness walk bites | PASS | `guard_driver_answers::every_agent_setting_is_reached_by_a_path_or_has_no_path` already held the walk (reach and `NO_PATH` over `AgentSetting::ALL`), so no second one was added. Scratch worktree off `e8786036` with `model.endpoint` moved to `Reach::CandlerOnly`: `FAIL ... reached by no path: [ModelEndpoint]`. It passes on the tip in the full run. The scratch worktree, branch and target dir are removed |
| fmt, clippy `-D warnings`, doc `-D warnings` | PASS | all clean |
| nextest workspace | PASS | `757 tests run: 757 passed (6 slow), 31 skipped` (the skipped are the container-gated tests, as on the base) |

### What changed

- `Cell::effort()` returns `Option<EffortLevel>` and `Cell::permission_mode()` returns `PermissionMode`; neither fails. Both drivers read them typed (`--effort` and `--permission-mode` take `as_str()`; OpenCode's `effort_variant` takes the level). OpenCode's own `settings` no longer re-reads the mode.
- `settings_set_by` is the one reader that refuses unknown text, with `DriverError::InvalidValue` at the field, which the refusal table maps to `experiment.cell_value_invalid`.
- MCP kind, `codemode` and `name` come from `mcp::servers` and `mcp::kind` (now `pub`).
- `DriverError::Unsupported` loses its `version` field, because the core, which now raises the refusal, knows the harness id and not the version a driver was validated against.

### Deviations

- **Slice.** `Reach::Agent` still takes a slice, used by more than one entry only on the `mcp` row: `CONFIGURATION_PATHS` has no per-field mcp rows, and the three keys of a server are read together from the transport entry.
- **Two readers of the spelling.** `EffortLevel::from_text` and `PermissionMode::from_text` stay beside the enums, in `candler-harness`, and are called from `settings_set_by` (the refusing read) and from `Cell::effort`/`Cell::permission_mode` (the typed read, which cannot fail). The `Cell` is a view over the resolved values that many sites build, so holding typed fields in it would have meant parsing at each constructor. A cell with unnamed text never reaches a driver through the core; a hand-built cell reads as unset (effort) or `Default` (mode).
- `permission_mode` absent now reads as `Default` where it used to be `cell_value_missing`. After the round, the walk also yields `PermissionMode(Default)` for an absent mode, so the driver is asked about it. In the round as first committed, an OpenCode cell with no mode was never asked about one. OpenCode's plan does not read the mode, so the cell ran under the allow-all baseline. Now it is refused with `DEFAULT_MODE`, while Claude Code runs in its own mode for no flag. The wiring double, whose preparation is fixed, answers the mode by that preparation. The walk also reads each row's path through `field_path` instead of mapping a parse error to `VersionOutput`.
- `SessionMode` and `Skills` are not yielded by the walk (the plan's session refuses the first, the treatment holds the second).

### Digest

The serialised cell does not change. The values stay the strings the experiment wrote; `from_text` and `as_str` are inverses, and nothing rewrites the text on its way into the cell. The digest holds.

### Vocabulary

Claude Code docs through context7 (`/websites/code_claude`, CLI reference): `--effort` accepts `low`, `medium`, `high`, `xhigh`, `max`, `ultracode`; that matches `EffortLevel`. `--permission-mode` accepts `default`, `acceptEdits`, `plan`, `auto`, `dontAsk`, `bypassPermissions` and `manual` (an alias for `default`). **Gap, closed after the round:** `PermissionMode` had no `auto`, `dontAsk` or `manual`, so resolve refused three spellings Claude Code reads. Claude Code is the reference, so `PermissionMode` gains `Auto` and `DontAsk`, and `from_text` reads `manual` as `Default`. Claude Code maps both new modes to `--permission-mode <mode>`. OpenCode refuses both: `auto` because it has no classifier, and `dontAsk` for the same read-only-set reason as `default`. The guard `every_claude_code_permission_mode_plans_and_opencode_refuses_what_it_cannot_carry` plans every spelling through resolve and `plan_runs`, and reads the argv word. It bites: with the alias removed it fails with "manual on Claude Code". nextest: 758 run, 758 passed, 31 skipped. `claude --version` here is 2.1.316; its `--help` lists the flags without the values.

### What it leaves

- **How it was developed.** One build slot (`l3-1`), one scratch build for the bite, removed with its worktree, branch and target dir; three full workspace runs.
- **What it leaves in the code.** One statement of each path (the table), one walk, one refusal site, one refusing reader of each spelling. Open points: the second `from_text` call in `Cell`; the message of `setting_unsupported` no longer names the validated version.
- **How it is consumed.** The experimenter sees one `cell_value_invalid` at the field for unknown effort or mode text on either harness, and `setting_unsupported` for a setting the harness has no way to honour, raised before any job starts.

### Cells built without resolve
`Cell` is a view over resolved JSON values. `effort_at` and `mode_at` refuse unknown text at resolve, and `Cell::effort` and `Cell::permission_mode` parse the checked text again. Their fallback (unset, `Default`) is right only while the text went through resolve. These places build a `Cell` from values that have not passed `settings_set_by` in that call:
- `runner/exec/mod.rs` (the run's view) and `jobs/orchestrate/servers.rs` (both views) read `plan.stored`, a cell read back from its stored record when a run resumes, when there is one. The text was checked when it was first resolved.
- `resolve/steps/catalog.rs` builds `Cell::new(&ctx.values)` for the catalog capture during resolve.
- Test rigs build cells by hand or from `ExperimentFile::cells()`: `guard_session`, `guard_container_record`, `guard_plugin_environment`, `plugin_env_e2e`, `support/harness_facts`, `guard_silent_acceptance`, and Claude Code's `conformance` and `prepare` tests.

Whether a resolved cell should hold typed values goes to seam review 3.
