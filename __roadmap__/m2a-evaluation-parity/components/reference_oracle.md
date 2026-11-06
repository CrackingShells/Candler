# Reference oracle: A-C expected rows and rates

**Goal**: A Python script that imports nothing from Candler derives, from the raw golden corpus and `_outcome.py` at `28be2c15`, the A–C counts, the per-cell rates and the 209 zero-weight pairs, a second engine (OR1) re-derives them, and a hand-written `equivalence.json` states the one excused difference.
**Pre-conditions**:
- [ ] [cmd] ENV, and DEP(skeleton)
- [ ] [cmd] `test -d "$GOLDEN" && git -C "$R" cat-file -e 28be2c15:.claude/skills/htn-creator-workspace/infrastructure/htn_creator_eval/_outcome.py`
- [ ] [nl] The implementer of this leaf implements no other M2a leaf and reads no `candler-core/src/evaluation/` code
**Success Gates**:
- ⬜ [cmd] STD, SUBJ and SCAFFOLD pass, and OWNED passes with `^crates/candler/tests/(oracle/|guard_oracle_ac\.rs$)`
- ⬜ [cmd] `uv run --no-project python crates/candler/tests/oracle/gen_oracle.py --check "$GOLDEN" "$R"` regenerates every committed oracle file byte for byte
- ⬜ [cmd] `CANDLER_GOLDEN_CORPUS="$GOLDEN" cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'binary(guard_oracle_ac)'` passes (OR1), and fails when the variable is unset
- ⬜ [cmd] Independence: `test -z "$(grep -nE '^\s*(import|from) +candler|candler_(core|schemas)' crates/candler/tests/oracle/gen_oracle.py crates/candler/tests/guard_oracle_ac.rs)"`
- ⬜ [cmd] `jq` cross-check from the raw corpus, logged in the report: the A–C pass/fail/blocked/not_measured counts are 7,999/415/466/0, of which 209 passes have `max_score == 0`
- ⬜ [cmd] `du -sk crates/candler/tests/oracle | awk '$1>30{exit 1}'`, and the report records `shasum -a 256 crates/candler/tests/oracle/rates_by_cell.toml`
**References**: [R03 §Scope move, §Generic diff](../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md) — the A–C counts, the equivalence file format; [R04 OR1, §Tautology traps](../../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md); `htn_creator_eval/_outcome.py` at `28be2c15` — gate and quality roles, the rate formula; `evals/eval.json` at `28be2c15` (sha `ca7f12af…`) — expectation ids and tiers; M1 `golden_oracle.md` — the corpus layout and the raw fields

## Step 1: Derive the A-C counts and the zero-weight pairs
**Goal**: The committed counts and pair list come from raw grading files and the pinned suite, never from the importer or the store.
**Implementation Logic**:
1. `gen_oracle.py <corpus> <R> [--check]` (stdlib only, `uv run --no-project`): read each run's `eval-K-grading.json` rows (status, `score`, `max_score`) and the expectation ids of `eval.json`, taken with `git -C <R> show 28be2c15:<path>` into a temp dir, its sha256 checked against `ca7f12af…` before use.
2. Tier membership comes from `eval.json`; `D-E1`…`D-E6` are the uncompared set. Write `counts.toml` with `[imported]` (pass, of which `zero_weight`, fail, blocked, not_measured, over A–C), `[live_expected]` (pass, `not_applicable`, fail, blocked, not_measured), `[uncompared]` (one count per D-E id) and `total`.
3. `na_pairs.txt`: one line `<cell dir>\t<run N>\t<expectation id>` per A–C pass with `max_score == 0`, sorted bytewise, ending in a newline.
4. The script never writes outside its output directory and never runs a git command that writes.
**Deliverables**: `crates/candler/tests/oracle/gen_oracle.py` (`derive_rows`, `write_counts`, `write_na_pairs`); `crates/candler/tests/oracle/{counts.toml,na_pairs.txt}`
**Consistency Checks**: `uv run --no-project python crates/candler/tests/oracle/gen_oracle.py --check "$GOLDEN" "$R" && test "$(wc -l < crates/candler/tests/oracle/na_pairs.txt | tr -d ' ')" = 209` (expected: PASS)
**Commit**: `test(evaluation): derive the a-c reference counts from the raw corpus`

## Step 2: Derive the per-cell rates with the reference formula
**Goal**: `rates_by_cell.toml` holds, per cell, the quality and gate rates that `_outcome.py` computes over A–C rows, in the exact format `candler show --rates --format toml` must print.
**Implementation Logic**:
1. Load `htn_creator_eval/_model.py` and `htn_creator_eval/_outcome.py` from the temp extraction of `28be2c15` with `importlib.util.spec_from_file_location`, under a bare `htn_creator_eval` package stub (its `__init__.py` imports the renderers and is never run), never from `$R`'s working tree or an installed package. If either needs a non-stdlib import, stop and report.
2. Roles are data, not code: each expectation's `role` (`gate`, `quality`, `descriptive`) is read from `eval.json` at `28be2c15`. Per run, build a `_model.RunView` of its A–C rows (id, status, role, `score`, `max_score`) and call `_outcome.score`. A cell's rate is the sum of its runs' numerators and denominators, which is `_rate_for_role` summed: `pass`, `fail` and `blocked` count (`blocked` with score 0), and `not_measured` and `not_applicable` are excluded.
3. Format, fixed here and copied into R03 §Generic diff for `surfaces`: one table per cell, `[cell."<cell dir>"]`, keys sorted, `gate = [<num>, <den>]` and `quality = [<num>, <den>]` as the integer score and max-score sums. NA rows, and 0/0 passes, contribute nothing to either sum.
4. The script adds a `[published]` comparison to its stdout (not to the file), from the corpus's own published rate file if one exists; the KT quotes it.
**Deliverables**: `gen_oracle.py` (`write_rates`); `crates/candler/tests/oracle/rates_by_cell.toml`
**Consistency Checks**: `uv run --no-project python crates/candler/tests/oracle/gen_oracle.py --check "$GOLDEN" "$R"` (expected: PASS)
**Commit**: `test(evaluation): derive the per-cell reference rates with _outcome.py`

## Step 3: Write the equivalence by hand and re-derive with a second engine
**Goal**: The one excused difference is a hand-written data file, and an engine that shares nothing with the script confirms the script's numbers.
**Implementation Logic**:
1. `equivalence.json`, written by hand in R03's format (never generated): one rule, direction `from` → `to` only. `from` is `status pass`, weight `0/0`, cause `{legacy: zero_weight}`; `to` is `status not_applicable`, cause `{reason: empty_population}`, no weight. `README.md` explains each file, its source and how to regenerate it.
2. `tests/guard_oracle_ac.rs` (`#![cfg(test)]`, `#[ignore]`, panics when `CANDLER_GOLDEN_CORPUS` is unset), OR1 (R3): reads the raw corpus with `serde_json::Value` only (no Candler crate) and recomputes the counts, `na_pairs.txt` and every cell's gate and quality sums. The gate and quality role membership is a hand literal copied from `eval.json`'s text at `28be2c15` (one id list per role), and the counted statuses are a hand literal of `_outcome.py`'s `_COUNTED_STATUSES`, cited by line. Each must equal the committed file. Oracle: the raw corpus via a second engine.
3. Bite: give one expectation the wrong role in the literal and see OR1 fail. Record it.
4. Dispatch the plain verifier: role membership against `_outcome.py`, source independence, and the 209 rows.
**Deliverables**: `crates/candler/tests/oracle/{equivalence.json,README.md}`; `crates/candler/tests/guard_oracle_ac.rs` (OR1)
**Consistency Checks**: `CANDLER_GOLDEN_CORPUS="$GOLDEN" cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'binary(guard_oracle_ac)' && jq -e 'length == 1' crates/candler/tests/oracle/equivalence.json` (expected: PASS)
**Commit**: `test(evaluation): re-derive the reference with a second engine`
