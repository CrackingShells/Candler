# level_of_prefix: leaf report (v0)

Status: step 1 done; step 2 (pair-orientation sentence) held on a spec defect.

## Callers

`level_of` is `pub(super)`: only `analysis/` reaches it. (`jobs/orchestrate/session.rs` has an unrelated `level_of` over specs.)

| Function | Calls | Reached from | `check_factors` first? |
|---|---|---|---|
| `blocking.rs::gather` | `level_of` | `blocks_of`, `unit_blocks_of`, `unit_block_keys` | via the chain below |
| `report.rs::place` | `level_of` | `compare_pairs` | via the chain below |
| `blocking.rs::blocks_of` / `unit_blocks_of` | `gather` | `report.rs::tested_of` | via the chain below |
| `report.rs::tested_of` | `blocks_of`, `unit_blocks_of` | `entries_of` <- `compare_pairs` | via the chain below |
| `report.rs::compare_pairs` (pub) | `place`, `entries_of` | `Report::build` | via the chain below |
| `Report::build` (pub) | `compare_pairs` | `commands/analyze.rs::run` (production); `guard_analysis_report.rs` tests | analyze: yes, `check_factors(...)` is the statement before `Report::build` |

Verdict: the only production path is `analyze::run`, which runs `check_factors` before `Report::build`. `Report::build` and `compare_pairs` are `pub` and do not call `check_factors` themselves; their only other callers are tests (`guard_analysis_report.rs` lines 320, 714, 1125), none of which feeds a prefix-broad factor (the one broad store, `two_by_two(&["plan"])`, goes to `check_factors` only). So the branch was unreachable in the product; a library caller that skips `check_factors` would now get `null` for a prefix factor instead of an object.

## Commits

- `fix(analysis): drop the unreachable prefix branch of level_of` (tip of the branch at report time, before this report's commit)
- `docs(analysis): record the callers of level_of` (this report)

## Gates

- guard_stats_blocking + guard_analysis_report: PASS (30 passed)
- guard_analyze + guard_golden_design: PASS (17 passed)
- clippy -D warnings: PASS; fmt --check: PASS
- G3 loop and ownership check: run by the author of this report after the commit, see the final message

## Findings

1. Spec defect (step 2 held). The leaf says each comparison names `level_a` and `level_b`. `Comparison` has no such fields: it has `a` and `b` (the first cell of each level, `CellKey`) and `levels: Option<(Value, Value)>`; `level_a`/`level_b` are loop locals in `compare_pairs`. The sentence in `architecture.md` §Analysis statistics is not written. Proposed text once ruled: "A and B are the two levels in their recorded key order, not the declared order; `Comparison.a` and `.b` are the first cell of each and `Comparison.levels` the two levels, A first."
2. `Report::build` and `compare_pairs` are public and unguarded; if the coordinator wants the guarantee structural, `compare_pairs` could call `check_factors`.
