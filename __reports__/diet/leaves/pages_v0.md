# Leaf P11 pages: the capability and configuration pages walk the declaration

Base `7d497545` (`origin/feat/integrity`), tip `3a8fcb2e` (step 1) then `f3011686` (step 2). Commits: `3a8fcb2e` (`test(api): hold the capability page to the drivers' answers`), `f3011686` (`docs(api): generate pages from the declaration`). This report covers steps 1 and 2; step 3's Sonnet review is the lead's.

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| 1. Capability rows from `AgentSetting::ALL` / `DriverFact::ALL`; `ROWS`, `Mapping`, `cap()` gone | PASS | `git grep -n 'ROWS\|fn cap\|Mapping' -- crates/candler/tests/it/support/contract_pages/` on `f3011686` prints nothing (exit 1). The rows are built in `capabilities.rs::rows()` as one row per `AgentSetting::ALL` entry, then the four typed-spec rows, then one per `DriverFact::ALL` entry; every cell of a setting or fact row is `answer_text(driver.answer(setting))` or `answer_text(driver.fact(fact))` |
| 2. Page test compares generated with committed page and names the differing row | PASS | Base scratch (`scratch/pages-1`, tree `7d497545`, target `pages-1`): with OpenCode's cost-cap answer changed to `no (OpenCode has no cost cap nobody enforces)`, `cargo nextest run -p candler -E 'test(contract_pages)'` ends `Summary [ 0.277s] 7 tests run: 7 passed, 162 skipped`: the hand-written `cost cap` row still renders `no (refused)` from `cap()`, which equals the committed page, so the change goes unnoticed. Tip scratch (`scratch/pages-2`, tree `f3011686`, target `pages-2`): the same one-word change fails the same command, naming the row: `docs/contract/harness-capabilities.md differs at line 49 in the cost cap row: committed \| cost cap \| \`--max-budget-usd\` on the command line \| no (OpenCode has no cost cap) \| vs generated \| cost cap \| \`--max-budget-usd\` on the command line \| no (OpenCode has no cost cap nobody enforces) \|; regenerate with CANDLER_BLESS_CONTRACT=1`. Both scratch worktrees, their branches and their target dirs are removed. The row-naming is `settle_capabilities` in `crates/candler/tests/it/contract_pages.rs`, which finds the first differing line and, when it is a table row, prints its first cell as the differing row; `d1_a_driver_answer_change_names_the_differing_row` holds it. On the tip a change to a row outside the table reports `a line of the surface outside the table` |
| 3. Configuration page shows each driver's answer for `Agent(..)` paths, marks `CandlerOnly` | PASS | `config_pages.rs::paths_fields` writes a column per registered driver. For `Reach::Agent(settings)` it joins each driver's `answer_text(driver.answer(setting))` for the row's keys into one cell (`docs/contract/configuration-paths.md` rows 20–28, 52–54, 56, 60: the `mcp` cell carries three answers separated by `; `, the budget rows the two drivers' words); for `Reach::CandlerOnly` every driver's cell is `Candler only` (rows 23, 29–51, 55, 57–59, 61–68). The page guard `d1_regenerating_the_pages_changes_nothing` holds it |
| 4. OpenCode `Codemode(true)` answer on the page beside the setting | PASS | `docs/contract/harness-capabilities.md` row `MCP codemode: on`: `no (Claude Code offers MCP tools directly; it has no code mode)` for Claude Code and, for OpenCode, the driver's `prepare::answer` text verbatim, ending `…MCP calls made inside \`execute\` are not attributed and do not compare with Claude Code` (line 45). The `mcp` row of `docs/contract/configuration-paths.md` carries the same sentence (line 27). The page reads both from `answer`; nothing in the page rewords it |
| 5. "What it leaves" at three levels | PASS | below |
| 6. fmt, clippy `-D warnings`, nextest, doc | PASS | `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean (`Finished dev profile …`); `cargo nextest run --workspace`: `769 tests run: 769 passed (6 slow), 31 skipped`; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` clean (`Finished … in 7.86s`) |

## Step-1 gate, on the base

The step-1 deliverable is the page test that names the differing row. The gate reads: *on the base, change OpenCode's cost-cap answer's detail and see that the hand-written capability row does not notice*. Base worktree `scratch/pages-1` at `7d497545` (the base has no `settle_capabilities` and no row naming; the walk is still the `ROWS`/`cap()` table, whose `cost cap` row renders `no (refused)` for any refused answer through `cap()`, so the new reason text is dropped and the committed page still matches). With `AgentSetting::CostBudget`'s reason changed to `no (OpenCode has no cost cap nobody enforces)`, the page test passes:

```
Summary [ 0.277s] 7 tests run: 7 passed, 162 skipped
```

That is the base failure of gate 2: the page test does not catch the answer change, because the hand-written row does not carry the answer text. The gate's phrasing records it as the observation the leaf requires — a PASS for "the base does not notice", which is what the gate asks us to show.

## Step-2 gate, on the tip

Tip scratch `scratch/pages-2` at `f3011686`, the same one-word change to the same reason. The page test now compares the generated page (whose `cost cap` cell is `answer_text(driver.answer(CostBudget))`) with the committed one, finds line 49, and names the row:

```
docs/contract/harness-capabilities.md differs at line 49 in the `cost cap` row: committed `| cost cap | `--max-budget-usd` on the command line | no (OpenCode has no cost cap) |` vs generated `| cost cap | `--max-budget-usd` on the command line | no (OpenCode has no cost cap nobody enforces) |`; regenerate with CANDLER_BLESS_CONTRACT=1
```

## What changed

- **`contract_pages/capabilities.rs`.** `ROWS`, the `Mapping` type and `cap()` are gone. `rows()` walks `AgentSetting::ALL` and `DriverFact::ALL`, interleaved with the four rows no driver declares an answer for (`gateway route`, `stop`, `authoritative trace`, `skills layout`), which are `SpecRow::cell` reading `gateway()`, `stop()`, the registry's offline streams plus `live_reader()` with `fact(LiveEvents)`, and `skills()`. `answer_text` words the three answer forms: `Mapped { construct }` verbatim, `Candler { how }` as `Candler: {how}`, `Refused { reason }` as `no ({reason})`. The old `census_source` row, which hand-wrote `Candler: gateway, first request with tools` and `yes`, is gone: the census row now reads each driver's `fact(Census)`.
- **The two drivers' `Census` facts.** The leaf requires the fact text to say what the census is: Claude Code's is its `system/init`, OpenCode's is the tools its model was offered at the gateway. Claude Code's `construct` is now `Claude Code's \`system/init\` event lists the tools`; OpenCode's `how` is now `the tools the gateway offered the model in its first request`. Both changed in the drivers' `fact`, not in the page.
- **`contract_pages/config_pages.rs`.** `paths_fields` takes the registry, writes one answer column per driver, and for each row either prints the joined answers of the `Agent(..)` keys or `Candler only` once per driver for `CandlerOnly`. `render_paths` passes the registry through.
- **`contract_pages.rs`.** `d1_regenerating_the_pages_changes_nothing` passes the registry to `render_paths` and uses `settle_capabilities` for the capability page. `settle_capabilities`, `first_diff`, `Diff` and `row_named` name the differing row of the capability table; the old generic `settle` stays for the other pages.
- **Regenerated pages.** `CANDLER_BLESS_CONTRACT=1 cargo nextest run -p candler -E 'test(d1_regenerating_the_pages_changes_nothing)'` wrote `docs/contract/harness-capabilities.md` and `docs/contract/configuration-paths.md`. The capability page now has 44 rows (31 settings, 4 spec rows, 6 facts, plus header/divider) from 9; the configuration page has two new columns per row. The hand note in the capability page that said `no (refused)` now reads `no (…)`, matching the cell wording.

## What it leaves

- **How it was developed.** One build slot (`l3-1`); two scratch worktrees for gate 2 (`pages-1` from the base with target `pages-1`, `pages-2` from the tip with target `pages-2`), each removed with its branch and target dir after its command logged. `rsync` of the scratch answer change was made in each scratch tree; nothing of either scratch is committed.
- **What it leaves in the code.** One producer of every per-driver cell: the drivers' `answer` and `fact`, plus the typed specs (`gateway`, `stop`, `offline`/`live` trace, `skills`) for the rows no driver declares. One reader of the declaration for the page: `capabilities.rs::rows()` and `config_pages.rs::paths_fields`, both over `AgentSetting::ALL` / `DriverFact::ALL` / `CONFIGURATION_PATHS`. One shape: settings and facts have the same row type (`Row` with `RowKey`), and the two pages return the same `Answer` text through `answer_text`/`answer_of`. Nothing is removed than replaced: `ROWS`, `Mapping`, `cap()`, and the hand-written `census_source` row are gone, not kept for compatibility. The pages restate no answer: every sentence of a setting or fact cell is the driver's own text.
- **How it is consumed.** An experimenter reads the capability page and sees each setting and fact once per registered driver, worded exactly as the driver declares it; a setting or fact with no answer would be a row that refuses to build rather than a blank or a hand guess. On the configuration page, a path either shows what each harness does with it (the `mcp` row shows transport, codemode and name per driver, including OpenCode's `execute` attribution note) or is marked `Candler only`. A change to a driver's answer text with no regeneration now fails the page test naming the row, so the page cannot drift silently from the declaration.

## Step 3

Not run by this branch: the leaf's Step 3 is the lead's Sonnet review (`reports(api): close the generated pages`). This report, the diff and the regenerated pages are the input to it.

## Not run / remaining

- Step 3 review, by the lead.
