# Settings declaration

**Goal**: One typed declaration, `AgentSetting`, names every setting that reaches the agent, and every registered driver must answer each one as mapped (with its harness construct), provided by Candler, or refused with a reason (P8).
**Estimate**: Sonnet, $10, cap $20. At the cap, stop, commit what is green, and list what is left. Builds use the target dir the campaign README names, removed at the leaf's merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor origin/feat/integrity HEAD` succeeds
- [ ] `[nl]` The level-3 plan is approved (A3)
**Success Gates**:
- ⬜ A cell that sets `budget.per_run.max_cost_usd` on OpenCode is refused through its `Refused` answer, and the hand-written refusal it replaces is gone; on the base the same refusal comes from `prepare::settings`
- ⬜ A scratch branch that adds one `AgentSetting` variant fails to compile in both drivers until each answers it (recorded in the report, not committed)
- ⬜ Claude Code answers every entry; the conformance suite fails a driver whose table misses an entry or answers one twice
- ⬜ Every `AgentSetting` is named by a `CONFIGURATION_PATHS` row through `PathRow.agent`, or is declared to have no experiment path (hooks); a guard fails when a variant is named by neither
- ⬜ `guard_silent_acceptance.rs`'s rows that state a driver's answer read it from the declaration instead of restating it
- ⬜ The report's "What it leaves" section judges the change at the three levels
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with the target dir the campaign README names
**References**: [Level-3 plan §Design](../../../../../../../../__reports__/diet/07-plan_integrity_level3_v0.md) — the declaration and its answers; `crates/candler-harness/src/driver.rs` — `SettingRow`, `settings_table`, `check_settings`; `crates/candler-harness/src/conformance.rs` — `check_settings_table`; `crates/candler-core/src/resolve/paths.rs` — `CONFIGURATION_PATHS`, `PathRow`; `crates/candler-harness-opencode/src/prepare.rs` — `table`, `settings`; `crates/candler-harness-claude-code/src/session.rs` — `spec` (hooks through `--settings`)

## Step 1: Guard on the base
**Goal**: The refusal and completeness guards fail on the base for the reason the leaf removes.
**Implementation Logic**:
Write the conformance completeness check first: every `AgentSetting` has exactly one answer from each registered driver. On the base, Claude Code has no table, so the check cannot pass, and OpenCode's refusal of a cost cap comes from code beside its table. Record both failures. The entries are what reaches the agent: model id, endpoint and effort; permission mode; allowed and disallowed tools; `env`; MCP servers with their transport, `codemode` and `name`; skills from the treatment; instruction files; `inputs`; the wall, turn and cost budgets; session mode; hooks (Candler-owned, no experiment path). Cite the Claude Code settings documentation (Context7) for how `--settings` stands beside `--setting-sources project,local`, so the hooks answer says what is true.
**Deliverables**: `crates/candler-harness/src/conformance.rs` (`check_settings_table` extended to completeness); the guard in `crates/candler-core/tests/it/` for `PathRow.agent` coverage
**Consistency Checks**: the completeness check on the base (expected: FAIL)
**Commit**: `test(library): require every driver to answer every agent setting`

## Step 2: Declare and answer
**Goal**: The declaration exists, both drivers answer it, and refusals come from the answers.
**Implementation Logic**:
Add `AgentSetting` (exhaustive, `#[non_exhaustive]` for outside crates) and `Answer { Mapped { construct }, Candler { how }, Refused { reason } }` with value-level answers for the settings whose values form a closed set (permission modes, effort levels, transports, `codemode`). `HarnessDriver` gains a required `answer`; `settings_table()` is derived from it, not overridable. `check_settings` refuses a set value whose answer is `Refused`, through `experiment.setting_unsupported`. Move OpenCode's hand-written refusals into its answers and delete them; write Claude Code's answers. `PathRow` gains `agent: Option<AgentSetting>`. Do not add a second table: the declaration and `CONFIGURATION_PATHS` are one vocabulary joined by that field. Keep the capability page compiling with the smallest change to its `cap` mapping; `pages.md` regenerates it.
**Deliverables**: `crates/candler-harness/src/driver.rs` (`AgentSetting`, `Answer`, `HarnessDriver::answer`, derived `settings_table`); `crates/candler-harness-opencode/src/{driver.rs,prepare.rs}`; `crates/candler-harness-claude-code/src/driver.rs`; `crates/candler-core/src/resolve/paths.rs` (`PathRow.agent`); `crates/candler/tests/it/guard_silent_acceptance.rs`
**Consistency Checks**: the completeness check and the coverage guard on the tip, with the scratch variant's compile failure recorded in the report (expected: PASS)
**Commit**: `feat(library): declare the agent settings and have every driver answer them`

## Step 3: Bite and verify
**Goal**: The guards catch a missing answer and a hand-written refusal.
**Implementation Logic**:
On a scratch branch, make OpenCode's cost-cap answer `Mapped` and see the refusal guard fail; drop one row's `agent` and see the coverage guard fail. A fresh Sonnet verifier gets this leaf, the report and the diff, with the brief: find a setting that reaches the agent and is answered by no driver, or answered in two places.
**Deliverables**: `__reports__/diet/leaves/declaration_v0.md`
**Consistency Checks**: both scratch edits (expected: FAIL)
**Commit**: `reports(api): close the settings declaration`
