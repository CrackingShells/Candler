# Selection, I4, rates and generic diff

**Goal**: `evaluation::selection` reads committed outcome files. It selects the current record per key and flags any I4 violation. It computes the quality and gate rates per cell. It diffs two evaluator versions on `(status, value, weight, cause)` exactly, counting missing and extra rows as changes, listing uncompared rows with counts, and excusing only the exact rows an equivalence file lists, counted apart and recorded by digest (DF1, DF2, SEL1, RT1).
**Pre-conditions**:
- [ ] [cmd] ENV and DEP(skeleton)
**Success Gates**:
- ✅ [cmd] STD, SUBJ and SCAFFOLD pass, and OWNED passes with `^crates/candler-core/(src/evaluation/selection/|tests/(guard_diff|guard_selection)\.rs$|tests/fixtures/selection/|tests/public_surface\.rs$)` with only the `selection_rates_surface` block of `public_surface.rs` changed
- ✅ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_diff) | binary(guard_selection)'` passes (DF1, DF2, SEL1, RT1)
- ✅ [nl] The report names, for each of DF1's planted classes, the diff output line that names it
**References**: [R01 §Data Model (outcome records), §Evaluator protocol (I4)](../../../__design__/architecture.md) — record keys, I4 text; [R03 §I4, §Generic diff](../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md) — the four fields, default-weight normalisation, the equivalence format, the uncompared listing, the rates format; `crates/candler-core/src/store/` — `outcome_files`, `read_outcomes`; `_outcome.py` at `28be2c15` — the rate formula (prose in R03); [R04 DF1, DF2, SEL1, RT1](../../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md)

## Step 1: Select records and flag I4
**Goal**: For each key, one current record is chosen, and a changed record under the same key is reported, never hidden.
**Implementation Logic**:
1. `select(records) -> Selection`: the I4 key is `(evaluator name@version, run, expectation, context_digest, suite_entry_digest)`. Records under one key are ordered by request id (v7, so time-ordered).
2. Two records under one key that differ in any of the four fields are an `I4Violation { key, first, second }`. The one exception is an error outcome (`not_measured` with an `instrument_error` or `deadline` cause) followed by a measured one, which is a recovery: the later record is current.
3. Otherwise the latest record is current.
**Deliverables**: `crates/candler-core/src/evaluation/selection/select.rs` (`select`, `Selection`, `I4Key`, `I4Violation`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(evaluation): select current outcomes and flag i4 changes`

## Step 2: Compute the rates per cell
**Goal**: Quality and gate rates per cell use the reference formula over current A–C records, with `not_applicable` outside the denominator.
**Implementation Logic**:
`rates(selection: &Selection, evaluator: &NameVersion) -> BTreeMap<CellKey, Rates>`, where `CellKey` is the run's cell digest and prints as its 64 bare hex digits (the `<cell hex>` of a run key `<cell hex>-r<N>-a<M>`): per cell, the integer sums of score and max score over that evaluator's current records whose own `role` (`record.role()`, I21) is `gate` or `quality`, as R03 transcribes `_outcome.py`. A `not_applicable` record and a 0/0 pass add nothing. No suite is read, and the core names no expectation. `Rates` serialises as R03's TOML format (`[cell."<cell hex>"]`, `gate = [num, den]`, `quality = [num, den]`, one blank line between tables). `surfaces` prints it.
**Deliverables**: `crates/candler-core/src/evaluation/selection/rates.rs` (`rates`, `Rates`, `CellKey`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(evaluation): compute quality and gate rates per cell`

## Step 3: Diff two evaluator versions exactly
**Goal**: The diff compares four fields, sees missing and extra rows, lists what it did not compare, and excuses only the listed rows that show exactly the rule's one change, in one direction.
**Implementation Logic**:
1. `diff(from: &Selection, to: &Selection, equivalence: Option<&Equivalence>) -> DiffReport`; `None` gives `equivalence: None` in the report (`null` in JSON). Rows are joined on `(run, expectation)`. Weights equal to the status default are normalised to absent on both sides first. Evidence is never compared.
2. A row in `from` with no match in `to` is `missing`, and the reverse is `extra`; both are changes. A row whose expectation is not in the `to` suite is `uncompared`, counted per expectation, and listed in the report with its count. A version's suite is read from its records (I21): an expectation is in it when the version has at least one record for it. `compared == 0` is reported as is; `surfaces` turns it into exit 2. `compared` counts the rows of the expectations in both suites, present on either side.
3. `Equivalence::load(path)` reads R03's JSON format with unknown keys refused and the file's sha256 kept; a row listed twice is refused. A rule's `from` and `to` are whole four-field records (an absent key means an absent field). A listed row is excused only when both sides exist and equal `from` and `to` exactly, in that direction; otherwise it is a change of class `equivalence_missing` (a side is absent) or `equivalence_mismatch` (any other pair, unchanged included). An unlisted row is never excused.
4. `DiffReport { compared, unchanged, excused, changed, changes: Vec<Change { class, run, expectation, from, to }>, uncompared: Vec<{expectation, count}>, equivalence: Option<{sha256, rows}> }`, with `unchanged + excused + changed == compared`. Change classes: `status`, `value`, `weight`, `cause`, `missing`, `extra`, `equivalence_missing`, `equivalence_mismatch`. The value class compares JSON numbers with their kind (`3` is not `3.0`). This is the type whose JSON key set `surfaces` exposes.
**Deliverables**: `crates/candler-core/src/evaluation/selection/diff.rs` (`diff`, `DiffReport`, `Change`, `ChangeClass`, `Equivalence`, `Uncompared`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(evaluation): diff evaluator versions on the four outcome fields`

## Step 4: Guard the diff, the selection and the rates
**Goal**: Four guards decide the module by hand literals and the I4 text.
**Implementation Logic**:
1. `tests/fixtures/selection/`: a hand-written base of a few records (built with the schema builders in the test, not captured), plus a hand-written `equivalence.json` in R03's format whose `rows` name a few base rows (not the golden file, which `reference_oracle` generates).
2. `tests/guard_diff.rs`:
   - DF1 (R2): one parameterised table, one planted change per row on a base that diffs clean (positive control): status, value 3→4, value 3→3.0, weight, cause kind, cause present→absent, missing row, extra row. Each row gives `changed == 1` with the named class. An evidence-only change gives `changed == 0`. Oracle: hand literal. Bite: drop weight from the comparison.
   - DF2 (R2): a listed row showing exactly `pass 0/0 {legacy: zero_weight}` → `not_applicable {empty_population}` is excused and counted in `excused`, never in `unchanged`, and `equivalence.sha256` is the file's. Each of these is a change with its class: the same change on an unlisted row; a listed row shown as a 1/1 pass → NA, as a 0/0 pass with another cause, in the reverse direction, or unchanged (`equivalence_mismatch`); a listed row absent on a side (`equivalence_missing`). A file with an unknown key or a duplicate row is refused. The uncompared list holds the expected expectations and counts. Oracle: the hand-written fixture file. Bite: excuse a listed row whatever pair it shows.
3. `tests/guard_selection.rs`:
   - SEL1 (R5): a changed field under the same key is an `I4Violation`; error→measured is a recovery and is not flagged. Oracle: the I4 text. Bite: remove the exception.
   - RT1 (R1): a hand-computed three-cell table; gate and quality sums equal literals, and NA is out of the denominator. Oracle: the `_outcome.py` formula in R03's prose. Bite: include NA.
4. Fill `selection_rates_surface`, delete scaffold tests, record bites.
**Deliverables**: `crates/candler-core/tests/guard_diff.rs` (DF1, DF2); `crates/candler-core/tests/guard_selection.rs` (SEL1, RT1); `crates/candler-core/tests/fixtures/selection/`; the `selection_rates_surface` block
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_diff) | binary(guard_selection)'` (expected: PASS)
**Commit**: `test(evaluation): guard the diff, the selection and the rates`
