# M6b surface consolidation build: knowledge transfer (v0)

Date: 2026-11-24

## Executive Summary
- **What changed.** The build covered R16's accepted rows (verdicts v3) in nine leaves over three levels, on `claude/rdm-m6b-surface-consolidation-8c47a6`:
  - every data verb prints a versioned record as JSON, and `--format text` renders that same record;
  - refusals carry their inner codes;
  - `resolve` refuses more errors before a run;
  - each job checks its MCP servers before its first run;
  - one table owns the configuration paths;
  - `analyze` prints a record;
  - the experiment file gains an `env` table;
  - R01 and the README are realigned.
- **Outcome.** Every standard gate is green on `82d28def`: fmt, clippy, doc, LINUX, SUBJ, DIGEST (9/9 and 47/47) and GOLDEN (2/2 and 1/1). nextest ran 836 tests; the first full run stopped on the `late.sh` timing flake (see below), and the full rerun passed 836/836. No digest moved. Every verifier closed.
- **Tests.** 27 test functions were added and 7 renamed (the `stratum` tests became `comparability_class` tests); none was removed. nextest went from about 809 to 836.
- **Withdrawn.** C1, the `resolve` plan counts, is out of M6b by Eliott's round 08. Building it made `resolve` run `run`'s setup and report sealed runs as new. It was withdrawn in new commits, and it returns as a follow-up that first needs a read-only run-key count. N6 is rejected (round 07). S1 and S2 are out of M6b (their own PR).

| Level | Leaf | Ids | Merge |
|:--|:--|:--|:--|
| L0 | `codes_inner` | G8 | 4705b21e |
| L0 | `analysis_core` | F2, C6, C8 | f8d504a1 |
| L0 | `preflight_checks` | G5, N2, N4 (C1's method withdrawn, N6 rejected) | 35653cb1 |
| L0 | `server_check` | C2, H1 | 0cbd72ff |
| L1 | `output_contract` | X1, G11, X10, D1, X4, N8 (C1 withdrawn) | f1b8b0b7 |
| L1 | `knowledge_pages` | C3, G20, H3, X7, X3, X9 | faa4511c |
| L2 | `docs_chores` | X5, D2, X8, X11, G16, Q7, Q8, the R01 record and the Future rows | 096d8959 |
| L2 | `analysis_record` | X4, F5, C9, Q7, Q8 | 40aaef40 |
| L2 | `experiment_env` | N9b | 82d28def |

## Migration
This section is for the HTN lane's coordinator. It ships in the first release after the supervisor's M6b PR merges.

- **Defaults.** Every data verb (`ls`, `ps`, `show`, `inspect`, `diff`, `analyze`, `resolve`, `watch`, `run`, `kill`, `ingest`, `evaluate`) now prints JSON on stdout, and every refusal prints a `refusal/1` document on stderr. Add `--format text` to get text back. `show` and `diff` no longer require `--format`.
- **Retired flags, with no alias** (clap refuses them as unknown):

  | Retired | Use instead |
  |:--|:--|
  | `--error-format` | the default for JSON; `--format text` for text |
  | `watch --json` | `watch` |
  | `resolve --json` | `resolve` |
  | `analyze --format json` | `analyze` |
- **Inner codes.** A value-invalid file now prints its own code instead of `*.document_invalid`:
  - `experiment.count_zero` and `experiment.cost_not_positive`, both with a pointer;
  - `experiment.factor_without_levels`, `experiment.table_empty`, `experiment.path_overlap` and `experiment.entry_repeated`;
  - `experiment.input_invalid` at `/inputs/N`;
  - `suite.list_empty` and `suite.json_invalid` at `/expectations/N`.

  `document_invalid` stays only for shape errors.
- **`analyze` keys:**
  - the record has a `schema` key, `analysis-report/1`;
  - `reading` is now `interval_reading`, in the JSON and in the CSV column;
  - the comparison key `stratum` is now `comparability_class`;
  - each arm has a `spent` key, and the record has a `summary` key.
- **Other records.**
  - The diff report gains `schema` and drops a null `equivalence`.
  - The `show` counts and the `ingest` document gain `schema`.
  - `inspect <run> --transcript` is new: it prints a run's stored trace, one event per line. A run without a trace is refused with `analysis.no_stored_trace`.
- **The `env` table.** An experiment file may hold `[env]`, which sets variables for the harness process. Its values enter the cell digest. A file without `[env]` hashes as before. `resolve` refuses these entries:
  - a name Candler sets: `PATH`, `HOME`, `VIRTUAL_ENV` or `LANG` (`experiment.env_reserved`);
  - a declared secret (`experiment.env_secret`);
  - a name that is not a variable name (`experiment.env_name_invalid`);
  - `env` given anywhere but `[env]`: a `[fixed]` key, a factor or `--set` (`experiment.env_source`, A11).

  Secret values still reach the gateway only.
- **The server start check.** Before a job's first run, each declared stdio MCP server is started in a container set up like a run's. If one exits, the job is refused with `jobs.server_exited_at_start` (experiment side), or with `jobs.server_check_failed` (environment side) when the check itself fails. Remote servers are skipped.

## Wins
- **Coordinator rulings at stop points kept the leaves honest.** Each cap stop came back with a list of what was left and of the defects found. The ruling went into the build README as an amendment (A6 to A11), and the same agent resumed with its context intact.
- **Implementer-owned verifiers found real defects:**
  - a transcript line that could vanish silently;
  - a hand-mapped `Reading` that folded new variants into "inconclusive";
  - a factor over `env` that skipped the reserved-name refusal.
- **Generated pages and the append-only `codes.toml` gave no merge conflict worth the name.** The race rule held at every level: the second leaf to merge rebases and regenerates, and never hand-merges a generated file.
- **DIGEST held at every merge.** No cell, run or golden digest moved across 9 leaves.

## Pain Points
- **Caps ran 1.5 to 3 times over the estimates:**
  - `experiment_env` spent 30 calls on its first reads alone and needed 85 + 45 + 25;
  - `analysis_record` needed 77 + 25;
  - L1 cost about 3.3 times its estimate.
- **Leaf briefs stated false facts about the tip:**
  - `analyze` taking `OutputArgs`;
  - the container plan living in `run.rs`;
  - `env.<NAME>` being a valid field path.

  Each one cost a stop and a ruling.
- **C1 was built, then withdrawn.** The ruling (Q5, option A) assumed that `resolve` could count without `run`'s setup.
- **Guards were often written with or after the code,** not first, so the bites carried the "can fail" proof. Bite 2 of `experiment_env` found a gap (the CLI feed was not covered by any DIGEST vector) and added its own guard.

## Root Causes
- **The design estimates priced the writing, not the reading.** Leaves that cross three crates spend half their calls locating facts through colgrep.
- **The leaf files were written from R16's text, which named files and shapes from the verdict study.** No first read checked them before the cap was set.
- **Rulings on a proposed behaviour (C1) were made before anyone checked that it could be computed without side effects.**

## Next-cycle Changes
- **Instructions.** A leaf that names a file, a type or a flag as a fact on the tip must cite a colgrep hit for it, taken when the leaf is written. If it cannot, its step 1 is a probe with its own small cap.
- **Workflow.** Set each cap at 2 times the estimate for cross-crate leaves, and keep 1.5 times for single-crate leaves.
- **Review.** Before ruling that a read-only verb shows a derived number, confirm that a read-only path to that number exists.

## Follow-ups
Each follow-up is marked for an R01 §Future features row (**Future**), for the diet milestone that follows M6b (**diet**), or for **nobody** (accepted as it stands).

| Follow-up | Origin | For |
|:--|:--|:--|
| C1, the `resolve` plan counts, which needs a read-only run-key count in `candler-core` first; `PlanOutcome::summary` has no CLI caller | output_contract, preflight_checks | Future |
| Varying `env` across cells (a factor over `env`) | experiment_env (A11) | Future (R01 row added at the close) |
| Driver and gateway variable names refused at `resolve`, which needs a driver at resolve | experiment_env (A8) | Future |
| `evaluate` and `resolve`'s failure document print hand-built shapes with no record | output_contract | diet |
| `watch --live` and `--format text` mean the same thing; retire one | output_contract | diet |
| Key names still spelled as literals in the value readers, the `slots` check and `is_location` (F2); five private path texts in `paths.rs`; `status` and `usage_failure` still type 0/1/2 | knowledge_pages | diet |
| `probe_env` in `isolation/image.rs` writes `HOME` as a literal | experiment_env | diet |
| `ResolveInput::secret_names` defaults to empty, so a caller that skips it skips the secret check | experiment_env | diet |
| An `env` name equal to `model.token_secret` is not refused (only `secrets` are checked) | experiment_env | diet |
| No guard that the gateway does not receive the `env` table (the plan has no gateway environment to assert on) | experiment_env | diet |
| `crates/candler/build.rs:66` and R01 outside §Analysis statistics still use "stratum" for the comparability sense | analysis_record, docs_chores | diet |
| No CLI test of `inspect --transcript`, no `--format text` content test, no guard that text and CSV read only the record | analysis_record | diet |
| `text_line` in `trace/transcript.rs` prints a degraded line on a serialisation failure (no line is dropped) | analysis_record verifier | diet |
| Record fields for role, cell, id and factor are strings, not the primitive types | analysis_record | diet |
| `Summary` omits refused comparisons; the level-order branches for numbers and treatments are guarded only through strings | analysis_core | diet |
| `[fixed] replicates` silently wins; `replicates.x` paths are accepted as no-ops; `NotPositive` for a non-integer | preflight_checks | diet |
| Server check: probes run serially; a non-string `command` or `args` is skipped silently; no `codes.toml` row for pre-run refusals beyond the two; `sh` is assumed in the image | server_check | diet |
| `ReservedRoot`, `ReservedValue` and `AbsolutePath` have no test row | codes_inner | diet |
| The README quick start's `resolve` step: no repository fixture resolves offline | docs_chores | nobody |
| Commits that are not green on their own (output_contract step 3, codes_inner, analysis_core step 1) | several | nobody |

## Open observation: the `late.sh` timing flake
This is not a follow-up of any leaf. The test `candler-core::it guard_protocol_client::every_failure_path_of_an_evaluator_is_accounted_for`, row `late.sh`, fails only under the load of a full `cargo nextest run --workspace`:
- Its stub sleeps 0.9 s inside a 1.9 s window (a 400 ms deadline plus a 1.5 s cancel grace). Under load, the run took 3.15 s, so the supervisor stopped the stub.
- It passes when run alone.
- It was first seen failing at the L0 close on `8872d38a`, and it passed in the full runs on `0cbd72ff` and at the L1 close.
- No build commit touches it.

The supervising session asked for it to be recorded so that it gets fixed. A fix would widen the window or use a clock the test controls.

## Spend
These figures were measured with `spend.py` on this session's transcripts after the last merge (82d28def), before the close commits. The build's share is the session total ($106.01) minus the verdict study's $39.62 (main $26.62, subagents $13.00).

| Budget row | Planned $ | Spent $ |
|:--|--:|--:|
| Implementers, verifiers and fix rounds (subagents) | 32.4 | 43.4 |
| Coordinators, three levels, and the close | 15.1 | 23.0 |
| Contingency 15 % | 7.1 | n/a |
| **Build** | **55, cap raised to 90 (A5)** | **66.4** |

| Level | Spent $ | Main overrun |
|:--|--:|:--|
| L0 (four leaves) | 32.6 | `preflight_checks` and `server_check` each needed a fix agent; the stop at $44 waited on Eliott's budget ruling |
| L1 (two leaves) | 16.7 | About 3.3 times the estimate; C1 was built, then withdrawn |
| L2 (three leaves) | 17.0 | `experiment_env` needed 165 calls against a 30-call cap, and `analysis_record` needed 102 against 77 |

The build stayed under the raised $90 cap, at 74 % of it. It ran 21 % over the original $55.

## Artifacts to Preserve
- The leaf reports under `__reports__/m6b-surface-consolidation/build/`, nine of them, each with its First reads, Bites, D3 and Verifier sections.
- The build README's Amendment Log (A1 to A11): the rulings and the reason for each one.
- `__reports__/agent-token-costs/scripts/spend.py`, the spend measure used at each level.

## Open Questions
- Should the diet milestone take every **diet** row above, or only those that remove a second source of a fact (hand-built shapes, literal key names, the duplicated `watch` flag)?
