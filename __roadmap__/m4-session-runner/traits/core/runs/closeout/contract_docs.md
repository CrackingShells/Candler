# Contract docs

**Goal**: A coding agent that writes a harness driver has a contract page for the driver traits in A5's frame (amendment A5), generated from the trait definitions where M3's generator exists, and every page of a surface M4 changed is regenerated with its tentative Notes re-stamped.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] The coordination session ruled on the order with M3: whether M3's frame and generator are on `main` (and rebased into this milestone) before this leaf starts
- [ ] [nl] OWNED: `docs/contract/driver-traits.md`, the driver-traits entry of M3's generator if it exists, `docs/contract/experiment-file.md` regenerated if it exists; `crates/candler-harness/tests/guard_contract_page.rs` when the page is written by hand
**Success Gates**:
- ⬜ [cmd] The page exists and keeps the frame's sections, in order and no others: `test -f docs/contract/driver-traits.md && grep -E '^## ' docs/contract/driver-traits.md | tr -d '\n' | grep -qx '## Surface## Version## Fields## Refusals## Notes (tentative)'`
- ⬜ [cmd] The page matches the code: M3's regeneration check leaves it unchanged when the generator exists; otherwise the drift guard passes (every public item of `HarnessDriver`, `TraceReader` and `SkillTree` is a row of Fields, and every row is an item): `cargo nextest run -p candler-harness --no-tests=fail -E 'binary(guard_contract_page)'` or M3's check
- ⬜ [cmd] The Notes stay within 120 words and carry a version stamp: `sed -n '/^## Notes/,$p' docs/contract/driver-traits.md | tail -n +2 | wc -w | awk '$1>120{exit 1}' && sed -n '/^## Notes/,$p' docs/contract/driver-traits.md | grep -qE 'checked against v[0-9]'`
- ⬜ [nl] Every page of a surface M4 changed (the experiment file, for `runner.*`) is regenerated when it exists, with its Notes re-stamped; the report says which pages existed
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
