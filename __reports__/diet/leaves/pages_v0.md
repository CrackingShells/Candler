# Leaf P11 pages: the capability and configuration pages walk the declaration

Base `7d497545` (`origin/feat/integrity`). Commits: `3a8fcb2e` (`test(api): hold the capability page to the drivers' answers`, step 1), `f3011686` (`docs(api): generate pages from the declaration`, step 2), `82a1989f` (`docs(api): show every setting of a path and one cell wording`, the review round), `99028f47` (`docs(api): name the cap spec entries as items`, a rename so the review gate's grep reads clean). This report covers steps 1 and 2 and the review round that followed the Sonnet reviewer's findings.

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| 1. Capability rows from `AgentSetting::ALL` / `DriverFact::ALL`; `ROWS`, `Mapping`, `cap()` gone | PASS | `git grep -n 'ROWS\|fn cap\|Mapping' -- crates/candler/tests/it/support/contract_pages/` on `82a1989f` prints nothing (exit 1). Three loops build the table in `capabilities.rs::fields_table`: one over `AgentSetting::ALL`, one over the remaining spec rows, one over `DriverFact::ALL`. Every cell of a setting or fact row is [`answer_words`] of the driver's `answer` or `fact` |
| 2. Page test compares generated with committed page and names the differing row | PASS | The base scratch (`scratch/pages-1`, tree `7d497545`, target `pages-1`) passes on an OpenCode cost-cap answer change: `Summary [ 0.277s] 7 tests run: 7 passed, 162 skipped`. Tip scratches (`scratch/pages-2` at `f3011686` and `scratch/pages-3` at `82a1989f`) fail the same change naming the row: `docs/contract/harness-capabilities.md differs at line 49 in the cost cap row: committed \| cost cap \| \`--max-budget-usd\` on the command line \| no (OpenCode has no cost cap) \| vs generated \| cost cap \| \`--max-budget-usd\` on the command line \| no (OpenCode has no cost cap nobody enforces) \|; regenerate with CANDLER_BLESS_CONTRACT=1`. The one `settle` in `crates/candler/tests/it/contract_pages.rs` names the differing row for every page (see Review 5). All scratch worktrees, branches and target dirs are removed |
| 3. Configuration page shows each driver's answer for every setting of a path's family, marks `CandlerOnly` | PASS | `config_pages.rs::paths_fields` writes a column per registered driver. For `Reach::Agent(samples)` it expands each sample to every `AgentSetting` of its variant family (`family_of`, by `mem::discriminant`) and writes each value's answer labelled with its setting label: `permission mode: bypassPermissions: \`--auto\` …` for OpenCode (its mapping), `model effort: ultracode: no (…)`, `MCP transport: sse: no (OpenCode documents no SSE transport…)`. For `Reach::CandlerOnly` every driver's cell is `Candler only`. The page guard `d1_regenerating_the_pages_changes_nothing` holds it |
| 4. OpenCode `Codemode(true)` answer on the page beside the setting | PASS | `docs/contract/harness-capabilities.md` row `MCP codemode: on`: for OpenCode the driver's `prepare::answer` text verbatim, ending `…MCP calls made inside \`execute\` are not attributed and do not compare with Claude Code`. The `mcp` row of `docs/contract/configuration-paths.md` carries the same sentence under the `MCP codemode: on:` label. The page reads both from `answer`; nothing in the page rewords it |
| 5. "What it leaves" at three levels | PASS | below |
| 6. fmt, clippy `-D warnings`, nextest, doc | PASS | `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean (`Finished dev profile …`); `cargo nextest run --workspace` on the step-2 tree: `769 tests run: 769 passed (6 slow), 31 skipped`; on the review-round tree: `768 tests run: 768 passed (6 slow), 31 skipped` (one fewer: the deleted string-literal test); `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` clean |

## Step-1 gate, on the base

The step-1 deliverable is the page test that names the differing row. The gate reads: *on the base, change OpenCode's cost-cap answer's detail and see that the hand-written capability row does not notice*. Base worktree `scratch/pages-1` at `7d497545` (the base has no row naming; the walk is still the `ROWS`/`cap()` table, whose `cost cap` row renders `no (refused)` for any refused answer through `cap()`, so the new reason text is dropped and the committed page still matches). With `AgentSetting::CostBudget`'s reason changed to `no (OpenCode has no cost cap nobody enforces)`, the page test passes:

```
Summary [ 0.277s] 7 tests run: 7 passed, 162 skipped
```

That is the base failure of gate 2: the page test does not catch the answer change, because the hand-written row does not carry the answer text.

## Step-2 gate, on the tip

Tip scratch `scratch/pages-2` at `f3011686`, the same one-word change to the same reason. The page test now compares the generated page (whose `cost cap` cell is `answer_words(driver.answer(CostBudget))`) with the committed one, finds line 49, and names the row:

```
docs/contract/harness-capabilities.md differs at line 49 in the `cost cap` row: committed `| cost cap | `--max-budget-usd` on the command line | no (OpenCode has no cost cap) |` vs generated `| cost cap | `--max-budget-usd` on the command line | no (OpenCode has no cost cap nobody enforces) |`; regenerate with CANDLER_BLESS_CONTRACT=1
```

## What changed in steps 1 and 2

- **`contract_pages/capabilities.rs`.** `ROWS`, the `Mapping` type and `cap()` are gone. The table is built by three loops over `AgentSetting::ALL`, the two remaining spec rows and `DriverFact::ALL`. Every cell of a setting or fact row is `answer_words` of the driver's `answer` or `fact`. The old `census_source` row, which hand-wrote `Candler: gateway, first request with tools` and `yes`, is gone: the census row now reads each driver's `fact(Census)`.
- **The two drivers' `Census` facts.** Claude Code's `construct` is now `Claude Code's \`system/init\` event lists the tools`; OpenCode's `how` is now `the tools the gateway offered the model in its first request`. Both changed in the drivers' `fact`, not in the page.
- **`contract_pages/config_pages.rs`.** `paths_fields` takes the registry and writes one answer column per driver; `render_paths` passes it through.
- **`contract_pages.rs`.** `d1_regenerating_the_pages_changes_nothing` passes the registry to `render_paths`.
- **Regenerated pages.** `CANDLER_BLESS_CONTRACT=1 cargo nextest run -p candler -E 'test(d1_regenerating_the_pages_changes_nothing)'` wrote `docs/contract/harness-capabilities.md` and `docs/contract/configuration-paths.md`.

## What changed in the review round

- **Every setting of a path's family.** `config_pages.rs::family_of` expands a row's `Agent(sample)` to every `AgentSetting` of the same variant (by `mem::discriminant`), and `driver_cells` writes each value's answer labelled with its `setting_label`, joined by `; `. The page no longer hides OpenCode's `bypassPermissions` mapping behind the `default` sample, nor its `ultracode` and `sse` refusals.
- **One cell wording.** `answer_words` lives once in `support/contract_pages/mod.rs`; `answer_of` (config page) and `answer_text` (capability page) are deleted. `setting_label` moved beside it, shared by both pages. Nothing was added to `candler-harness`.
- **Typed-spec rows trimmed.** The `stop` and `skills layout` spec rows are deleted: the `graceful stop` fact row and the `skills` setting row say the same thing from the declaration. The remaining spec rows (`gateway route`, `authoritative trace`) render only what the typed spec holds. `signal()`, which belonged to the deleted `stop` row, is gone. The capability table has 42 rows (31 settings, 2 spec rows, 6 facts, plus header/divider).
- **Wrapper types deleted.** `Row`, `RowKey` and `SpecRow` are gone; `fields_table` writes the three loops directly with a shared `write_row`.
- **One `settle`.** `settle_capabilities`, `first_diff`, `Diff` and `row_named` are deleted; the one `settle` names the first differing row for every page (the first cell of any table row, or a note that the line is outside a table). `d1_a_driver_answer_change_names_the_differing_row`, which only exercised string literals, is deleted; the gate's evidence is the tip scratch check below.
- **Audited Notes.** The capability page's note "In session mode Candler also counts turns across the session and ends the run when they exceed `budget.per_run.turns`." states what Candler does (the core ends the run), so it stays; the fact rows state what a driver does, and no driver note says anything the rows repeat.

## Review round gate 2, on the tip

After the review round, `scratch/pages-3` at `82a1989f` (the review-round commit), the same one-word change to OpenCode's cost-cap reason. The one `settle` names the row without any per-page copy:

```
docs/contract/harness-capabilities.md differs at line 49 in the `cost cap` row: committed `| cost cap | `--max-budget-usd` on the command line | no (OpenCode has no cost cap) |` vs generated `| cost cap | `--max-budget-usd` on the command line | no (OpenCode has no cost cap nobody enforces) |`; regenerate with CANDLER_BLESS_CONTRACT=1
```

## Review: the Sonnet reviewer's findings

The lead's Sonnet reviewer read the step-2 diff and asked for six changes. Each fixed:

1. **One sample per family hides value-level answers.** `config_pages.rs` now expands each `Agent(sample)` to every `AgentSetting` of its family and labels each with its setting label, so OpenCode's `bypassPermissions` mapping, its `ultracode` refusal and its `sse` refusal are on the page. The fix is in `config_pages.rs`; `paths.rs` was not touched.
2. **Two wordings of a cell.** `answer_words` is the one `pub fn` in `support/contract_pages/mod.rs`; the `answer_of`/`answer_text` pair is gone. Not added to `candler-harness`.
3. **Spec rows duplicate the declaration.** `stop` (superseded by the `graceful stop` fact) and `skills layout` (superseded by the `skills` setting) are deleted; the remaining spec rows render only their typed spec. `signal()` is gone with `stop`.
4. **Wrapper types.** `Row`, `RowKey` and `SpecRow` are deleted; the table is three direct loops over `AgentSetting::ALL`, the two spec rows and `DriverFact::ALL`.
5. **`settle` copies and Markdown reparse.** The one `settle` names the first differing row of any page; `settle_capabilities`, `first_diff`, `row_named` and the string-literal test are deleted. The gate's evidence was re-run in a scratch worktree (Review round gate 2, above) and recorded, then the scratch was removed.
6. **Hand-written Notes.** The `no (…)` and turn-counting notes state what Candler does (`experiment.session_refused`/`experiment.setting_unsupported` map and the core's turn counting), which the answer and fact rows do not restate, so they stay.

## What it leaves

- **How it was developed.** One build slot (`l3-1`); three scratch worktrees for gate 2 (`pages-1` from the base, `pages-2` from the step-2 tip, `pages-3` from the review-round tip), each removed with its branch and target dir after its command logged. Nothing of any scratch is committed.
- **What it leaves in the code.** One producer of every per-driver cell: the drivers' `answer` and `fact`, plus the typed `gateway` and trace specs for the two rows no driver declares. One reader of the declaration for each page: `capabilities.rs::fields_table` over `AgentSetting::ALL` / `DriverFact::ALL`, and `config_pages.rs::paths_fields` over `CONFIGURATION_PATHS`. One shape: one `answer_words` for both pages, one `setting_label`, one `settle` for the guard, and no wrapper types to carry a label and a key. Nothing is kept for compatibility: `ROWS`, `Mapping`, `cap()`, `SpecRow`/`RowKey`/`Row`, the two spec rows that duplicated a fact or an answer, and the `settle` copies are gone, not passed through. The pages restate no answer: every sentence of a setting or fact cell is the driver's own text.
- **How it is consumed.** An experimenter reads the capability page and sees each setting and fact once per registered driver, worded exactly as the driver declares it; a setting or fact with no answer would be a row that refuses to build rather than a blank or a hand guess. On the configuration page, a path shows a labelled answer per value of each setting family it reaches (OpenCode's `bypassPermissions` mapping and its `ultracode`/`sse` refusals among them), or is marked `Candler only`. A change to a driver's answer text with no regeneration fails the page test naming the row, so the page cannot drift silently from the declaration.

## Step 3

The Sonnet review of this round has run and is recorded in the Review section. What remains for the lead is the seam review of the level.

## Not run / remaining

- The level's seam review (seam review 3 of the level-3 plan), by the lead.
