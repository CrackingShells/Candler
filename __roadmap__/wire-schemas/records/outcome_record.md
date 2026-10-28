# Outcome record

**Goal**: `OutcomeRecord` carries every field of R01 §Wire schema: outcome record under the rules of §Wire encodings. It is valid however it was made, its schema is generated and drift-checked, and three test layers check it.
**Pre-conditions**:
- [x] `../workspace.md` and `../fixtures.md` are `done` and merged on `feat/wire-schemas`
- [ ] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
**Success Gates**:
- ⬜ `cargo nextest run -p candler-schemas outcome` passes
- ⬜ Adding a field to `OutcomeRecord` makes `cargo nextest run -p candler-schemas drift` fail; `CANDLER_BLESS_SCHEMAS=1` then makes it pass (the scratch change is reverted)
- ⬜ A use of the legacy cause variant without `#[allow(deprecated)]` makes `cargo clippy -p candler-schemas --all-targets -- -D warnings` fail
- ⬜ `jq '.. | objects | select(.deprecated == true)' crates/candler-schemas/schema/outcome-record.schema.json` prints the legacy variant
- ⬜ `cargo clippy --workspace --all-targets -- -D warnings` passes
- ⬜ Adversarial verification and Grit review report no open finding
**References**: [R01 §Wire encodings, §Wire schema: outcome record, §Golden corpus](../../../__design__/architecture.md) — fields, rules, legacy mapping; `htn_creator_eval/_outcome.py` at R04's tag — today's status and role spellings

## Step 1: Define the outcome record
**Goal**: The record and its members exist, and no invalid record can be built or deserialised.
**Implementation Logic**:
1. Module `candler_schemas::outcome`. Fields, with formats from `primitives`:
   - `run: RunKey`, `expectation_id: ExpectationId`, `role: Role`;
   - `evaluator: NameVersion`, `instruments: Vec<Instrument>`;
   - `suite_entry_digest: Digest`, `context_digest: Digest`, `request_id: RequestId`;
   - `status: Status`, `value: Option<Value>`, `weight: Option<Weight>`;
   - `probability: Option<Probability>`, `evidence: Vec<Evidence>`, `cause: Option<Cause>`.
2. The member types:
   - `Status`: `pass`, `fail`, `blocked`, `not_measured`, `not_applicable`.
   - `Role`: `gate`, `quality`, `descriptive`.
   - `Value`: tagged on `kind`: `number`, `count`, `fraction`, `text`, `counts`.
   - `Fraction`: `{num, den}` with `den ≥ 1`.
   - `Weight`: `{num, den}` with `num ≤ den` and `den ≥ 1`; `0/0` is accepted only on a `pass` with a legacy cause.
   - `Finite`: a finite `f64` (refusing NaN and infinities), the payload of a `number` value.
   - `Instrument`: `{name, version (SemVer), digest: InstrumentDigest, dirty}`; a `git:` digest requires `dirty: false`.
   - `Evidence`: `{ref?: EvidenceRef, note?}`, at least one present.
   - `Cause`: one key: `expectation`, `artifact` (a `RelPath`), `reason` (a `ReasonCode`), or `legacy` (a `ReasonCode`).
   - `Probability`: `{p ∈ [0, 1], calibration_ref}`.
3. The cross-field rules, in `OutcomeRecord::validate()`:
   - a cause (legacy included) is required on `blocked`, `not_measured` and `not_applicable`, and absent on `pass` and `fail` unless it is legacy;
   - a `pass` weighs `num = den`, a `fail` `num < den`, a `blocked` `num = 0`;
   - a `gate` or `quality` record carries no `probability` and no dirty instrument (I5, I6);
   - `legacy` is the only cause `pass` and `fail` may carry;
   - `weight` is absent on `not_measured` and `not_applicable`;
   - `effective_weight()` returns the stated weight or the status default (pass 1/1, fail 0/1, blocked 0/1), and `None` for `not_measured` and `not_applicable`.
   - every struct is read from a JSON object only, never from an array.
4. Grit shape:
   - every struct is `#[non_exhaustive]` and built with `OutcomeRecordBuilder` (and builders for its members), whose `build()` runs `validate()`;
   - deserialisation goes through a private `RawOutcomeRecord` and `#[serde(try_from)]`;
   - serialisation omits absent options and never writes `null`, and a `null` on input is rejected;
   - `Value` and `Cause` are `#[non_exhaustive]`; `Status` and `Role` carry `// EXHAUSTIVE:`; `Cause::Legacy` is `#[deprecated]`;
   - errors are one `thiserror` enum, `OutcomeError`;
   - `#[must_use]` and `# Errors` throughout; field docs say what each field is and its range.
**Deliverables**: `crates/candler-schemas/src/outcome.rs` (`OutcomeRecord`, `OutcomeRecordBuilder`, `Status`, `Role`, `Value`, `Fraction`, `Weight`, `Instrument`, `Evidence`, `Cause`, `Probability`, `OutcomeError`, `OutcomeRecord::validate`, `OutcomeRecord::effective_weight`); `crates/candler-schemas/src/lib.rs` (`pub mod outcome`)
**Consistency Checks**: `cargo clippy -p candler-schemas --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(schemas): define the outcome record every evaluator emits`

## Step 2: Publish its JSON Schema
**Goal**: `outcome-record.schema.json` is generated, states the by-status rules, and is drift-checked.
**Implementation Logic**:
Register the root in `generated_schemas()`. A schemars transform adds, as `if`/`then` clauses, the rules serde cannot derive: cause by status, weight by status, and a `legacy` cause on `pass`/`fail` and `0/0` weights. It also sets `deprecated: true` on the `legacy` variant. The rules JSON Schema cannot express (`num ≤ den`, the pass and fail weight rules, name and 64-bit limits) stay in `validate()`, and the schema states them in a `$comment` that says what holds. Bless the file.
**Deliverables**: `crates/candler-schemas/src/outcome.rs` (`outcome_schema_transform`); `crates/candler-schemas/src/generate.rs` (the root's entry); `crates/candler-schemas/schema/outcome-record.schema.json`
**Consistency Checks**: `cargo nextest run -p candler-schemas drift` (expected: PASS)
**Commit**: `feat(schemas): publish the outcome record as JSON Schema`

## Step 3: Test it in three layers
**Goal**: The record is checked by domain-free properties, by one fixture per variant, and against real golden rows.
**Implementation Logic**:
1. **Properties** (`proptest`, strategies spanning every variant of every member):
   - a generated valid record serialises to JSON that the committed schema accepts (`jsonschema`), deserialises, passes `validate()` and round-trips equal;
   - one mutation per rule is rejected by both the schema and Rust: a `null` field; the `not-measured` spelling; a reason cause on `pass`; `blocked` without a cause; a weight on `not_measured`; `0/0` without a legacy cause; `num > den`; a value fraction with `den` 0; malformed digests, paths and ids;
   - where the two must differ (JSON Schema reads `3.0` as an integer, `u64` does not), a test asserts the difference.
2. **Variants:** `tests/fixtures/outcomes/synthetic/`, one record per row of the R01 table, including `not_applicable` with its cause, a text value and named counts.
3. **Golden acceptance:** `tests/fixtures/outcomes/golden/`, one record converted from each row of `legacy/golden_rows.jsonl`, by R01 §Golden corpus's mapping:
   - integer → `count`, float → `number`;
   - `score`/`max_score` → `weight` (omitted when default, dropped on `not_measured`);
   - `blocked_by` → `{artifact}`;
   - the marker reason, `zero_mappings`, `zero_weight` or `unrecorded` → `{legacy}`;
   - instrument `htn_creator_scripts`, version `0.4.2`, `git:28be2c158fbc6ee786a789242c2d8fd47bfde9a8`, dirty false;
   - evaluator `htn-creator-eval@0.4.2`.

   Identity fields are synthetic, and `outcomes/golden/README.md` says so. A mapping test re-derives each expected `(status, value, weight, cause, role)` from the raw row and grading entry through a table written in the test, independent of the fixtures, and compares. A float test checks that every value in `legacy/golden_floats.json` re-serialises to the same text.
**Deliverables**: `crates/candler-schemas/tests/outcome_properties.rs`; `crates/candler-schemas/tests/outcome_fixtures.rs`; `crates/candler-schemas/tests/fixtures/outcomes/{synthetic,golden}/*.json`; `crates/candler-schemas/tests/fixtures/outcomes/golden/README.md`
**Consistency Checks**: `cargo nextest run -p candler-schemas outcome` (expected: PASS)
**Commit**: `test(schemas): validate the outcome record by properties and fixtures`
