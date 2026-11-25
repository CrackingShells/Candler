# Meet run context

**Goal**: When M6.5 lands after M6d (run context, `feat/run-context`), the two trace changes meet in one generated schema and contract page, with one minor version bump, and the evaluator-facing docs say that `needs.trace` now hands over the run's starting context. If M6.5 lands first, M6d owes this, and the leaf records that. (P0, agreed between the M6.5 and M6d leads on 2026-11-25)
**Estimate**: Sonnet, $3, cap $6. One build slot of `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, removed when it merges.
**Pre-conditions**:
- [ ] `[cmd]` `git merge-base --is-ancestor feat/run-context origin/main` is checked after `git fetch origin`. If it succeeds, M6d landed first and every gate below applies. If it fails, M6.5 is landing first: record that, mark the gates "owed by M6d", and stop.
- [ ] `[nl]` `adversarial_review.md` is `done`, or runs beside this leaf on the same tip
**Success Gates**:
- ✅ Recorded 2026-11-25: `git fetch origin`, then `feat/run-context` is not on `origin/main` (it has no remote branch, and `HEAD..origin/main` is empty), so M6.5 lands first. Every gate below is owed by M6d.
- ✅ (met by M6d, 2026-11-26: `feat/run-context` took in `origin/main` at `769ebd4d` by a merge; `KnownKind` holds `ContextStart` and `tool_result.exit`; the schemas and `trace-file.md` were blessed, and `drift` and `contract_pages` pass unblessed) `feat/m6.5-agent-config` takes in `origin/main` by a merge. History is not rewritten, so the milestone branch is not rebased; that matches the leads' agreement in substance. Generated files are regenerated, not merged by hand: `KnownKind` in `crates/candler-schemas/src/trace.rs` holds both `ContextStart` and M6d's typed `tool_result.exit`. The normalised-trace schema and the trace contract page come from `CANDLER_BLESS_SCHEMAS=1 … drift` and `CANDLER_BLESS_CONTRACT=1 … contract_pages`, and both pass unblessed afterwards.
- ✅ (met by M6d: no crate `version` or CHANGELOG differs from `origin/main`; the helper's `pyproject.toml` is at 0.2.0 by hand, as CONTRIBUTING's helper line requires) No version is bumped by hand. `git diff origin/main..HEAD` shows no change to any crate's `version` or to a CHANGELOG. The `feat(schemas)` commits alone set the release's single minor bump.
- ✅ (met by M6d: the `Needs.trace` doc, hence the evaluator schemas, and the helper README; the protocol page's Notes are at their 120-word cap) The merge commit, or a commit made with it, adds the sentence next to `needs.trace` in the evaluator-facing docs. It says that declaring `needs.trace` hands the evaluator kind the run's system text and first message verbatim (`context_start`), which may be the treatment under test.
- ✅ (met by M6d: 772 passed, 38 skipped; clippy `-D warnings`, fmt and rustdoc `-D warnings` pass; no `candler.test` container left) `cargo nextest run --workspace` passes on the merged tip, and `docker ps -a` shows none of the leaf's containers.
**References**: `crates/candler-schemas/src/trace.rs` (`KnownKind`); `docs/contract/trace-file.md`; M6d's `needs.trace` doc on `origin/main`; the leaf [trace_context.md](../../../trace_context.md), which added `context_start`

## Step 1: Merge and regenerate
**Goal**: One schema, one page and one doc sentence that carry both milestones.
**Implementation Logic**:
1. Merge `origin/main` into `feat/m6.5-agent-config`. Resolve conflicts in the source (`trace.rs` and the hand-written parts of the pages) by keeping both kinds, then regenerate every generated schema and page instead of resolving them by hand.
2. Add the `needs.trace` sentence where M6d documents `needs.trace`, in the same words a consumer reads elsewhere about `context_start`.
3. Run the workspace tests on the merged tip.
**Deliverables**: the merge commit; regenerated `crates/candler-schemas` outputs and `docs/contract/` pages; the `needs.trace` sentence
**Consistency Checks**: `cargo nextest run -p candler-schemas drift` and `cargo nextest run -p candler contract_pages` (expected: PASS)
**Commit**: `merge(api): land M6.5 beside the run context`
