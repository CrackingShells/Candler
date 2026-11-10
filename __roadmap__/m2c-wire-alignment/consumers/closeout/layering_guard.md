# Layering guard

**Goal**: The I1 guard matches identifier segments, not substrings or whole words. It scans every crate's `src`, the published schemas, `assets/` and the evaluator helper. Its denied segments include `planner`, `tier` and `problems`, with a short exact allowlist. Its rename test renames `tier`. Planted domain symbols fail it, and `run_plan`, `ContainerPlan` and `WeightNotAllowed` pass.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone after L1: `git merge-base --is-ancestor feat/m2c-wire-alignment HEAD && git log --first-parent --format=%s feat/m2c-wire-alignment | grep -qE '^merge\(.*\): land design_record$'`
- [ ] [cmd] The landing greps already hold, so the guard starts on a clean tree: `! git grep -n -i -E 'gtpyhop|planner' -- ':(glob)crates/*/src/**' assets && ! git grep -n -w -i tier -- ':(glob)crates/*/src/**'`
- [ ] [nl] OWNED: `crates/candler-core/tests/guard_layering_i1.rs`, `crates/candler-core/tests/support/mod.rs` (the scanner). A domain name the new guard finds in `src/` is reported, not fixed here, unless it is a comment or a doc line, which this leaf may reword
**Success Gates**:
- ⬜ [cmd] L1 (the real tree): `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_layering_i1) & test(/workspace_holds_no_domain_segment/)'`.
  - The scanned folders are found by listing `crates/` (so a new crate is scanned without editing the guard), plus `crates/candler-schemas/schema`, `assets/` and `python/candler-evaluator/src`.
  - Each folder must read at least one file.
  - Oracle: the deny literals of the file
- ⬜ [cmd] L2 (the plants): `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_layering_i1) & test(/plant/)'`, a parameterized table over a temp tree laid out like the workspace.
  - Found: `struct PlannerAxis;` in `crates/candler/src/x.rs`, `let planner_package` in `crates/candler-core/src/y.rs`, `gtpyhop-core>=2` in `assets/docker/env/z.in`, `"tier-a"` in `crates/candler-schemas/src/w.rs`, `TIER_D` and `PlanTrace` anywhere.
  - Not found: `run_plan`, `ContainerPlan`, `WeightNotAllowed`, `frontier`, `problem_count`.
  - Oracle: the hand-written table
- ⬜ [cmd] L3 (the rename): `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_layering_i1) & test(/renamed_domain/)'`.
  - The renamed mini-corpus also renames `tier` to `band`, and the rules name `entry_params = ["scale", "text", "band"]`.
  - It gives the oracle's run and status counts, and the store holds no denied segment.
  - Oracle: `oracle::expected_rows` over the original mini-corpus
- ⬜ [cmd] The guard's other tests pass: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_layering_i1)'`
- ⬜ [cmd] STD (`-p candler-core`), LINUX and SUBJ pass
- ⬜ [nl] The leaf report records the bites. Each turns L1 or L2 red:
  - `assets/` taken out of the scan;
  - the splitter without case boundaries;
  - matching by whole words;
  - matching by substring, which turns the `WeightNotAllowed` row red.
- ⬜ [nl] The `layering_guard` verifier closes with no open finding. It applies mutations to the real tree and sees the guard fail each time, then restores the tree with `git checkout`:
  - `PlannerAxis` appended to `crates/candler/src/commands/supervise.rs`;
  - `planner_package` added to a core file;
  - `gtpyhop` added to an `assets/` file;
  - `tier` added to a schemas source.
**References**: [R05 G and "Needs execution"](../../../../__reports__/candler-v1-review/00-findings_v0.md) — the planted `PlannerAxis` bite and the `("tier","band")` rename; [R04 Risk](../../../../__reports__/m2c-wire-alignment/gap_analysis_planner_v0.md) — segments, not whole words; [R10 guards L1–L3](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md); the current `guard_layering_i1.rs`

## Step 1: Segment matching over the whole layering
**Goal**: The guard sees what the plants and the mutations put in, and nothing generic.
**Implementation Logic**:
1. Write the L2 table first, with the new splitter stubbed, and see the planted rows red.
2. Add `support::scan_segments(root, denied, literals)`.
   - It splits every identifier-like token on non-alphanumerics, `_`, lower-to-upper case changes and acronym boundaries (`HTTPServer` → `http`, `server`), compares lowercase segments with the denied set, and reports `(file, line, segment)`.
   - It gunzips as `scan` does.
3. The denied segments are: `htn`, `gtpyhop`, `pyhop`, `rikyu`, `planner`, `planners`, `tier`, `tiers`, `problems`, `libcst`, `arango`, `dotenv`, `plantrace`.
4. The literal list keeps the multi-token names: `mcp-python-ingestion` and `D-E`.
5. The allowlist is a `const` of exact `(file, line text)` pairs, empty unless a generic use is found. If one is needed, the leaf report justifies each entry, and a self-test proves that an entry excuses that line only.
6. Rewrite L1 over the folders listed from `crates/`.
7. Extend `RENAMES` with `("tier", "band")`, applied after `tier_d`/`TIER_D`, and add `"band"` to the rules' `entry_params` (L3).
8. Delete the old substring `DENY_LIST` scans that the segment scan replaces. Keep the rename test's `assert_clean` on the store, now with segments.
**Deliverables**: `crates/candler-core/tests/support/mod.rs`: `scan_segments`; `crates/candler-core/tests/guard_layering_i1.rs`: L1, L2, L3
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_layering_i1)'` (expected: PASS)
**Commit**: `test(repo): match the i1 guard on identifier segments across the tree`
