# Harness guard

**Goal**: The I1 layering guard denies the identifier segments `claude`, `anthropic` and `opencode` in `candler-core/src` and `candler-schemas/src` only (amendment A3), so the core cannot name a harness again, while the driver crates and the binary stay free to.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The core already names no harness: `! git grep -n -i -E 'claude|anthropic|opencode' -- crates/candler-core/src crates/candler-schemas/src`
- [ ] [nl] OWNED: `crates/candler-core/tests/guard_layering_i1.rs`, `crates/candler-core/tests/support/mod.rs` (the segment scanner)
**Success Gates**:
- ⬜ [cmd] L1: the real tree holds no harness segment in the two scanned roots; plants are found (`claude_version` in a core module, `AnthropicKey` in a schemas type, `opencode-db` in a core string); the same plants in `candler-harness-claude-code/src`, `candler-harness-opencode/src` and `crates/candler/src` are not reported: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_layering_i1)'`
- ⬜ [cmd] The domain deny-list still holds: no existing test of `guard_layering_i1.rs` is removed: `! git diff a26fe53 -- crates/candler-core/tests/guard_layering_i1.rs | grep -E '^-\s*fn '`
- ⬜ [cmd] STD (`-p candler-core`), DIGEST and SUBJ pass
- ⬜ [nl] The leaf report records the bites the verifier applied to the real tree (a `claude_version` constant in the core, an `ANTHROPIC_` string in the schemas), each turning the guard red
- ⬜ [nl] The `harness_guard` verifier closes with no open finding: it applies the mutations and attacks the scope (widened to every crate: false red; narrowed to none: no bite)
**References**: [R03 Gap analysis A3](../../../../../../__reports__/candler-v1-review/01-gap_analysis_harness_env_v0.md) — the deny-list and its scope; [R01 I1](../../../../../../__design__/architecture.md)

## Step 1: Scope the harness deny-list
**Goal**: The scanner applies a second deny-list to two roots only.
**Implementation Logic**:
Give `scanned_targets` a per-root rule set: the existing domain rules everywhere they apply today, plus `HARNESS_SEGMENTS` (`claude`, `anthropic`, `opencode`) for `crates/candler-core/src` and `crates/candler-schemas/src`. Add the plants test and the `RENAMES` entries the rename test needs. Write the plants first and see them unreported (red) before the rule exists.
**Deliverables**: `crates/candler-core/tests/guard_layering_i1.rs`: `HARNESS_SEGMENTS`, `harness_plants_are_found_only_in_the_core_and_the_schemas`; `crates/candler-core/tests/support/mod.rs`: per-root rules
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_layering_i1)'` (expected: PASS)
**Commit**: `test(repo): deny harness names in the core and the schemas`
