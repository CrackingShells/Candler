# Import treatment

**Goal**: An imported experiment declares the same factors as its native rerun: the importer declares `request.treatment` when the imported cells' typed treatments differ, and drops a value path whose levels map one to one onto the treatment's (import gate, A8).
**Estimate**: about 60 Sonnet calls, $3.0 with its verifier; verifier `spec-verifier`, Sonnet, "at 30 calls, stop and report"
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (campaign README, Gotchas); L2 is merged
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/import/.*$|^crates/candler-core/tests/it/guard_import_mini\.rs$|^crates/candler-core/tests/it/support/legacy_import\.rs$|^crates/candler/tests/it/guard_analyze_golden\.rs$|^__reports__/m6a-paired-stats/leaves/import_treatment_v0\.md$`. Nothing in `crates/candler-schemas/` or `codes.toml`
**Success Gates**:
- ⬜ [cmd] Targeted test: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_import_mini::/)'`
- ⬜ [cmd] Golden row (GOLDEN's corpus): `CANDLER_GOLDEN_CORPUS=/Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'test(/^guard_analyze_golden::declared_/)'`
- ⬜ [cmd] DIGEST passes unchanged
- ⬜ [nl] The probe's answer, each Bite and the verifier's verdict are in the report
**References**: `m6a-l3-addendum.md` and `m6a-l2-handover.md` §For golden_blocked (notes folder of session local_7f577b41); A7

## Step 1: Probe
**Goal**: Know what the change can move before changing it.
**Implementation Logic**:
1. Read the importer's factor declaration (`import/assemble.rs`, `import/manifest.rs`) and record: which value paths it declares today and why; whether any fixture pins imported manifest bytes or factor lists (`guard_import_mini`, `guard_legacy_mapping`, DIGEST's oracle files); whether the factor list enters the cell digest or the experiment digest.
2. A change that would move a cell digest, an experiment digest or a pinned fixture is a stop-and-report to the coordinator before any code.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/import_treatment_v0.md`: section "Probe"
**Consistency Checks**: `grep -q '^## Probe' __reports__/m6a-paired-stats/leaves/import_treatment_v0.md` (expected: PASS)
**Commit**: `docs(importer): record what the treatment factor can move`

## Step 2: Declare the treatment
**Goal**: The imported factors equal the native ones.
**Implementation Logic**:
1. When the imported cells' typed treatment references `{name, tag}` differ, declare `request.treatment`, and do not declare a value path whose levels map one to one onto the treatment's levels (on the golden runs `plan.skill_version`, `provenance.skill_version`, `resolved_skill_version`). The values stay in the cell untouched. A one-version import declares what it declares today.
2. `it_two_versions_declare_treatment` (in `guard_import_mini.rs`, mini legacy import from `support/legacy_import.rs` with 2 versions × 2 models): declares exactly the treatment and the model. `it_one_version_unchanged`: a one-version import's factors are today's.
3. `declared_golden_factors` (ignored, in `guard_analyze_golden.rs`, with `CANDLER_GOLDEN_CORPUS`): the imported golden runs declare exactly `plan.model` and `request.treatment`, and `analyze` gives 6 version comparisons with 3 blocks and 3 model comparisons with 4 blocks. Shape only, no p-value pinned.
**Deliverables**: `crates/candler-core/src/import/` (the declaration rule); `crates/candler-core/tests/it/guard_import_mini.rs`: `it_two_versions_declare_treatment`, `it_one_version_unchanged`; `crates/candler-core/tests/it/support/legacy_import.rs` (a two-version mini fixture); `crates/candler/tests/it/guard_analyze_golden.rs`: `declared_golden_factors`
**Consistency Checks**: the two targeted gates above (expected: PASS)
**Commit**: `fix(importer): declare the treatment as an imported factor`

## Step 3: Bites, verifier and the report
**Goal**: The rows bite and an independent read agrees.
**Implementation Logic**:
1. Bites, each injected, seen red, restored from a copy: the importer keeps declaring the mapped value paths; it declares the treatment on a one-version import; it drops a value path that does not map one to one. Record each failing message.
2. Dispatch one fresh `spec-verifier` (Sonnet) with this leaf, the branch, the worktree, colgrep-only reading and "at 30 calls, stop and report"; fix its findings (`fix(importer): …`), one re-check by SendMessage, record the verdict.
**Deliverables**: `__reports__/m6a-paired-stats/leaves/import_treatment_v0.md`: sections "Bites", "Verifier"
**Consistency Checks**: `grep -q '^## Verifier' __reports__/m6a-paired-stats/leaves/import_treatment_v0.md` (expected: PASS)
**Commit**: `docs(importer): record the bites and the verifier of the import`
