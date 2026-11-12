# analyze, leaf report v0

`candler analyze`: Candler's default tables over a store's outcomes, built on the `stats` methods. A Sonnet implementer did the work in three rounds. The required Sonnet verifier attacked twice: FAIL, then PASS. The coordinator rebased the branch onto 6514ef4, resolving one dispatch conflict in `main.rs`. Branch `m3/analyze` has 6 commits, head dc45381, merged as `merge(analysis): land analyze`.

## What `analyze` reports
- **Arms:** a (stratum, cell). The stratum is the manifest's experiment digest. Each arm has its rates per role with their counts, the statuses, and a Wilson 95 % interval per expectation where every weight is 0/1; otherwise `omitted: weighted`.
  - Invalid runs are counted per reason.
  - Experimenter ends (`killed`, `budget_stop`, `interrupted`) are listed apart and never charged to an arm (I20).
  - Unsealed runs are counted as `unsealed`, outside k, n and every verdict. A run with no manifest lands in an `unknown` stratum.
- **Factors:** each stratum's factors come from its manifests' `factors`, which Candler's own `varying_paths` computes.
  - `--experiment <file>` is a cross-check: a disagreement is refused as `analysis.experiment_disagrees`.
  - A recorded factor that is a strict prefix of a differing path is refused as `analysis.factor_broader_than_variation`. Candler's `varying_paths` cannot produce one; the check guards hand-written or imported manifests.
- **Comparisons:** every pair of cells in one stratum that differs in exactly one recorded factor.
  - The verdict is `stratum_verdict` over every valid run of both arms. `analyze` is its first `src` caller, and `compare.rs` is untouched.
  - A comparable pair gets, per expectation graded in both arms, Fisher's two-sided p and a Newcombe difference interval. An expectation graded in one arm only is an omitted entry with the reason `one_arm`.
  - A refused pair keeps both arms' rates and its causes, and takes no part in any test.
  - Pairs differing in zero or several factors are counted in `pairs_not_one_factor`.
- **Multiplicity:** Holm, one family per role per call, over every tested entry. The family size is reported. α = 0.05, with the source `core_default`.
- **Per-run rows:** an open map of every recorded `comparable` and `measured` field, so M4's `session.*` and `census.init_tools` appear with no change.
- **Output:** JSON, or CSV tables (`arms`, `comparisons` with a row per refused pair and its `causes`, `runs`, and `summary`). CSV lines end in CRLF.
- **Codes** (`[analysis]`):

  | code | side |
  |---|---|
  | `analysis.mixed_evaluator_versions` | config |
  | `analysis.unknown_evaluator` | config |
  | `analysis.ambiguous_evaluator_name` | config |
  | `analysis.experiment_unreadable` | experiment |
  | `analysis.experiment_disagrees` | experiment |
  | `analysis.factor_broader_than_variation` | environment |
  | `analysis.no_outcomes` | environment |

  The three evaluator-selection codes are `config` because they are fixed by a different `--evaluator` value on the command line, not by editing a file. `StatsError` is `[internal]`.

## Rulings taken in the leaf (coordinator, 2026-11-12, for Eliott's review)
1. **The experiment file is not derived from a digest.** A manifest's experiment digest covers the resolved cells, so no file can be matched to it without a `Resolution`.
   - Round 1 kept `--experiment` as the only source of factors.
   - After the verifier showed that a wrong file passes silently, round 3 derives the factors from the manifests. The file became a cross-check.
   - This reverses the round-1 ruling.
2. **`main.rs` is edited outside OWNED,** in its one dispatch line. The scaffold had dispatched the stub with the wrong signature.
3. **A4's oracle covers rows A–C only.** The imported evaluator also grades D-E1 to D-E6, four of them `gate` rows. A4 therefore compares the A–C rows with `rates_by_cell.toml` exactly (12/12 cells), and checks the D–E rows by the identity rate = A–C sum + D–E rows. The oracle is untouched.
4. **A run without `run.done` is unsealed, not an error.**

## Verifier
**Round 1, FAIL**, at 07f16ee. Every gate passed, and 110 values matched scipy and statsmodels with no mismatch.
1. **HIGH:** a cell present in two strata was pooled across them, and its pair was duplicated, which doubled the Holm m. An experiment that adds a level makes this case.
2. **MEDIUM:** a wrong experiment file went undetected. A parent path made two-factor pairs comparable.
3. **MEDIUM-LOW:** the CSV dropped refused pairs.
4. **LOW:** an unsealed run failed the whole call with `candler.internal`.

**Round 2, PASS**, at d4bcf53.
- A store with only some cells of a 2×2 run is not refused.
- The four findings are fixed.
- A two-stratum store matched scipy and statsmodels on 36 values (gate m = 12, quality m = 6).
- An unsealed run or a run with no manifest joins no comparison or family.
- **LOW, accepted:** a hand-forged manifest with `factors: []` yields no compared pair. Such pairs count only in `pairs_not_one_factor`. This follows from ruling 1.

## Bites (each seen red, then reverted)
- **A1:**
  - invalid runs counted in k and n;
  - `blocked` left out of n;
  - `killed` charged as invalid;
  - Holm run per pair, and Holm removed;
  - the per-run map filtered to an allowlist;
  - `num` and `den` swapped through the binary;
  - the CSV `zero_weight` column dropped;
  - all strata pooled;
  - an unsealed run counted in n.
- **A2:**
  - a comparison made across a refused pair;
  - its cause dropped;
  - the verdict over the first run per arm;
  - the wrong-experiment checks disabled;
  - the refused CSV row dropped;
  - the `one_arm` entry dropped.
- **A3:** mixed evaluator versions accepted; a bare version accepted when two names hold it.
- **A4:** `not_measured` counted in the denominator; `blocked` counted as a pass.
- **R2-style:** the plain `refused(text)` restored; the side of `mixed_evaluator_versions` back to `suite`; `.with_file` removed.

## Gates
All PASS on dc45381, rebased, run by the coordinator:
- fmt, workspace clippy with `-D warnings`, workspace nextest (542 passed, 21 skipped), `cargo doc` with `-D warnings`, and the Linux clippy;
- A1 (10), A2 (10), A3 (4), R1 (2);
- A4 GOLDEN (2, 52.6 s);
- `cog check`.

The leaf's own gates also passed: OWNED apart from `main.rs`, `compare.rs` untouched, no imported-stratum hardcoding, doc lines, G-digest, G-text and `guard_layering_i1`.

## Production
| Round | Time | Tokens |
|---|---|---|
| Implementer, round 1 | 38 min | 360k |
| Implementer, round 2 (rebase, codes, sides) | ~25 min | 388k |
| Verifier, FAIL | 13 min | 200k |
| Implementer, round 3 (findings) | 21 min | 74k |
| Verifier, PASS | 13 min | 249k |

Token counts are the harness's per-call figures. Peak target dir is not recorded. A4 takes 52–73 s, depending on load.
