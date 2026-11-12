# Contract docs

**Goal**: A coding agent that writes a harness driver has a contract page for the driver traits in A5's frame (amendment A5), written by hand and held to the trait definitions by a drift guard until it is folded into M3's generator; an experimenter choosing a harness has a generated capability page that shows, per harness, what a run can and cannot do; and every page of a surface M4 changed is regenerated with its tentative Notes re-stamped.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [x] [nl] The coordination session ruled on the order with M3 (2026-11-11): do not wait; write the page by hand in the frame with a drift guard; the milestone that merges to `main` second folds it into M3's generator (`crates/candler/tests/contract_pages.rs`)
- [ ] [nl] OWNED: `docs/contract/driver-traits.md`, the driver-traits entry of M3's generator if it exists, `docs/contract/experiment-file.md` regenerated if it exists; `crates/candler-harness/tests/guard_contract_page.rs` when the page is written by hand; `docs/contract/harness-capabilities.md`, its row in `docs/contract/index.md`, and its entry in M3's generator (`crates/candler/tests/support/contract_pages/`)
**Success Gates**:
- ⬜ [cmd] The page exists and keeps the frame's sections, in order and no others: `test -f docs/contract/driver-traits.md && grep -E '^## ' docs/contract/driver-traits.md | tr -d '\n' | grep -qx '## Surface## Version## Fields## Refusals## Notes (tentative)'`
- ⬜ [cmd] The page matches the code: M3's regeneration check leaves it unchanged when the generator exists; otherwise the drift guard passes (every public item of `HarnessDriver`, `TraceReader` and `SkillTree` is a row of Fields, and every row is an item): `cargo nextest run -p candler-harness --no-tests=fail -E 'binary(guard_contract_page)'` or M3's check
- ⬜ [cmd] The Notes stay within 120 words and carry a version stamp: `sed -n '/^## Notes/,$p' docs/contract/driver-traits.md | tail -n +2 | wc -w | awk '$1>120{exit 1}' && sed -n '/^## Notes/,$p' docs/contract/driver-traits.md | grep -qE 'checked against v[0-9]'`
- ⬜ [nl] Every page of a surface M4 changed (the experiment file, for `runner.*`) is regenerated when it exists, with its Notes re-stamped; the report says which pages existed
- ⬜ [cmd] The capability page is generated from the registered drivers and keeps the frame: `test -f docs/contract/harness-capabilities.md && grep -E '^## ' docs/contract/harness-capabilities.md | tr -d '\n' | grep -qx '## Surface## Version## Fields## Refusals## Notes (tentative)'`, and M3's regeneration check leaves it unchanged
- ⬜ [cmd] The capability page bites: a driver whose declared capability changes (for example OpenCode declaring `session`) makes the regeneration check fail until the page is regenerated: the check in `crates/candler/tests/contract_pages.rs`, with the bite recorded in the leaf report
- ⬜ [cmd] SUBJ passes
**References**: [R05 Gap analysis A5](../../../../../../__reports__/contract-docs/gap_analysis_contract_docs_v0.md) — the frame (field classes, sections, Notes cap, checks); [R01 §Harness drivers](../../../../../../__design__/architecture.md)

## Step 1: Write the driver-traits page
**Goal**: The page lists every trait item with its class, generated or guarded against drift.
**Implementation Logic**:
1. Surface: one sentence (what the traits are, who implements them, who calls them). Version: the core line and the `candler-harness` crate's version. Fields: one row per trait item (path, signature, class: `required` for an item without a default, `optional` with its default otherwise). Refusals: the `DriverError` and resolve refusals a driver can cause (codes if M3's codes exist, else the variant names). Notes (tentative): only rules the table cannot state (drivers never act on processes; the reader is synchronous), stamped with the version they were checked against.
2. If M3's generator is on the milestone branch, add the driver-traits surface to it and generate the page. Otherwise write the page by hand in the frame and add `guard_contract_page.rs`, which parses the trait items from `candler-harness/src` and the Fields table and fails on any difference.
3. Regenerate the experiment-file page if it exists.
**Deliverables**: `docs/contract/driver-traits.md`; `crates/candler-harness/tests/guard_contract_page.rs` or the generator's driver-traits entry
**Consistency Checks**: `test -f docs/contract/driver-traits.md` (expected: PASS)
**Commit**: `docs(runner): add the driver-traits contract page`

## Step 2: Generate the harness capability page
**Goal**: A reader sees at a glance, per harness, what a run can do, what Candler supplies in its place, and what is refused.
**Implementation Logic**:
1. `docs/contract/harness-capabilities.md` in the frame. Surface: one sentence (what a run can expect of each harness). Version: the core line. Fields: one table, one row per capability, one column per registered driver, plain cells only:
   - rows: gateway route, census source, session mode, turn cap, cost cap, stop, authoritative trace, skills layout, MCP code mode;
   - each cell is one of `yes` (the harness does it), `Candler: <how>` in a few words (for example `Candler: counts turns across the session`, `Candler: gateway, first request with tools`), or `no (refused)` when resolve refuses a cell that asks for it.
   Refusals: the resolve refusals a missing capability causes (codes from M3's registry). Notes (tentative): only what the table cannot say, within 120 words, stamped with the version.
2. Generate the table from the binary's driver registry: each row reads a declared item (`Capabilities`, `census_stream`, `session`, `stop`, the offline reader, the skill tree), never prose kept beside the code. A cell's wording comes from one mapping per row in the generator, so a reader never meets two spellings of one fact.
3. Add the page's row to `docs/contract/index.md`.
**Deliverables**: `docs/contract/harness-capabilities.md`; `docs/contract/index.md`; the page's entry in `crates/candler/tests/support/contract_pages/`
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'binary(contract_pages)'` (expected: PASS)
**Commit**: `docs(runner): add the harness capability page`
