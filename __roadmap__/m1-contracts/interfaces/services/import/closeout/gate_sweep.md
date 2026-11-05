# Gate sweep

**Goal**: The milestone's five gates (R02) are run and recorded on the joined `feat/m1-contracts`, the services API is re-reviewed against its consumers, and the crates, fixtures and schema roots are documented so M2a and M2b start cold.
**Pre-conditions**:
- [ ] [cmd] Every other leaf is landed: `for l in wire_foundations wire_trace golden_oracle wire_execution wire_evaluation harness_traits resolve_config supervisor run_store claude_code_reader legacy_mapping legacy_import seams; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`
- [ ] [cmd] `test -f "$GOLDEN/PROVENANCE.json"`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
**Success Gates**:
- ⬜ [cmd] **G1** (golden import): `CANDLER_GOLDEN_CORPUS="$GOLDEN" CANDLER_GOLDEN_REPORT="$TMPDIR/m1-golden-report.json" cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_golden)'` passes, its log kept in the leaf report
- ⬜ [cmd] **G2** (traces explained): `jq -e '(.trace_failures | all(.explanation | length > 0)) and ([.invalid_runs[] | select(.reason != "input_leak")] | length == 0)' "$TMPDIR/m1-golden-report.json"`, and the leaf report lists the trace-failure count (expected 0) run by run
- ⬜ [cmd] **G3** (integrity): `cargo nextest run -p candler-core -E 'binary(guard_legacy_mapping) | binary(guard_import_mini)'` passes (the 56 `incomplete` runs valid in `guard_golden`; the synthetic contaminated run `invalid{input_leak}` with no outcome file)
- ⬜ [cmd] **G4** (supervision): `cargo nextest run -p candler-core -E 'binary(guard_supervise)'` passes on macOS; [nl] the Linux run is decided by the PR's CI
- ⬜ [cmd] **G5** (drift): DRIFT passes, and `diff <(ls crates/candler-schemas/schema | sort) __roadmap__/m1-contracts/interfaces/services/import/closeout/expected_roots.txt` prints nothing
- ⬜ [cmd] `test -f crates/candler-core/README.md && test -f crates/candler-harness/README.md && test -f crates/candler-harness-claude-code/README.md && test -f crates/candler-schemas/schema/README.md && test -f crates/candler-core/tests/fixtures/README.md`
- ⬜ [cmd] STD and SUBJ pass
- ⬜ [nl] Three bite mutations, one each from the supervisor, claude_code_reader and legacy_import leaf reports, re-applied on the joined branch, each make their test fail; the log is in the leaf report
- ⬜ [nl] The api_review verifier, re-engaged on `git diff <L1 merge>..feat/m1-contracts -- crates/*/src`, reports no open finding against M2a, M2b and the OpenCode facts in R01
**References**: [R01 §The `schemas@` line holds, §Golden corpus](../../../../../../__design__/architecture.md) — the expected roots and gates; [R02](../../../../../candler-v1/contracts/m1_contracts.md) — the five milestone gates; [R07](../../../../../candler-v1/contracts/parity/README.md) — the consumers re-reviewed against

## Step 1: Record the milestone gates and expected roots
**Goal**: G1–G5 are run on the joined branch and the expected schema roots are written by hand.
**Implementation Logic**:
1. `expected_roots.txt`: one file name per line, sorted, written from R01 §The `schemas@` line holds and the leaves' publish commits, never from `ls`; a root the list lacks or a listed root absent from `schema/` is a finding, not a list edit.
2. Run G1–G4 and the three bite re-checks; keep the logs for the leaf report; dispatch the api_review verifier on the services diff and fold its findings back to the leaves' implementers.
**Deliverables**: `__roadmap__/m1-contracts/interfaces/services/import/closeout/expected_roots.txt`
**Consistency Checks**: `diff <(ls crates/candler-schemas/schema | sort) __roadmap__/m1-contracts/interfaces/services/import/closeout/expected_roots.txt` (expected: PASS)
**Commit**: `rdm(importer): record the m1 schema roots and gate sweep`

## Step 2: Describe the crates, fixtures and schema roots
**Goal**: A cold reader finds what each crate holds, where each fixture comes from, and which schema root is which.
**Implementation Logic**:
1. Crate READMEs for `candler-core`, `candler-harness` and `candler-harness-claude-code`: what the crate holds, its public modules, and the guard files that pin them.
2. `crates/candler-schemas/schema/README.md`: each root, its wire tag and the R01 section it encodes; `crates/candler-core/tests/fixtures/README.md`: the mini-corpus, its synthetic runs and the provenance file.
3. Any design text the sweep found out of date, in its own `docs(design)` commit.
**Deliverables**: `crates/{candler-core,candler-harness,candler-harness-claude-code}/README.md`; `crates/candler-schemas/schema/README.md`; `crates/candler-core/tests/fixtures/README.md`
**Consistency Checks**: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` (expected: PASS)
**Commit**: `docs(readme): describe the m1 crates, fixtures and schema roots`
