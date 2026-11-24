# Snapshot out

**Goal**: The constant catalog producer is gone and the census is the one record of the tools a run had; the cell digest compares the resolved `mcp.servers`, and the README and `AGENTS.md` say "per-run tool census" (P12).
**Estimate**: Sonnet, $8, cap $16. At the cap, stop, commit what is green, and list what is left. Builds use the target dir the campaign README names, removed at the leaf's merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor origin/feat/integrity HEAD` succeeds
- [ ] `[nl]` `../census.md` is `done`
- [ ] `[nl]` The coordinator ruled on the comparable change: `comparable.snapshots` leaves the cell digest under a new digest version, old manifests stay readable, and new oracle vectors are added beside the old ones
**Success Gates**:
- ⬜ A census guard on a real `candler run` path with one declared server reads the record through `inspect` and finds the server and its tools; on the base the stored catalog of the same run says `servers: []`
- ⬜ `CATALOG_PROGRAM`, `CommandCatalogProvider::capture`'s constant command, `CatalogStep`'s capture and the fake docker's catalog constant are gone; nothing in the tree prints a catalog it did not observe
- ⬜ `comparable` carries no catalog snapshot; the new digest version's oracle vectors are added and the old ones still verify old manifests
- ⬜ A sealed run from before the change still reads through `inspect`, with its stored catalog shown as what it is
- ⬜ The `tool-catalog-snapshot/1` schema and its plug-in consumers (catalog providers, Tier D, upstream fixtures) are unchanged
- ⬜ The README purpose paragraph says "per-run tool census" in place of "per-run tool-catalog snapshots", and `AGENTS.md`'s "What sane work means in Candler" names the census among the stored states, in the same commit
- ⬜ Refusal codes that only the removed producer emitted are removed from `codes.toml` and the refusal tables, and the contract pages are regenerated
- ⬜ The report's "What it leaves" section judges the change at the three levels
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with the target dir the campaign README names, and `container-e2e` is green on the pull request
**References**: [Level-3 plan §Design](../../../../../../../../../../__reports__/diet/07-plan_integrity_level3_v0.md) — one record, compatibility; [Dogfooding findings, finding (1)](../../../../../../../../../../__reports__/diet/05-findings_dogfooding_pass_v0.md) — the cause chain; `crates/candler/src/commands/preflight.rs` — `CATALOG_PROGRAM`; `crates/candler-core/src/runner/evidence/catalog.rs` — `CommandCatalogProvider::capture`; `crates/candler-core/src/resolve/steps/catalog.rs` — `CatalogStep::run`; `crates/candler/tests/it/support/cli_run.rs` — the fake docker; `crates/candler/tests/it/guard_catalog_snapshot.rs`; `crates/candler-schemas/src/execution.rs` — `Comparable`

## Step 1: Guard on the base
**Goal**: The census guard fails on the base where the stored catalog says no server.
**Implementation Logic**:
Write the census guard on a run from the campaign's sealed-run producer, entering through `candler run` and reading back through `inspect`, with an experiment that declares one server. See it fail on the base for the stored catalog's `servers: []`. The guard takes the place of `guard_catalog_snapshot.rs`, which agreed with the code on the wrong value.
**Deliverables**: the census guard in `crates/candler/tests/it/`
**Consistency Checks**: the census guard on the base (expected: FAIL)
**Commit**: `test(runner): read a run's census back through inspect`

## Step 2: Remove the second producer
**Goal**: Only the census says what tools a run had.
**Implementation Logic**:
Delete the constant program, its capture, the resolve-time catalog step and the fake docker's constant; do not keep any of them for compatibility without a reader. Drop the snapshot from `comparable` under a new digest version and add its vectors; keep the reading of old manifests. `inspect` and the op API show the census; an old run's stored catalog stays readable. Reword the README phrase and the `AGENTS.md` paragraph together, as Eliott ruled, and every current page under `docs/` that quotes the phrase; historical reports and other campaigns' roadmaps keep their words, and the lead tells the coordinator which of its roadmap's quotes to sweep. Remove refusal codes that only the deleted code emitted. Regenerate schemas and contract pages.
**Deliverables**: the files named in References; `README.md`; `AGENTS.md`; `codes.toml`; the oracle vectors; the regenerated schemas and pages
**Consistency Checks**: the census guard on the tip (expected: PASS)
**Commit**: `refactor(runner): make the census the one record of a run's tools`

## Step 3: Bite and verify
**Goal**: The guard catches a record that no longer names the server.
**Implementation Logic**:
On a scratch branch, drop the servers from the census and see the guard fail. A fresh Sonnet verifier gets this leaf, the report and the diff, with the brief: find any code that still produces a statement of a run's tools other than the census, and any old run that no longer reads.
**Deliverables**: `__reports__/diet/leaves/snapshot_out_v0.md`
**Consistency Checks**: the scratch edit makes the census guard fail (expected: FAIL)
**Commit**: `reports(api): close the snapshot removal`
