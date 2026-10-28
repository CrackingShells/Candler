# README status and the milestone gate sweep

**Goal**: The README says what exists, and every gate of the milestone leaf passes on the integrated `feat/wire-schemas`.
**Pre-conditions**:
- [x] Every leaf of `../` is `done` and merged on `feat/wire-schemas`
**Success Gates**:
- ⬜ `cargo build --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` pass
- ⬜ `cargo test -p candler-schemas` passes
- ⬜ The drift test fails on a deliberate drift and passes after `CANDLER_BLESS_SCHEMAS=1` (scratch change reverted)
- ⬜ `cog check main..HEAD` passes, and the hook rejects `feat: no scope`
- ⬜ `actionlint .github/workflows/*.yml` passes
- ⬜ `dirtree-rdm validate __roadmap__/wire-schemas/` and `dirtree-rdm validate __roadmap__/candler-v1/` pass
- ⬜ R01 §Wire encodings matches what was built (coordinator's read, differences fixed in a `docs(design)` commit before this one)
**References**: [R01 Milestone leaf](../../../candler-v1/wire_schemas.md) — the gates; [R02 README](../../../../README.md) — §Status

## Step 1: Say what exists
**Goal**: README §Status describes the workspace and the first two wire schemas.
**Implementation Logic**:
Rewrite README §Status: the crates that exist, `candler-schemas` with the outcome record and the tool-catalog snapshot and their committed JSON Schema files, and the other crates still empty. Then run every gate above and record the results in the campaign README's Progress table through `dirtree-rdm`.
**Deliverables**: `README.md` (§Status)
**Consistency Checks**: `cargo test -p candler-schemas` (expected: PASS)
**Commit**: `docs(readme): say the workspace and the first wire schemas exist`
