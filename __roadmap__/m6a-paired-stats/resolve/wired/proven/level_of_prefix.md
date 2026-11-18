# Level-of prefix

**Goal**: `analysis/blocking.rs::level_of` carries no unreachable branch: its object-under-prefix branch is deleted if every caller runs after `check_factors`, else guarded through the path that reaches it (A13); the pair orientation is stated in R01.
**Estimate**: about 20 Sonnet calls, $0.8; no verifier (STD is the evidence)
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (campaign README, Gotchas); golden_design is merged
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/analysis/blocking\.rs$|^__design__/architecture\.md$|^__reports__/m6a-paired-stats/leaves/level_of_prefix_v0\.md$`
**Success Gates**:
- ⬜ [cmd] STD passes (workspace form, campaign README)
- ⬜ [nl] The report lists every caller of `level_of`, `blocks_of` and `unit_blocks_of` and where each runs `check_factors` first
**References**: `__reports__/m6a-paired-stats/leaves/golden_design_v0.md` finding 1; A13

## Step 1: Callers and the branch
**Goal**: The branch is either gone or guarded.
**Implementation Logic**:
1. With colgrep, list every caller of `level_of`, `blocks_of` and `unit_blocks_of` and confirm each runs only after `report.rs::check_factors`, which refuses a declared factor that is a strict prefix of a differing path.
2. If all do: delete the object-under-prefix branch of `level_of` and fix its doc comment. If any path reaches `level_of` without `check_factors`: keep the branch, guard it through that path, and report.
**Deliverables**: `crates/candler-core/src/analysis/blocking.rs`; `__reports__/m6a-paired-stats/leaves/level_of_prefix_v0.md`: section "Callers"
**Consistency Checks**: STD (expected: PASS)
**Commit**: `fix(analysis): drop the unreachable prefix branch of level_of`

## Step 2: Pair orientation in R01
**Goal**: Readers know which level is A.
**Implementation Logic**:
1. One sentence in `__design__/architecture.md` §Analysis statistics: A and B are the two levels in their recorded key order, not the declared order, and each comparison names `level_a` and `level_b`.
**Deliverables**: `__design__/architecture.md`
**Consistency Checks**: the G3 loop (expected: PASS)
**Commit**: `docs(design): state which level of a comparison is A`
