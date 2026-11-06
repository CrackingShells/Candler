# selection_rates: leaf report (v0)

The implementer's reports follow as they came back. The rulings (cell key, `&NameVersion`, interpretations (a)–(i)) are in [l1/00-rulings_v0.md](../l1/00-rulings_v0.md). The leaf is merged on report, without a verifier (R04).

## First report

## 1. Commits
- 3a4c75a feat(evaluation): select current outcomes and flag i4 changes
- 9b68a5f feat(evaluation): compute quality and gate rates per cell
- f62bcc5 feat(evaluation): diff evaluator versions on the four outcome fields
- f03f26b test(evaluation): guard the diff, the selection and the rates
Each commit passed `cog verify --file` and `cargo check -p candler-core` (steps 1-3 built with the later stubs still in place). Nothing pushed.

## 2. Gates
- STD: PASS. `cargo fmt --check` clean; workspace clippy `-D warnings` Finished; `cargo nextest run --workspace` "275 tests run: 275 passed, 3 skipped"; `cargo doc --no-deps` with `-D warnings` Generated.
- SPAWN (before the workspace run): PASS, "4 tests run: 4 passed" on guard_signal after `cargo clean -p candler-core`.
- Linux clippy (`--target x86_64-unknown-linux-gnu --workspace --tests -D warnings`): PASS, Finished.
- Leaf gate `cargo nextest run -p candler-core -E 'binary(guard_diff) | binary(guard_selection)'`: PASS, "6 tests run: 6 passed".
- SUBJ: PASS (`cog check` "No errored commits", all subjects <= 72).
- OWNED: PASS against the merge-base f5c2533 with the leaf regex. NOTE: `feat/m2a-evaluation-parity` has moved to 40138a6 since I was cut (docs commits); against the branch tip the gate lists those six `__roadmap__`/`__reports__` files, none mine.
- OWNED, only the `selection_rates_surface` block of public_surface.rs changed: PASS (one hunk, at that block).
- SCAFFOLD: PASS (no `mod scaffold`; 6 `#[test]` = 6 `Guards:` lines: guard_diff 4, guard_selection 2). I wrote no development tests.
- NOPUSH: PASS (empty ls-remote). DRIFT, PYSTD: not applicable (no schemas, no python touched).

## 3. Guards (all Oracle = hand literals; bites restored with `git checkout --`)
- DF1 `df1_every_planted_class_is_seen_and_evidence_is_not` (guard_diff). `Guards: R2 — the diff names each class of change in (status, value, weight, cause) and both kinds of absent row, sees nothing in evidence or in a default weight spelled out, and a base that diffs clean is the positive control of every row. Oracle: hand literals from the rule text.` One parameterised table, 10 rows: status; value 3->4; value 3->3.0; weight; cause kind; cause present->absent; missing row; extra row; evidence-only (changed 0); default weight spelled out (changed 0). Each row first diffs its base clean (compared 4, unchanged 4, changed 0, `equivalence` null in JSON), then plants one change (changed 1, unchanged 3, named class, run, expectation E1, and the absent side is null). Bite: `&& self.weight == other.weight` -> `&& true` in `fields.rs`: FAIL (left (0,4) right (1,3)). Second bite: value comparison reduced to presence: FAIL on the value rows.
- DF2 `df2_only_the_exact_listed_pair_is_excused`. `Guards: R2 — only a listed row that shows exactly pass 0/0 {legacy: zero_weight} then not_applicable {empty_population} is excused, counted apart from unchanged ... Oracle: the hand-written fixture equivalence.json and its shasum -a 256.` Table Z1..Z9: Z1 excused; Z2 unlisted same change = `status`; Z3 listed 1/1 pass->NA, Z4 0/0 pass other legacy code, Z5 reversed, Z6 unchanged, Z9 NA with another reason = `equivalence_mismatch`; Z7 absent in to, Z8 absent in from = `equivalence_missing`. Totals compared 11, excused 1, unchanged 2, changed 8; JSON `equivalence` = {sha256: <shasum literal d426cddf...>, rows: 8}; the report's seven JSON keys. Bite: `if a == &rule.from && b == &rule.to` -> `if a == a && b == b`: FAIL (left 6 right 1).
- DF2 `df2_unknown_keys_and_duplicate_rows_are_refused`: unknown key at the top, rule, side and row; a duplicate row in one rule and across two rules; a valid control file loads. Oracle: inline literals one edit from valid. Bite not separately tried (see section 5).
- DF2 `df2_uncompared_rows_are_listed_with_counts_and_nothing_compared_is_zero`: uncompared [{U1,2},{U2,1}] and an empty `to` gives compared 0 and lists everything. Oracle: hand-counted rows.
- SEL1 `sel1_a_changed_field_under_one_key_is_flagged_and_recovery_is_not` (guard_selection). `Guards: R5 — two records under one I4 key that differ in status, value, weight or cause are an I4Violation ... Oracle: the I4 text of R03 (§I4).` Each of four fields; request order beats input order; evidence/default weight/other context digest not flagged; instrument_error and deadline then measured is recovery (later current); measured then error, and a non-error not_measured then measured, are flagged. Bite: drop `&& !recovery` in `select.rs`: FAIL at guard_selection.rs:140. (The weight bite also fails it.)
- RT1 `rt1_three_hand_computed_cells`. `Guards: R1 — per cell the gate and quality pairs are the sums of the effective weights ... Oracle: the _outcome.py formula as R03 §Rates states it, summed by hand.` Cells a gate (1,3) quality (3,5); b (2,2),(0,4); c (0,0),(0,0); NA, not_measured, descriptive, 0/0 pass and another evaluator's records add nothing; the TOML text is a literal. Bite: let NA count as 0/1: FAIL (a gate (1,4), b quality (0,5), c gate (0,1)).

## 4. DF1 class -> diff output line
Each change is one JSON entry `{"class": ..., "run", "expectation", "from", "to"}` in `changes`: status -> "class":"status"; value 3->4 and 3->3.0 -> "value"; weight -> "weight"; cause kind and present->absent -> "cause"; missing row -> "missing" (`to` null); extra row -> "extra" (`from` null). Evidence-only and default-weight rows give no entry (changed 0). No wall-clock or probe records asked of this leaf.

## 5. Findings and deviations
1. Spec defect: `rates(selection, &EvaluatorId)`: no `EvaluatorId` exists. I used `&NameVersion`.
2. Spec defect (cell naming): R03's rates TOML keys are cell directory names (`20260806-035940_v0.2.3_...`), but a record's run holds only the cell digest. `CellKey` wraps that digest and prints its bare hex, so `show --rates` cannot byte-match `rates_by_cell.toml` without a digest -> directory-name map owned by surfaces/golden_parity. Needs a ruling.
3. Added public items: `fields::FourFields` (shared by select and diff), `rates_toml` (the TOML text), `Selection::{current,get,violations,is_empty,for_evaluator}`, `Equivalence::{from_bytes,sha256,row_count,rule_names}`, `EquivalenceError`. `Selection` holds all evaluators; callers use `for_evaluator` per side before `diff`.
4. Interpretations to confirm: (a) uncompared is listed in both directions (an expectation only `to` has is also listed; spec says only absent-from-`to`); (b) one change per row, class = first differing field in the order status, value, weight, cause; (c) "measured" = any status except `not_measured`, so error -> a different error is an I4 violation; (d) after a violation the later record is current; (e) if one version's selection holds several current records for one (run, expectation) (different context or suite digests), the latest request id wins silently; (f) a listed row on neither side is not in the join and is not counted; (g) a cell with any current record has a rates entry, zeros included; (h) `-0.0` differs from `0.0`; (i) `generator` is optional in the equivalence file, other keys refused.
5. Not done: the DF2 refusal test and the uncompared test have no bite recorded (the brief asks a bite per guard; the four bites above cover DF1, DF2, SEL1, RT1). `tests/fixtures/README.md` is outside OWNED, so it does not describe `fixtures/selection/`.
6. Base moved to 40138a6 during the work; I did not rebase.

## Fix round (after ruling A4)
- **Cell key.** `CellKey` prints through the schemas' `Digest::hex()`: 64 lowercase hex digits with no prefix, as `RunKey` spells it (M2 coordinator). RT1 also checks the key length and that `Digest::from_hex` round-trips.
- **TOML.** `rates_toml` returns a `String`. It writes `[cell."<cell hex>"]` tables sorted by key, one blank line apart, with one trailing newline. It is hand-formatted because the toml crate prints the key unquoted. RT1's text is a hand literal.
- **Bites for the DF2 guards.** Each was restored with `git checkout --`.

  | Guard | Mutation | Result |
  |---|---|---|
  | refusal | duplicate rows accepted (`if false`) | FAIL at `guard_diff.rs:392` |
  | refusal | `deny_unknown_fields` removed from `RawRow` | FAIL at `guard_diff.rs:392` |
  | uncompared | the `to`-only direction dropped | FAIL at `guard_diff.rs:466` |
  | uncompared | count always 1 | FAIL at `guard_diff.rs:466` |
  | excusal | the reverse direction excused too | FAIL: excused 2 against 1 |
  | excusal | any 0/0 pass cause excused | FAIL: excused 2 against 1 |

## Landing (coordinator, rebased onto 3d31151)
Rebased onto `3d31151` as `62bd916`, `fcbe1cb`, `ac5d061`, `f545ab9`, subjects unchanged.

| Gate | Result |
|---|---|
| OWNED | PASS against `3d31151` |
| SUBJ | `No errored commits`, subjects of 61, 57, 68 and 61 characters |
| SPAWN | `guard_signal` 4/4, after `cargo clean -p candler-core`, uid 501 |
| STD | `278 tests run: 278 passed, 3 skipped`, with fmt, clippy and doc clean |
| Linux clippy | PASS |
| `guard_diff` and `guard_selection` | 6/6 |

The leaf landed as `dd34a38` `merge(evaluation): land selection_rates`.
