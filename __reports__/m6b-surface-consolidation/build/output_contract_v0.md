# Output contract: build report (v0)

Branch `claude/m6b-build-output_contract`, branch point `e016b320`. The leaf stopped short of two of its rows (C1's printed summary and the `resolve_counts_equal_the_plan` guard); see Follow-ups.

## First reads

| Verb | Prints today | Record | Default after this leaf |
|:--|:--|:--|:--|
| `ps` | tab-separated rows | new `job-list/1` | JSON; `--format text` is the rows |
| `ls` | tab-separated rows | new `experiment-list/1` (with the `store` root) | JSON; `--format text` is the rows |
| `inspect` run, job | `json!` documents | new `inspect-run/1`, `inspect-job/1` (`spent` is `analysis_core`'s keys) | JSON; `--format text` lines |
| `diff` | core `DiffReport` JSON, or text, required `--format` | new `diff-report/1` | JSON; `--format text` lines |
| `show --rates` | `json!` `{cell: ...}`, or TOML, required `--format` | new `rates/1` | JSON; `text`, `toml` |
| `show` (counts) | `json!({"counts": ...})` | none: a hand shape R16 did not list, not built | JSON as before; `text`, `toml` |
| `watch` | `EventEnvelope` lines (already the schema type), `--json` or `--live` | `event-envelope` (existing) | JSON events; `--format text` draws the `--live` progress |
| `resolve` | tab-separated rows, or the core `Resolution` as JSON with `--json` | core `Resolution` (record deferred by R16) | JSON; `--format text` rows |
| `run` | the job id, one line | none (an id, not a document) | unchanged |
| `kill` | text lines | none | unchanged |
| `ingest` | `json!({runs, records})` or `json!({runs, created, existing, skipped})` | none: a hand shape R16 did not list, not built | unchanged |
| `evaluate` | the job's report file as read | none: its shape is the report file's, not built | unchanged |
| `analyze` | JSON (`json!`) or CSV | `analysis_record`'s, L2 | unchanged here; keeps its own `--format json\|csv` |

`--error-format` covered: the refusal of a verb that ran (`CliError`, written by `refusal::render`) and the clap usage errors raised before the parse (`usage_failure`, which read `--error-format json` from the words of the call). Both are kept: `--format text` is read from the words of the call for both paths.

Stop points (step 1.2): `show` counts, `ingest` and `evaluate` print data documents in hand shapes R16 did not list. They received the option only where they already had one (`show`), or the stderr-only option (`ingest`, `evaluate`, `run`, `kill`, which take `--format text` so their refusals can be asked for as text). The coordinator decides whether to build their records.

N8: probe red on the tip (the printed `store.root` named the default as its source and the default path, not `CANDLER_STORE`). It became a guard (`resolve_names_the_store_source`) and `fix(cli): name the explicit store root in what resolve prints` makes it pass: `resolve` adds the explicit root as an explicit argument, as `store_root` does.

## Guards changed

| Guard | Change | Reason |
|:--|:--|:--|
| `guard_cli_observe` (`o1_resolve...`, `o2_ls...`, `o3_ps...`, `o3_watch...`, `o3_live...`) | pass `--format text` where they read rows; `watch --json` pair reduced to the default; the `--live --json` conflict assertion removed | the default flipped; `--json` no longer exists |
| `guard_cli_surface` (`row_resolve`, `row_ps`, `u5_*`, `u6_*`) | `--format json` dropped (JSON is the default); text readers pass `--format text`; the `--error-format json` prefix dropped | R16 §Guards that change |
| `guard_cli_refusal::r2_the_text_form_is_unchanged_without_the_flag` | reads the text form with `--format text` only (the default is JSON, covered by `refusals_print_json_by_default`) | Q2 |
| `guard_cli` `diff_*` key sets | `fixtures/cli/diff_keys.json` gains `schema`; a call with no equivalence file has no `equivalence` key | compatibility finding: the diff report is now a record (marker added; the null `equivalence` is omitted because the schema has no null) |
| `guard_cli_run::t3_cli_equal_levels_are_refused` | matches the escaped form of the quoted `source` inside the JSON message | stderr is JSON by default |
| `guard_ingest`, `guard_cli_diff_refusal`, `guard_cli_evaluate`, `guard_registry`, `guard_analyze`, `guard_golden_parity`, `i8_e2e` | `--error-format json` and `--format json` dropped; text expectations pass `--format text` | same |
| `guard_refusal` (schemas) | doc comment only | names the new option |

No assertion was weakened: each reads the same fact from the same stream.

## Bites

Each bite was made, seen red and reverted (`git status` clean after each).

| Bite | Red on |
|:--|:--|
| `ps` prints `jobs <n>` from a hand-built `writeln!` instead of its record | `every_data_verb_prints_its_record` (the `ps` row) |
| stderr is text by default (`render` branch inverted) | `refusals_print_json_by_default` |
| `resolve` ignores the explicit store root (the N8 fix removed) | `resolve_names_the_store_source` |
| a key dropped from a record but kept in `text` | not made: `ps` and `ls` text come from the core row, and no guard compares text facts with the record (Follow-ups) |
| the printed new-run count off by the reused ones | not made: C1's summary is not built |

## D3

| Test | Role | Question | Why existing tests miss it | Cost |
|:--|:--|:--|:--|:--|
| `every_data_verb_prints_its_record` | guard | does a verb drift back to a hand-built shape? | U1, U2 check which verbs and options exist, not what they print | one run per row over a seeded store and one job |
| `refusals_print_json_by_default` | guard | is stderr JSON by default and text only with `--format text`, usage errors included? | the old guards passed the retired flag | two calls, two forms each |
| `resolve_names_the_store_source` | guard (was a probe, red on the tip) | does the printed `store.root` name the explicit source? | no test read the source column of `store.root` | one fake-docker `resolve` |

`resolve_counts_equal_the_plan` is not written; see Follow-ups.

## Migration

For the HTN lane's coordinator; ships in the release after the supervisor's PR merges.

| Verb | Before | After |
|:--|:--|:--|
| `ps`, `ls` | TSV | JSON (`job-list/1`, `experiment-list/1`); `--format text` for TSV |
| `resolve` | TSV; `--json` for JSON | JSON; `--format text` for TSV; `--json` removed |
| `watch` | JSON events; `--json` accepted | JSON events; `--json` removed; `--live` or `--format text` draws progress |
| `inspect` | JSON | JSON with a `schema` key; `--format text` lines |
| `show`, `diff` | required `--format json\|toml`, `json\|text` | `--format` optional: JSON default; `show` keeps `toml`; `--format text` for lines |
| stderr (every verb) | `candler: <message>`; `--error-format json` for `refusal/1` | `refusal/1` JSON; `--format text` for the old text; `--error-format` removed with no alias |

## Verifier

One pass (no re-check; call cap). Dispositions:

| # | Finding | Disposition |
|:--|:--|:--|
| 1 | C1 not built | open, coordinator (Follow-ups) |
| 2 | `analyze` keeps `json` value, has no `text`, so `analyze --format text` fails to parse | open, `analysis_record`'s; R01 row says "every verb" |
| 3 | `schema/README.md:39` still names `--error-format` | open, not owned |
| 4 | stale `--json` prose in `guard_cli_observe` | fixed |
| 5 | hand shapes (`resolve` failures, `show` counts, `ingest`, `evaluate`) | open, coordinator (stop point) |
| 6 | `--format` is stderr-only on `run`, `kill`, `ingest`, `evaluate` | doc says so now |
| 7 | `ps`/`ls` text from the core row; no text-vs-record guard | open (Follow-ups) |
| 8 | `ls` prints `store: ""` when the root cannot be resolved | open, follow-up |
| 9 | history wording in comments | fixed |
| 10 | step 3 commit not green alone | accepted |

## Follow-ups

- C1 not built: `resolve` does not print `PlanOutcome::summary()`. `resolve` stops before planning and opens no store; printing the counts needs `run`'s input declaration, per-level resolution and a `PlanMode::Check` pass inside `resolve`, which is a design call (joining two operations, seam 5) and a second planning pass is an attack of the verifier brief. `resolve_counts_equal_the_plan` is left for the coordinator's ruling.
- `ps` and `ls` text renderers take the core row (`Row`, `Listing`), the source the record is built from, not the record itself; `diff` and `inspect` render from the record. A guard that text carries no fact its record lacks is not written.
- `watch`: `--live` and `--format text` now mean the same; one of them can retire.
- `analyze` keeps `--format json|csv` (its `text` and the record are `analysis_record`'s).
- `show` counts, `ingest`, `evaluate` hand shapes (see First reads).
- Gate 3 (`git grep ... --error-format`) still hits `crates/candler-schemas/schema/README.md:39`, which the OWNED regex does not include; left unedited.
- The step 3 commit is not green alone: `guard_cli_surface.rs` carries both changes and lands with the step 4 commit.
