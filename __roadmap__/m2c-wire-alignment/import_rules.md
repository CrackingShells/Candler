# Import rules

**Goal**: The parameters an imported suite entry carries (`scale`, `text`, `tier` for the HTN rules) come from the import rules, not from a core constant, with the same values for the HTN rules and so the same digests.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone: `git merge-base --is-ancestor feat/m2c-wire-alignment HEAD`
- [ ] [cmd] The golden corpus is in place: `test -d /Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`
- [ ] [nl] OWNED: `crates/candler-core/src/import/{legacy,rules,grading}.rs`, `crates/candler/src/commands/import.rs`, `crates/candler/tests/fixtures/cli/mini-corpus-rules.toml` and any other rules TOML under `crates/*/tests/fixtures`, `crates/candler-core/tests/support/legacy_rules.rs`, `crates/candler-core/tests/guard_import_mini.rs`, and the `rules_without_markers` hunk of `crates/candler-core/tests/guard_layering_i1.rs`
**Success Gates**:
- ⬜ [cmd] The constant is gone, and with it the word `tier` from the core's sources: `! git grep -n ENTRY_PARAMS -- ':(glob)crates/*/src/**' && ! git grep -n -w -i tier -- ':(glob)crates/candler-core/src/**' ':(glob)crates/candler/src/**'`
- ⬜ [cmd] R1: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_import_mini) & test(/entry_params/)'`. Rules that name `["scale", "text"]` import a mini-corpus entry and synthesise a suite entry without `tier`. Rules that name a key an entry lacks are refused, naming the key and the entry. Oracle: the mini-corpus oracle (`oracle::expected_rows`) and hand-written expectations
- ⬜ [cmd] The HTN rules give the same digests: G5c and GOLDEN pass, including `guard_import_mini`, `guard_legacy_mapping` and `guard_oracle_mini`
- ⬜ [cmd] The binary reads the new key: `cargo nextest run -p candler --no-tests=fail -E 'binary(guard_cli)'` passes. `guard_golden_parity` also reads a rules file, but it is ignored and needs the HTN plug-in (`CANDLER_HTN_PLUGIN`): run it if the plug-in is at hand, and say in the leaf report whether it ran
- ⬜ [cmd] STD (`-p candler-core -p candler`), LINUX and SUBJ pass
- ⬜ [nl] The leaf report records R1's bite (the constant restored and the rules field ignored) and the ownership check of the digest guards (dropping `tier` from the HTN rules file turns `guard_import_mini` red)
**References**: [R01 §Golden corpus, "Importing the corpus"](../../__design__/architecture.md) — the rules supply every domain name; [R05 D5](../../__reports__/candler-v1-review/00-findings_v0.md); [R10 guard R1](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md)

## Step 1: Entry params in the rules
**Goal**: `ImportRules` carries the entry params, the rules file names them, and the importer reads them from there.
**Implementation Logic**:
1. Write R1 first and see it red.
2. `import/rules.rs`: `ImportRules` gains `entry_params: Vec<Name>` (or the type the other rule fields use), required by the builder, because I12 forbids a silent default.
3. `import/grading.rs::suite_entry`: iterate the rules' list instead of `ENTRY_PARAMS`, and delete the constant from `legacy.rs`.
4. Reword `grading.rs:363`'s doc so that it names no domain key.
5. `crates/candler/src/commands/import.rs`: `RulesFile` gains `entry_params` (`deny_unknown_fields` already holds).
6. Every rules TOML fixture and `tests/support/legacy_rules.rs` gain `entry_params = ["scale", "text", "tier"]`. `guard_layering_i1.rs::rules_without_markers` gains the same line in its builder. Touch nothing else there: `layering_guard.md` rewrites the file later.
7. Run GOLDEN.
**Deliverables**: `crates/candler-core/src/import/rules.rs`: `ImportRules::entry_params`, the builder's `entry_params`; `crates/candler-core/src/import/grading.rs`: `suite_entry` reading the rules; `crates/candler-core/src/import/legacy.rs` without `ENTRY_PARAMS`; `crates/candler/src/commands/import.rs`: `RulesFile::entry_params`; the rules fixtures; `crates/candler-core/tests/guard_import_mini.rs`: the R1 guard
**Consistency Checks**: `! git grep -n ENTRY_PARAMS -- ':(glob)crates/*/src/**' && cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_import_mini) | binary(guard_legacy_mapping) | binary(guard_layering_i1)'` (expected: PASS)
**Commit**: `feat(importer)!: take the imported entry's params from the rules`
