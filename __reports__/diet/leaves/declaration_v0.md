# Leaf P8 declaration: report v0

Base `bf1f03ea` (origin/feat/integrity). Status: Steps 1 and 2 landed; Step 3 (bite edits, scratch compile failure, verifier) not run.

## Gates
| Gate | Verdict | Evidence |
|---|---|---|
| Old types gone (`SettingRow`, `settings_table`, `Capabilities`, builder, `BudgetCaps`) | PASS | tree builds, nothing replaces them with a table |
| Guards seen failing on the base | PASS | `conformance_claude_code_fixtures`: `settings_table`, "the driver answers no row for model.id"; the walk listed 20 unanswered (driver, path) pairs; a `Capabilities::builder()` driver with no `.session(..)` passed conformance (39/39) |
| New variant fails to compile in both drivers | MISSING | scratch edit not run (cap) |
| wildcard denied in both drivers | PASS | `clippy::wildcard_enum_match_arm = "deny"` is workspace-wide; clippy clean |
| OpenCode cost cap refused through `Refused` arm | PASS by test | `guard_opencode_driver` case agrees with `answer`; bite edit MISSING |
| `PathRow.reach` and the walk | PASS | `guard_driver_answers.rs`; bite edit MISSING |
| `guard_silent_acceptance.rs` rows read `answer` | MISSING | not edited |
| fmt, clippy, nextest (756 passed), doc | PASS | |

## What it leaves
- Development: one build slot, three full runs.
- Code: one shape per list. Deviations to decide: `Reach::Agent` takes a slice of keys (one row, `mcp`, reaches transport, codemode and name); `NO_PATH` holds only `Skills`; `AgentSetting::set_by` does not read `SessionMode` (the core refuses by the plan's session). `Env`, `Inputs` and `WallBudget` answers are stated as Candler's without a check against the core.
- Consumption: refusal messages are unchanged text; per-name tool detail rows of the old OpenCode table are gone from the code (name validation stays).
