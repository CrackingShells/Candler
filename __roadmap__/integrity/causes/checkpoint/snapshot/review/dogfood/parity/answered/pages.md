# Generated pages

**Goal**: The capability page and the configuration contract page are walks over `AgentSetting::ALL`, `DriverFact::ALL`, `CONFIGURATION_PATHS` and the registered drivers, so an experimenter reads per harness what happens to each setting and fact (P11).
**Estimate**: RiVault, Reasoning-Medium first, then Reasoning-Large, then Sonnet if both fail; the Sonnet review is mandatory, $2, cap $4. Check that thinking ran on each RiVault model before trusting its output. At the cap, stop, commit what is green, and list what is left. Builds use the target dir the campaign README names, removed at the leaf's merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor origin/feat/integrity HEAD` succeeds
- [ ] `[nl]` `../declaration.md` is `done`
**Success Gates**:
- ⬜ The capability page's rows come from `AgentSetting::ALL` and `DriverFact::ALL` for each registered driver. `ROWS`, the `Mapping` fn pointers and `cap()`'s string match on `budget.per_run.turns` are gone. The rows built from typed specs (stop signal, gateway route, trace) read `fact` and the driver's specs, and nothing else
- ⬜ Changing one driver's answer without regenerating fails the page test; on the base, changing OpenCode's cost-cap row leaves the page test passing as long as the hand-written row still finds a refused row
- ⬜ The configuration contract page shows, for each path whose `reach` is `Agent(..)`, each driver's answer, and marks `CandlerOnly` paths as such
- ⬜ OpenCode's `Codemode(true)` answer, which says its trace does not attribute MCP calls inside `execute`, is on the page beside the setting
- ⬜ The Sonnet review's report says the pages add no second statement of an answer, and checks the principle as well as correctness
- ⬜ The report's "What it leaves" section judges the change at the three levels
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with the target dir the campaign README names
**References**: [Level-3 plan §Leaves](../../../../../../../../../__reports__/diet/07-plan_integrity_level3_v1.md), the pages row. Code: `crates/candler/tests/it/support/contract_pages/capabilities.rs` (`ROWS`, `cap`); `crates/candler/tests/it/support/contract_pages/config_pages.rs` (`paths_fields`), the configuration page that reads `CONFIGURATION_PATHS`

## Step 1: Guard on the base
**Goal**: The page test fails when an answer changes and the page does not.
**Implementation Logic**:
Write the page test so that it compares the generated page with the committed one and names the row that differs. On the base, change OpenCode's cost-cap row's detail and see that the hand-written capability row does not notice; record it.
**Deliverables**: the page test in `crates/candler/tests/it/`
**Consistency Checks**: the page test after the scratch answer change on the base (expected: PASS)
**Commit**: `test(api): hold the capability page to the drivers' answers`

## Step 2: Generate from the declaration
**Goal**: One source for every setting row.
**Implementation Logic**:
Replace `ROWS` with walks over `AgentSetting::ALL` and `DriverFact::ALL` for each registered driver, written from `Answer`. Add the answers to the configuration page through `PathRow.reach`. Write no new page, and nothing that restates an answer. Regenerate the committed pages.
**Deliverables**: `crates/candler/tests/it/support/contract_pages/{capabilities.rs,…}`; the regenerated pages under `docs/`
**Consistency Checks**: the page test on the tip, after the same scratch answer change (expected: FAIL)
**Commit**: `docs(api): generate the capability and configuration pages from the declaration`

## Step 3: Review
**Goal**: A Sonnet reviewer accepts the RiVault output.
**Implementation Logic**:
A fresh Sonnet reviewer gets this leaf, the diff and the regenerated pages, with the brief: find a statement of a driver's answer that the pages make without reading it from the declaration, and say whether the leaf left anything a reader of the pages would have to reconcile by hand.
**Deliverables**: `__reports__/diet/leaves/pages_v0.md`
**Consistency Checks**: the page test on the tip, unchanged (expected: PASS)
**Commit**: `reports(api): close the generated pages`
