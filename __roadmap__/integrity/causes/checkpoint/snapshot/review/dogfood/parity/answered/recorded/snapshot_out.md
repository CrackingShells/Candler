# Snapshot out

**Goal**: Candler's constant catalog producer, its resolve step and the level-2 consumer surface are gone. `inspect` and the op API show the MCP configuration Candler installed, read from the resolved `mcp.*` values the cell digest already hashes. The `tool-catalog-snapshot/1` schema stays for its outside producer and reader (P12).
**Estimate**: Sonnet, $8, cap $16. At the cap, stop, commit what is green, and list what is left. Builds use the target dir the campaign README names, which is removed when the leaf merges.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor origin/feat/integrity HEAD` succeeds
- [ ] `[nl]` `../trace_parity.md` is `done`
- [x] `[nl]` The coordinator ruled on the comparable change. `comparable.snapshots` leaves the cell digest under a new digest version, old manifests stay readable, new oracle vectors are added beside the old ones, and `InspectRun.catalog` leaves `inspect-run.schema.json`. Accepted, on the condition in the `inspect` callers gate below
**Success Gates**:
- ⬜ A guard on a real `candler run` path, with one declared server, reads the run back through `inspect` (as JSON and as text) and finds the server among the shown `mcp.*` values. On the base, `inspect` of the same run shows only a catalog that says `servers: []`
- ⬜ `CATALOG_PROGRAM`, `CatalogCommand`, `CommandCatalogProvider`, `CatalogStep` and the fake docker's catalog constant are gone. The provider seam behind them is gone too if colgrep finds no other implementor. Nothing in the tree prints a catalog it did not observe
- ⬜ `comparable` carries no catalog snapshot. The new digest version's oracle vectors are added, and the old ones still verify old manifests. The old vectors stay because the maintainer's own stores (golden and HTN runs) hold manifests of the old version, and the vector files say so in a comment, so nobody later prunes them as dead
- ⬜ Before `InspectRun.catalog` is dropped, the report records that nothing in htn-creator-eval@0.5.3 reads `inspect`'s `catalog` field (its Tier D reads its own `/1` file; the check is for `inspect` callers). If something reads it, the drop waits and the lead takes it to the coordinator
- ⬜ `InspectRun.catalog` and `inspect.rs`'s `catalog_of` are gone. A sealed run from before the change still reads through `inspect`. Its stored blob stays in the store and is not shown, with no shim
- ⬜ The `tool-catalog-snapshot/1` schema stays in `candler-schemas` for level 3: its types in `candler-schemas/src/catalog.rs` and its fixture tests are unchanged. Only its description changes, to say that Candler neither writes nor reads it and that the HTN consumer produces it: its `capture_catalog_snapshot.py` writes it and its Tier D reads it (htn-creator-eval@0.5.3). The generated JSON schema is regenerated
- ⬜ The README purpose paragraph says "per-run tool census" in place of "per-run tool-catalog snapshots". In the same commit, `AGENTS.md`'s "What sane work means in Candler" names the census, not a catalog snapshot, among the stored states, and the live candler-v1 M6c Goal, which quotes the old promise (`__roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/m5b/stats/surface/integrity/README.md`, line 7), is reworded the same way. The done leaves that quote the old phrase as history keep their words: wire-schemas, `catalog_snapshot.md`, `dogfooding_pass.md` and `diet/shed/e2e_audit.md`
- ⬜ Refusal codes that only the removed code emitted leave `codes.toml` and the refusal tables, and the contract pages are regenerated
- ⬜ The report's "What it leaves" section judges the change at the three levels
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with the target dir the campaign README names, and `container-e2e` is green on the pull request
**References**: [Level-3 plan §Design](../../../../../../../../../../__reports__/diet/07-plan_integrity_level3_v1.md), on the snapshot surface out. [Dogfooding findings, finding (1)](../../../../../../../../../../__reports__/diet/05-findings_dogfooding_pass_v0.md), on the cause chain. Code:
- `crates/candler/src/commands/preflight.rs`: `CATALOG_PROGRAM`;
- `crates/candler-core/src/runner/evidence/catalog.rs`: `CommandCatalogProvider`;
- `crates/candler-core/src/resolve/steps/catalog.rs`: `CatalogStep`;
- `crates/candler/src/commands/inspect.rs`: `catalog_of`, `render_run`, `text_of_run`;
- `crates/candler/tests/it/support/cli_run.rs`: the fake docker;
- `crates/candler/tests/it/guard_catalog_snapshot.rs`;
- `crates/candler-schemas/src/execution.rs`: `Comparable`.

## Step 1: Guard on the base
**Goal**: The `inspect` guard fails on the base, where the shown record of a declared server is a constant.
**Implementation Logic**:
Write the guard on a run from the campaign's sealed-run producer. It enters through `candler run`, declares one server, and reads the run back through `inspect`. See it fail on the base, where `inspect` shows `servers: []`. The guard takes the place of `guard_catalog_snapshot.rs`, which agreed with the code on the wrong value.
**Deliverables**: the guard in `crates/candler/tests/it/`
**Consistency Checks**: the guard on the base (expected: FAIL)
**Commit**: `test(api): read a run's installed MCP config back through inspect`

## Step 2: Remove the second producer and its consumer
**Goal**: The run record states only what Candler installed, and states it once.
**Implementation Logic**:
- Delete the constant program, its capture, the resolve-time catalog step and the fake docker's constant. Delete the level-2 surface that showed their output. Keep none of them "for compatibility" without a reader.
- Drop the snapshot from `comparable` under a new digest version, add its vectors, and keep reading old manifests.
- `inspect` and the op API show the resolved `mcp.*` values from `comparable`.
- Reword the README phrase and the `AGENTS.md` paragraph together, as Eliott ruled, along with every current page under `docs/` that quotes the phrase and the live candler-v1 M6c Goal. Historical reports and done leaves keep their words.
- Reword the `/1` schema's description to say that Candler neither writes nor reads it and that the HTN consumer produces it, and comment the old oracle vectors with why they stay.
- Before dropping `InspectRun.catalog`, check htn-creator-eval@0.5.3 for `inspect` callers that read `catalog`, and record the result. If one does, stop and tell the lead.
- Remove the refusal codes that only the deleted code emitted. Regenerate the schemas and contract pages.
**Deliverables**: the files named in References; `README.md`; `AGENTS.md`; `__roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/m5b/stats/surface/integrity/README.md`; `docs/contract/codes.toml`; the oracle vectors; the regenerated schemas and pages
**Consistency Checks**: the guard on the tip (expected: PASS)
**Commit**: `refactor(runner): remove the constant catalog producer and show the installed MCP config`

## Step 3: Bite and verify
**Goal**: The guard catches a record that no longer shows the server.
**Implementation Logic**:
On a scratch branch, drop the `mcp.*` values from what `inspect` shows, and see the guard fail. A fresh Sonnet verifier gets this leaf, the report and the diff. Its brief is to find any code that still states a run's tools or servers other than the resolved config, the census's tool list and the trace, and any old run that no longer reads.
**Deliverables**: `__reports__/diet/leaves/snapshot_out_v0.md`
**Consistency Checks**: the scratch edit makes the guard fail (expected: FAIL)
**Commit**: `reports(api): close the snapshot removal`
