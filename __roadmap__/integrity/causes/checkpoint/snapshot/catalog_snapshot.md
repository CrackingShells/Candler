# Catalog snapshot

**Goal**: A sealed run stores the canonical bytes of the tool catalog it ran with, so the README's "per-run tool-catalog snapshots" can be read back (P5, option (b), B2).
**Estimate**: Sonnet, $6, cap $12. At the cap, stop, commit what is green, and list what is left. Builds use the target dir the campaign README names, removed at the leaf's merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/integrity HEAD` succeeds
- [ ] `[nl]` The spend checkpoint in `../seam_review_one.md` ruled that P5 runs
**Success Gates**:
- ⬜ `inspect` (and the op API) of a sealed run shows the catalog it ran with; on the base, the snapshot digest resolves to no blob
- ⬜ `resolve` stays a pure read: it carries the canonical bytes in the plan it hands on and writes nothing to the store
- ⬜ The snapshot is stored by the same sealing step as the outputs, and the guard gets its run from P1's producer
- ⬜ No digest or golden vector is edited
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with the target dir the campaign README names, and `container-e2e` is green on the pull request
**References**: [Cycle plan v2 §B2, §Proof](../../../../../__reports__/diet/04-architecture_cycle_plan_v2.md) — the ruling and the observation; `crates/candler-core/src/resolve/steps/catalog.rs` — `CatalogStep::run`

## Step 1: Guard and store
**Goal**: The run writes the catalog bytes at the seal.
**Implementation Logic**:
Write the guard first, on a run from P1's producer: open the sealed run through `inspect` and read the catalog; see it fail on the base. Then keep the canonical catalog bytes (the bytes `Catalog::content_digest` digests) in the plan `resolve` hands on, and have the runner store them when it seals, beside the outputs, so the digest resolves to a blob.
**Deliverables**: `crates/candler-core/src/resolve/steps/catalog.rs`; the runner's sealing step; the guard
**Consistency Checks**: the snapshot guard passes on the tip (expected: PASS)
**Commit**: `feat(runner): store the tool-catalog snapshot when the run is sealed`

## Step 2: Bite and verify
**Goal**: The guard catches a missing snapshot.
**Implementation Logic**:
Skip the snapshot write on a scratch branch and see the guard fail. A fresh Sonnet verifier gets this leaf, the report and the diff, with the brief: find a run path that seals without its snapshot.
**Deliverables**: `__reports__/diet/leaves/catalog_snapshot_v0.md`
**Consistency Checks**: the skipped write makes the snapshot guard fail (expected: FAIL)
**Commit**: `reports(api): close the catalog snapshot`
