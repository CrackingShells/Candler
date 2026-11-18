# golden_design v0

Leaf `golden_design` of m6a-paired-stats (A8). Branch `m6a/golden_design`, base `afd32f0b`.

## Design

- **Plan row** (`gd_plan_twelve_cells`): the experiment file `fixtures/golden_design/experiment.toml` declares
  `model.id` at 3 levels and `request.treatment` at 4 table levels (htn-creator, tags 0.4.0 to 0.4.3) at 20
  replicates. `plan_runs` plans 12 cells and 240 runs, each (model, version) pair once with 20 runs, each cell
  holding its own level's treatment record (tag and commit).
- **Store**: `design.toml` (read by the test and by `second_engine.py`) fixes the pass counts of three binary
  expectations (G1, G2 gates; Q1 quality) and one weighted quality expectation (Q2, weights 5/5 and 2/5) for each of the
  12 cells. The first k replicates pass.
- **Planted effect**: version 0.4.0 to 0.4.1 raises G1 from 5/20 to 17/20 in model 1 and from 6/20 to 16/20 in model 2
  (differences 0.60 and 0.50) and leaves model 3 at 10/20: a real effect in 2 of 3 models. Versions 0.4.2 and 0.4.3 hold
  identical counts in every model and expectation: the null pair.
- **Why it has power**: CMH without continuity correction over 3 blocks of 20 + 20 runs, at true rates 0.25 to 0.85,
  0.30 to 0.80 and 0.50 to 0.50, rejects at alpha 0.05 with power and at the Holm-adjusted 0.025 (two gates) with
  power (200 000 simulated stores, numpy, numbers: 0.9899 0.9739). The planted store reads p = 6.9e-5 (Holm 1.4e-4), OR_MH 4.17
  with RBG interval 1.97 to 8.79; the null pair reads p = 1 and an interval spanning 1.
- **Second engine** (`oracle/golden_design/second_engine.py`; no Candler import; scipy 1.17.0, statsmodels 0.15.0):
  36 rows (9 comparisons x 4 expectations). Binary: battery, blocks, p (statsmodels `StratifiedTable`,
  `correction=False`), `oddsratio_pooled` and RBG interval, Mantel-Fleiss criterion, per-block counts and differences,
  reading, Holm over the gates per comparison. Weighted rows match battery (permutation), blocks, per-block differences
  and the point estimate (block-size-weighted difference of sum num / sum den). **Not matched**: the permutation p, the
  BCa interval and the seed (the draws belong to the core crate's resampling vectors); so the quality family's Holm is
  checked in the test by exact two-p-value arithmetic over the engine's Q1 p and the report's own Q2 p.
  `exact_conditional` and `midp` are in the engine (one block cross-checks them against each other) but no golden row
  reaches the exact fallback: every comparison passes Mantel-Fleiss, so the fallback is not exercised by this leaf.
- The report orders a pair as its store does (cell order), so a pair can come out B to A relative to the declared order;
  the test flips signs and inverts the odds ratio for a reversed pair and asserts the other quantities as they are.

## Bites

Each injected, seen red, restored from a copy of the file (git status clean after).

| Bite | Injection | First failing message |
|---|---|---|
| pooled instead of blocked | block key empty in `blocking.rs::gather` | `gd_comparisons_shape` count of blocks; `gd_agrees`: battery `mid_p` against `cmh` |
| blocks include the compared factor | `gather` keeps the compared factor in the key | `gd_agrees`: G1 per-block key `Null`, one-armed blocks; `gd_readings` left `Null` |
| continuity correction on | `cmh.rs`: `(abs(excess) - 0.5)^2 / variance` | `gd_agrees`: G1 p 0.00014557 against 0.00006909 |
| `level_of` ignores the typed treatment | the `REQUEST_TREATMENT` branch disabled | `gd_agrees`: no comparison of request.treatment 0.4.0 0.4.1; `gd_comparisons_shape` blocks 3 against the count held |

## Commits

- `2b22efaa` test(analysis): add a second engine for the golden design
- `9d3083f8` test(analysis): guard the golden design comparisons (also holds the plan row and the experiment file)
- the plan row has no commit of its own: it shares the test file with step 3

## Gates

- targeted test: PASS (4 tests)
- second engine independent and current: PASS (`rows match`)
- `tests/oracle` gains only new files: PASS
- `cargo clippy -p candler --all-targets -- -D warnings`: PASS; `cargo fmt --check`: PASS

## Claims checked

- 12 cells, 240 runs, one cell per pair: true (`gd_plan_twelve_cells`).
- 6 version comparisons with 3 blocks and 3 model comparisons with 4 blocks: true.
- Weighted point estimate equals the engine's block-size-weighted difference within 1e-9: true.

## Findings

1. **Spec defect: `gd_level_of_reads_prefix_object` is unreachable through `analyze`.** `report.rs::check_factors`
   refuses a recorded factor that is a strict prefix of a path where cells differ ("records the factor plan, which is
   broader than plan.model, where its cells differ"), so the object branch of `level_of` (values below a prefix factor)
   cannot be reached from the binary; `level_of` is `pub(super)`, so a test from `candler` cannot call it. And the
   leaf's parenthetical (two treatment levels differing inside the `request.treatment` object) goes through the typed
   `REQUEST_TREATMENT` branch, not the prefix branch. The test was not written; the bite "`level_of` reading the exact
   path only" is replaced by "`level_of` ignoring the typed treatment", which the golden shape guards. The prefix-object
   branch remains unguarded (possibly dead code): coordinator to rule.
2. **Plan row without a git repository.** The leaf asks for a fixture git repo with 4 tags and `source` pointing at it.
   Resolving 4 real levels goes through `candler run`, which launches a job with the fake docker client; 240 runs through
   it is the wrong weight. The row plans with `plan_runs` and four fabricated level resolutions (the way the core crate's
   `t3_each_cell_uses_its_level` does); the experiment file's `source = "skills"` is a placeholder it never resolves.
3. Pair orientation follows the store's cell order, not the declared level order (see Design).

## What is left

Nothing of the leaf except the ruling on finding 1.
