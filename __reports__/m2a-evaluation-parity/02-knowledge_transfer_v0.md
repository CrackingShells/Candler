# M2a evaluation parity: knowledge transfer

## Executive summary
- **What was built.** M2a adds the plug-in registry, the evaluation engine, the Python evaluator helper, the HTN plug-in and `candler import | evaluate | show | diff`.
- **The parity result.** A live regrade with `htn-creator-eval@0.5.1` reproduces the 0.4.2 import exactly:

  | compared | unchanged | excused | changed | uncompared | total |
  |---|---|---|---|---|---|
  | 8,880 | 8,671 | 209 | 0 | 1,440 | 10,320 |

  - The 1,440 uncompared rows are D-E1…D-E6 × 240, as the A–C scope move intended.
  - The 209 excused rows are the zero-weight passes that the equivalence file maps to `not_applicable`.
- **Throughput.**
  - The regrade went from 13 to 157–158 items/s.
  - The corpus guard went from 24 min to 162 s, and the workspace suite runs in 11.7 s.
  - The release import takes 7.7–9.1 s.
- **Status.** Every node of [the roadmap](../../__roadmap__/m2a-evaluation-parity/README.md) is done, and no verifier holds an open BLOCKER or MAJOR.
- **What remains.** The rebase onto main after PR #4 (M2b), the HTN re-pin, Linux clippy, the `architecture.md` check, and the PR. The PR is pushed on Eliott's go.

## Wins
- **The oracle is independent, and it bit.** `gen_oracle.py` reads only the raw corpus and three HTN files. OR1 re-derives the oracle a second way and caught a generator that was wrong but self-consistent, which `--check` alone passed ([reference_oracle](reference_oracle/00-report_v0.md) §3).
- **Adapt, don't port.** `grader.py` and the checkers ran byte-identical inside forked children. Kind parity (KP1) held on 240 × 37 rows, so no grading logic was rewritten.
- **The throughput was found by profiling, not by subsets.** Eliott ruled out a fast subset. The [profile](throughput/00-regrade_profile_v0.md) located the time, and four changes took the regrade to about 12× its old rate:
  - merged batches;
  - a plug-in preload;
  - `fsync` instead of `F_FULLFSYNC`;
  - a lazy helper start.

  The regrade is now bound by cores and the per-item fork.
- **Guards were bitten.** Every cornerstone guard has a recorded bite in its leaf report, and the verifier's surviving bites became ENG1 rows ([throughput_proof](throughput/throughput_proof/00-report_v0.md), "Verifier round").

## Pain points
| Pain | Cost | Where it is recorded |
|---|---|---|
| Tag 0.5.1 was cut before the helper re-pin | The tag was withdrawn and recut, and the PR body explains both cuts | [rulings](l1/00-rulings_v0.md) |
| A deadline guard slept 33 s to prove a deadline | It was the suite's critical path (35.9 of 35.9 s) and proved only a lower bound | rulings, "The deadline guard (fix round)" |
| The leaf's import gate named no binary | The debug run (44 s) failed a gate that release passes in 7.7–9.1 s. A ruling was needed | rulings, "The import gate (Eliott, 2026-11-09)" |
| The `_await` pipe race in the helper | It forced the 0.5.1 recut | regrade_throughput report |
| Nextest runs each test in its own process | The three corpus guards share one fixture through a lock and a marker, and a bite that breaks the build costs three builds | [golden_parity](golden_parity/00-report_v0.md), finding 3 |
| Spec gaps found by implementers (wire role and digests, describe mode, editable digest, prior) | Each needed a ruling before landing | [helper_runtime](helper_runtime/00-report_v0.md) §5 |

## Root causes
- **A release must re-pin what it depends on.** The plug-in's lock pinned a helper commit older than a fix it needed. The tag procedure had no step that checked the pin against the helper's tip.
- **A gate must name what it measures.** "The import takes 30 s or less" left the binary open. A guard that can read a duration should read it rather than wait for it to pass.
- **The spec ran ahead of the wire.** The `OutcomeRecord` and the `Item` fields were fixed before the helper existed, so the helper could not fill `role`, the digests or `prior` from what it received.

## Next-cycle changes
1. **Tag procedure.** Before a plug-in tag, run `uv lock --upgrade-package candler-evaluator` and check the pinned commit against the helper's tip. Add the re-pin line to the CHANGELOG.
2. **Gate wording.** Every timing gate names the binary profile and the machine. The ruling principle stands: a guard that sleeps to prove a duration is not accepted when the duration can be read.
3. **Corpus guard cadence.** Run it before landing any leaf that changes the engine, store, helper or HTN plug-in, and before the PR. It takes 162 s.

### Follow-ups
| Follow-up | Scope | Detail |
|---|---|---|
| **Import throughput** | Candler | About 40% of import time is the gzip round trip (`frames/mod.rs:314`; `trace::normalise` gunzips again in `import/assemble.rs`). About 20% is one `sync_dir` per blob, and the import runs on one core (`assemble.rs:513`). The cost is linear, at about 33 ms per run. An estimated 3–4× gain. |
| **KP4 test 2** | HTN | The test that compares outcomes with and without the preload kills no bite. Rework it with forked children, or drop it. |
| **Sync gaps** | Candler | The `frames` `.jsonl.tmp` stream and the environment `ready` marker are never synced, and a new index file's directory entry is not dir-synced. These predate A1. |
| Tier D | HTN | D-E1…D-E6 (1,440 rows), snapshot attach and suite `inputs[]`. The registry refuses `snapshots` and `inputs` needs today. |
| `git+file` → GitHub pin | HTN | The lock points at a local checkout. Move it to the GitHub URL once Candler is pushed. |
| The 0.4.2 instrument spelling | Candler import | It is not the live `htn-creator-scripts` 0.1.0 form. |
| Engine and store | Candler | Make `ReaderFactory` fallible. Seal the outcome files. Make `diff` report I4 violations. A dependent in the same request still costs its own item. |
| Docs | Both | Stale `Mapping` annotations. The `deltas.toml` C6 wording (unreadable C1/C5). The `specs.py` deadline docstring. Plug-ins must not wrap commands in `uv run`, because Candler activates the environment itself. |
| Small | HTN | HP2 tests a single byte. A duplicate prior takes the first record. `regex_engine` is `python-re`. |
| Wire note | Docs | A kind's `prior[]` also holds the records of earlier siblings in the same request (helper_runtime finding 4). |
| Worker-pool ceiling | Candler | Profiling projects about 400–600 items/s before the fork and the cores bind again. |

### What M2c needs from M2a
- **Comparable paths M2a reads.**

  | Path | Read by | For |
  |---|---|---|
  | `plan.prompt_domain_name` | `context/assemble.rs` (`needs.metadata` → `comparable.values`) | The HTN kinds' `<<domain_name>>` |
  | `plan.config` | `engine/plan.rs` `applies()` → `run.comparable()` | Every suite entry's `applies_to` (`with_skill`, `without_skill`) |
  | `plan.model` | The HTN import rules (`import/htn-results-rules.toml`, `metadata`), via `candler import --rules` | The metadata the import records |

  The artifact need is `outputs/**`. The comparable itself is built at import, in `import/manifest.rs`.
- **Tier D `needs.inputs[]` kinds.** The HTN plug-in will want a servers directory and a tool-catalog snapshot.
- **One schema addition.** The run-manifest `ImportRecord` gained an optional `rules` digest. It is additive and is not in the cell digest.

## Gate and quality rates per cell
- The corpus publishes no gate or quality rates. `PROVENANCE.json` holds only the per-run tier A/B/C pass rates, and their per-cell means are set beside the live rates for comparison. The tier means are not the same measure, because gate and quality are role sums.
- The live rates equal `rates_by_cell.toml` byte for byte.

| model | skill | gate | gate % | quality % | published A | B | C |
|---|---|---|---|---|---|---|---|
| claude-haiku-4-5 | 0.2.3 | 611/696 | 87.8 | 96.1 | 0.900 | 0.800 | 0.748 |
| claude-haiku-4-5 | 0.2.4 | 688/771 | 89.2 | 96.8 | 0.939 | 0.847 | 0.777 |
| claude-haiku-4-5 | 0.2.5 | 680/764 | 89.0 | 95.4 | 0.925 | 0.825 | 0.771 |
| claude-haiku-4-5 | 0.2.6 | 766/849 | 90.2 | 95.3 | 0.910 | 0.794 | 0.802 |
| claude-sonnet-4-6 | 0.2.3 | 907/1003 | 90.4 | 99.2 | 1.000 | 0.925 | 0.832 |
| claude-sonnet-4-6 | 0.2.4 | 867/931 | 93.1 | 99.8 | 1.000 | 0.975 | 0.880 |
| claude-sonnet-4-6 | 0.2.5 | 905/991 | 91.3 | 99.3 | 1.000 | 0.925 | 0.847 |
| claude-sonnet-4-6 | 0.2.6 | 904/928 | 97.4 | 99.7 | 1.000 | 0.975 | 0.915 |
| claude-sonnet-5 | 0.2.3 | 928/1022 | 90.8 | 98.5 | 1.000 | 0.825 | 0.781 |
| claude-sonnet-5 | 0.2.4 | 971/1070 | 90.7 | 98.2 | 1.000 | 0.800 | 0.788 |
| claude-sonnet-5 | 0.2.5 | 939/1023 | 91.8 | 99.0 | 1.000 | 0.875 | 0.816 |
| claude-sonnet-5 | 0.2.6 | 962/1011 | 95.2 | 98.4 | 1.000 | 0.825 | 0.839 |

- The numerators and denominators of the quality column are in [golden_parity](golden_parity/00-report_v0.md).
- The published means are printed by `gen_oracle.py --check`.

## Artifacts to preserve
- **The oracle.** `crates/candler/tests/oracle/`: `gen_oracle.py`, `counts.toml`, `na_pairs.txt`, `cells.tsv`, `rates_by_cell.toml` and `equivalence.json`.
- **The rulings log.** [`l1/00-rulings_v0.md`](l1/00-rulings_v0.md) holds every ruling, including Eliott's.
- **The throughput record.** The [profile](throughput/00-regrade_profile_v0.md) and the [throughput_proof report](throughput/throughput_proof/00-report_v0.md), with the before and after timings.
- **The HTN tags.** `htn-creator-eval@0.5.0` (e96257d9) and `htn-creator-eval@0.5.1` (ae4eef09) are local annotated tags in `$R` and are not pushed.

## Open questions
1. **Import throughput.** Does it become an M2 amendment, or wait for the importer's next owner? Release already meets the 30 s gate.
2. **Tier D.** Who owns it on the HTN side, and does it need M2c's `inputs[]` first?
3. **The helper release.** Does `candler-evaluator` go to PyPI with M2a, or stay on the `git+file` pin until the HTN lock moves to GitHub? Publishing is Eliott's call.
