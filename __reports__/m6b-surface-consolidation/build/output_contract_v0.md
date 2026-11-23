# Output contract: build report (v0)

Branch `claude/m6b-build-output_contract`, branch point `e016b320`. C1 (the printed plan summary and `resolve_counts_equal_the_plan`) is out of this leaf by ruling; see Follow-ups.

## First reads

| Verb | Prints today | Record | Default after this leaf |
|:--|:--|:--|:--|
| `ps` | tab-separated rows | new `job-list/1` | JSON; `--format text` is the rows |
| `ls` | tab-separated rows | new `experiment-list/1` (with the `store` root) | JSON; `--format text` is the rows |
| `inspect` run, job | `json!` documents | new `inspect-run/1`, `inspect-job/1` (`spent` is `analysis_core`'s keys) | JSON; `--format text` lines |
| `diff` | core `DiffReport` JSON, or text, required `--format` | new `diff-report/1` | JSON; `--format text` lines |
| `show --rates` | `json!` `{cell: ...}`, or TOML, required `--format` | new `rates/1` | JSON; `text`, `toml` |
| `show` (counts) | `json!({"counts": ...})` | new `show-counts/1` | JSON; `text`, `toml` (TOML carries `schema`) |
| `watch` | `EventEnvelope` lines (already the schema type), `--json` or `--live` | `event-envelope` (existing) | JSON events; `--format text` draws the `--live` progress |
| `resolve` | tab-separated rows, or the core `Resolution` as JSON with `--json` | core `Resolution` (record deferred by R16) | JSON; `--format text` rows |
| `run` | the job id, one line | none (an id, not a document) | unchanged |
| `kill` | text lines | none | unchanged |
| `ingest` | `json!({runs, records})` or `json!({runs, created, existing, skipped})` | new `ingest-report/1` | JSON with `schema` |
| `evaluate` | the job's report file as read | none: its shape is the report file's, not built | unchanged |
| `analyze` | JSON (`json!`) or CSV | `analysis_record`'s, L2 | unchanged here; keeps its own `--format json\|csv` |

`--error-format` covered: the refusal of a verb that ran (`CliError`, written by `refusal::render`) and the clap usage errors raised before the parse (`usage_failure`, which read `--error-format json` from the words of the call). Both are kept: `--format text` is read from the words of the call for both paths.

Stop points (step 1.2): `show` counts and `ingest` printed hand shapes R16 did not list; the coordinator ruled them built as records. `evaluate` stays a hand shape (Follow-ups). `ingest`, `evaluate`, `run` and `kill` take `--format text` so their refusals can be asked for as text.

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

`resolve_counts_equal_the_plan` is out with C1; see Follow-ups.

## Migration

For the HTN lane's coordinator; ships in the release after the supervisor's PR merges.

| Verb | Before | After |
|:--|:--|:--|
| `ps`, `ls` | TSV | JSON (`job-list/1`, `experiment-list/1`); `--format text` for TSV |
| `resolve` | TSV; `--json` for JSON | JSON; `--format text` for TSV; `--json` removed; still read-only, no plan counts |
| `watch` | JSON events; `--json` accepted | JSON events; `--json` removed; `--live` or `--format text` draws progress |
| `inspect` | JSON | JSON with a `schema` key; `--format text` lines |
| `show`, `ingest` | JSON, no marker | JSON with `schema: show-counts/1` or `ingest-report/1` |
| `show`, `diff` | required `--format json\|toml`, `json\|text` | `--format` optional: JSON default; `show` keeps `toml`; `--format text` for lines |
| stderr (every verb) | `candler: <message>`; `--error-format json` for `refusal/1` | `refusal/1` JSON; `--format text` for the old text; `--error-format` removed with no alias |

## Verifier

One pass (no re-check; call cap). Dispositions:

| # | Finding | Disposition |
|:--|:--|:--|
| 1 | C1 not built | withdrawn by ruling (see Follow-ups) |
| 2 | `analyze` keeps `json` value, has no `text`, so `analyze --format text` fails to parse | open, `analysis_record`'s; R01 row says "every verb" |
| 3 | `schema/README.md:39` names `--error-format` | fixed |
| 4 | stale `--json` prose in `guard_cli_observe` | fixed |
| 5 | hand shapes | `show` counts and `ingest` now records; `evaluate` and resolve failures stay (Follow-ups) |
| 6 | `--format` is stderr-only on `run`, `kill`, `ingest`, `evaluate` | doc says so now |
| 7 | `ps`/`ls` text from the core row | fixed: rendered from the record; bite 2 red on `o3_ps_*` |
| 8 | `ls` prints an empty `store` when unresolved | fixed: `null` |
| 9 | history wording in comments | fixed |
| 10 | step 3 commit not green alone | accepted |

### Re-check

First re-check (tip e16e5e2d), on C1 as then built:

| # | Finding | Disposition |
|:--|:--|:--|
| R1 | `resolve` resolved twice | withdrawn with C1 |
| R2 | `resolve` initialised the store, declared inputs, built the sidecar | withdrawn with C1 |
| R3 | planning with `force=false` could refuse in `resolve` | withdrawn with C1 |
| R4 | `reused 0` after a sealed run, guard could not go red | withdrawn with C1; kept as a finding for the C1 re-plan |
| R5 | `plan.*` lines and sorted text | withdrawn with C1 |
| R6 | `lines.sort()` reordered the value lines; it came with C1 | withdrawn with C1 (`resolve` text is as before) |
| R6b | `PlanCounts` had no schema file or marker | withdrawn: the record is removed (it was never a root, so no schema file existed) |
| R7 | `show` text and TOML not rendered from the record | fixed: both render from `show-counts/1`; TOML carries `schema` as JSON does, being the same record in another syntax |
| R8 | `ls` printed a blank line for an unknown variant; long doc line; split `use` block | fixed (the split `use` block went with `resolve.rs`) |

Second re-check (tip 8149ba8d), on the withdrawal and R7, R8: code held; it found this report stale, which this edit corrects. Commit e16e5e2d is a `!` commit whose `resolve` text (the `plan` key and `plan.*` lines) is void: 7e08e01f withdraws it, and `resolve` is as at eed30d8f.

## Follow-ups

- C1 (plan counts) is out of M6b by Eliott's ruling; needs a read-only run-key count in candler-core first; the first attempt reported sealed runs as new (reused 0 after the job sealed them, with the same settings). `resolve_counts_equal_the_plan` and its bite are not delivered; the leaf Goal's C1 clause is open for the coordinator.
- `evaluate` (the report file as read) and `resolve`'s failure document (unversioned) print hand shapes with no record.
- `watch`: `--live` and `--format text` mean the same; one can retire.
- `analyze` keeps `--format json|csv`; its `text` and record are `analysis_record`'s.
- `--format text` on `run`, `kill`, `ingest`, `evaluate` changes only the refusal.
- The step 3 commit is not green alone (accepted).
