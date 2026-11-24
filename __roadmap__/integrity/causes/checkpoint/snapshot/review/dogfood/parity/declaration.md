# Settings declaration

**Goal**: Every per-harness list has one shape: a fieldless key enum with a `const ALL`, a `Copy` answer of `&'static str`, and one exhaustive `match` per driver. `AgentSetting` holds what an experiment sets that reaches the agent. `DriverFact` holds what a driver does on its own and replaces `Capabilities`. Every `CONFIGURATION_PATHS` row says whether it reaches the agent (P8).
**Estimate**: Sonnet, $12, cap $24, with the lead reviewing at each joint. At the cap, stop, commit what is green, and list what is left. Builds use the target dir the campaign README names, which is removed when the leaf merges.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor origin/feat/integrity HEAD` succeeds
- [ ] `[nl]` The level-3 plan, revision 2, is approved (A3)
**Success Gates**:
- ⬜ `SettingRow`, `HarnessDriver::settings_table`, `Capabilities`, `CapabilitiesBuilder` and `BudgetCaps` are gone from the tree, and nothing replaces them with a second table
- ⬜ A test driver that omits its session-mode answer compiles and passes conformance on the base, through the builder's default `false`. On the tip, a scratch branch that adds one variant to `AgentSetting` and one to `DriverFact` fails to compile in both drivers until each answers it. The failure is recorded in the report and not committed
- ⬜ Both drivers deny `clippy::wildcard_enum_match_arm`, so a `_ =>` arm cannot stand in for an answer
- ⬜ A cell that sets `budget.per_run.max_cost_usd` on OpenCode is refused through its `Refused` arm with `experiment.setting_unsupported`. OpenCode's hand-written refusal lists in `prepare.rs` are gone
- ⬜ `PathRow` has a required `reach` column (`Agent(AgentSetting)` or `CandlerOnly`) that `row()` takes as an argument. A test in `crates/candler` walks every row for every registered driver, and fails when an `AgentSetting` is reached by no row and is not in the `NO_PATH` const
- ⬜ `guard_silent_acceptance.rs`'s rows read each driver's answer from `answer` instead of restating it
- ⬜ Hooks and instruction files are not `AgentSetting` variants. They are a later feature round (non-goal)
- ⬜ The report's "What it leaves" section judges the change at the three levels
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with the target dir the campaign README names
**References**: [Level-3 plan §Design](../../../../../../../../__reports__/diet/07-plan_integrity_level3_v1.md), on the one shape for every per-harness list. Code:
- `crates/candler-core/src/resolve/paths.rs`: `CONFIGURATION_PATHS`, `PathRow` and `row`, the idiom kept;
- `crates/candler-schemas/src/refusal.rs`: `refusal_table!`, the exhaustive match kept;
- `crates/candler-harness/src/driver.rs`: `SettingRow`, `settings_table`, `check_settings`;
- `crates/candler-harness/src/spec.rs`: `Capabilities`, `CapabilitiesBuilder`;
- `crates/candler-harness/src/conformance.rs`: `check_settings_table`, `check_capabilities`;
- `crates/candler-harness-opencode/src/prepare.rs`: `table`, `settings`.

## Step 1: Guard on the base
**Goal**: The completeness guards fail on the base, for the reason the leaf removes.
**Implementation Logic**:
Write the walk over every path for every driver in `crates/candler`. Write the conformance check that a driver answers every key. On the base, show that a test driver built with `Capabilities::builder()` and no `.session(..)` passes conformance with the default `false`, and that Claude Code returns no settings table. Record both observations. The keys are:
- for `AgentSetting`: model id, endpoint and effort; permission mode; allowed and disallowed tools; `env`; `inputs`; skills; MCP servers, with transport, `codemode` and `name`; the wall, turn and cost budgets; and session mode;
- for `DriverFact`: graceful stop, live events, the in-harness guard, execution outside the child, and the census.

A key whose values form a closed set carries the value, as in `Effort(EffortLevel)`, `Transport(Transport)` and `Codemode(bool)`.
**Deliverables**: the walk in `crates/candler/tests/it/`; `crates/candler-harness/src/conformance.rs`
**Consistency Checks**: the walk and the conformance check on the base (expected: FAIL)
**Commit**: `test(library): require every driver to answer every setting and fact`

## Step 2: One shape
**Goal**: Each fact is stated once per driver, in an exhaustive match, and every list reads it.
**Implementation Logic**:
- Add `Answer` (`Mapped { construct }`, `Candler { how }`, `Refused { reason }`, all `&'static str`), `AgentSetting` and `DriverFact` to `candler-harness`, each with a `const ALL`.
- `HarnessDriver` gains `answer` and `fact`, both required and with no default.
- Delete `SettingRow`, `settings_table`, `Capabilities`, its builder and `BudgetCaps`. Every reader of a capability bool reads `fact` or `answer` instead.
- `check_settings` refuses a set value whose answer is `Refused`. OpenCode's refusal lists become match arms, and the lists are deleted.
- `PathRow` gains `reach`, and every `row(...)` call names it.
- Keep the capability page compiling with the smallest change; `answered/pages.md` regenerates it.

Do not keep a wrapper that rebuilds the old structs for any reader.
**Deliverables**: `crates/candler-harness/src/{driver.rs,spec.rs,conformance.rs}`; `crates/candler-harness-opencode/src/{driver.rs,prepare.rs}`; `crates/candler-harness-claude-code/src/driver.rs`; `crates/candler-core/src/resolve/paths.rs`; the readers of the old capability fields in `crates/candler-core`; `crates/candler/tests/it/guard_silent_acceptance.rs`
**Consistency Checks**: the walk and the conformance check on the tip, with the scratch variant's compile failure recorded in the report (expected: PASS)
**Commit**: `refactor(library): state every per-harness fact in one shape`

## Step 3: Bite and verify
**Goal**: The guards catch a missing answer and a restated one.
**Implementation Logic**:
On a scratch branch, make two edits and watch each guard fail:
- make OpenCode's cost-cap arm `Mapped`, and see the refusal guard fail;
- move one setting's path to `CandlerOnly` while it is not in `NO_PATH`, and see the walk fail.

A fresh Sonnet verifier gets this leaf, the report and the diff. Its brief is to find a fact about a harness stated outside a driver's `answer` or `fact`, or a setting that reaches the agent with no key.
**Deliverables**: `__reports__/diet/leaves/declaration_v0.md`
**Consistency Checks**: both scratch edits (expected: FAIL)
**Commit**: `reports(api): close the settings declaration`
