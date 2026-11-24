# close_out

**Goal**: Close the diet: restore the yardstick, record the test policy and store layout in R01, and measure what the diet cost.
**Estimate**: about 30 Sonnet calls ($1.1 with the verifier), cap 45. At 45 calls, stop, commit what is green, and list what is left in the leaf report.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/diet HEAD` succeeds, with every L0 piece merged into `feat/diet`
- [ ] `[nl]` OWNED = `^README\.md$|^__design__/architecture\.md$|^__reports__/diet/|^__roadmap__/diet/`
**Success Gates**:
- ⬜ `README.md` opens with the purpose paragraph as written in `254ff4a0` (K3), above the current text
- ⬜ R01 §Testing names the six consumer interfaces and the critical set; the store-layout row matches the code
- ⬜ `__reports__/diet/02-knowledge_transfer_diet_v0.md` gives nextest and line counts before and after, measured spend per piece from transcripts, and Next
- ⬜ `cargo nextest run --workspace` passes on the diet tip
- ⬜ `cargo fmt --check` passes
- ⬜ `cargo clippy --workspace --all-targets -- -D warnings` passes
- ⬜ `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes
- ⬜ DIGEST unchanged: `cargo nextest run --workspace -E 'test(/cell_digest|context_digest|build_context/)'` passes with no vector edited
- ⬜ GOLDEN unchanged: `cargo nextest run --workspace -E 'test(/golden/)'` passes with no vector edited
- ⬜ Ownership: `git diff --name-only feat/diet...HEAD | grep -vE '^README\.md$|^__design__/architecture\.md$|^__reports__/diet/|^__roadmap__/diet/'` prints nothing
- ⬜ A fresh Sonnet verifier, given only this leaf and the diff, reports no promise left without a test and no gate unmet
**References**: [Diet plan](../../../../../__reports__/diet/00-architecture_diet_plan_v0.md) — §The yardstick, §How the suite becomes the critical set, §Cost and §Next; [AGENTS.md](../../../../../AGENTS.md) — checks and conventions

## Step 1: Restore the yardstick and update R01
**Goal**: The README states what Candler is; R01 states how it is tested.
**Implementation Logic**:
Put the purpose paragraph back at the top of `README.md`, word for word from `git show 254ff4a0:README.md`, and keep the store-window text below it. In `__design__/architecture.md` §Testing, write the guard rule (a test under `tests/` is a guard at one of six consumer interfaces) and list the critical set by name; check the store-layout row against what `s1_seal_blobs` merged.
**Deliverables**: `README.md`, `__design__/architecture.md`.
**Consistency Checks**: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` and the docs guards (expected: PASS)
**Commit**: `docs(readme): restore the purpose paragraph`

## Step 2: Knowledge transfer and spend
**Goal**: The next agent inherits numbers, not impressions.
**Implementation Logic**:
Count nextest tests and product and test lines on the diet tip against `1002cb82`. Measure spend per piece from the transcripts (`spend.py`, cache reads included), and compare with the plan's estimates and caps. Write the knowledge transfer (wins, pain points, Next with evidence) and update `__reports__/diet/README.md`. Mark the roadmap nodes done with `dirtree-rdm update`.
**Deliverables**: `__reports__/diet/02-knowledge_transfer_diet_v0.md`, `__reports__/diet/README.md`, roadmap statuses.
**Consistency Checks**: `dirtree-rdm validate __roadmap__/diet/` (expected: PASS)
**Commit**: `reports(api): close out the architecture diet`
