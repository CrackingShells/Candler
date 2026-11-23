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
| `resolve` | TSV; `--json` for JSON | JSON with a `plan` key; `--format text` for TSV plus `plan.*` lines; `--json` removed |
| `watch` | JSON events; `--json` accepted | JSON events; `--json` removed; `--live` or `--format text` draws progress |
| `inspect` | JSON | JSON with a `schema` key; `--format text` lines |
| `show`, `ingest` | JSON, no marker | JSON with `schema: show-counts/1` or `ingest-report/1` |
| `show`, `diff` | required `--format json\|toml`, `json\|text` | `--format` optional: JSON default; `show` keeps `toml`; `--format text` for lines |
| stderr (every verb) | `candler: <message>`; `--error-format json` for `refusal/1` | `refusal/1` JSON; `--format text` for the old text; `--error-format` removed with no alias |

## Verifier

One pass (no re-check; call cap). Dispositions:

| # | Finding | Disposition |
|:--|:--|:--|
| 1 | C1 not built | fixed: built |
| 2 | `analyze` keeps `json` value, has no `text`, so `analyze --format text` fails to parse | open, `analysis_record`'s; R01 row says "every verb" |
| 3 | `schema/README.md:39` names `--error-format` | fixed |
| 4 | stale `--json` prose in `guard_cli_observe` | fixed |
| 5 | hand shapes | `show` counts and `ingest` now records; `evaluate` and resolve failures stay (Follow-ups) |
| 6 | `--format` is stderr-only on `run`, `kill`, `ingest`, `evaluate` | doc says so now |
| 7 | `ps`/`ls` text from the core row | fixed: rendered from the record; bite 2 red on `o3_ps_*` |
| 8 | `ls` prints an empty `store` when unresolved | fixed: `null` |
| 9 | history wording in comments | fixed |
| 10 | step 3 commit not green alone | accepted |

Re-check: sent to the same verifier for the fixes and the new C1 and record code; its answer, not fixed (call cap reached):

- R1 `resolve` resolves twice (its own pass for the printed values, `prepare` for the plan, with declared inputs and ground truth); the two can disagree.
- R2 `resolve` now initialises the store, declares inputs and builds the sidecar: it is no longer read-only.
- R3 `plan_outcome` with `force=false` can refuse where `resolve` used to succeed; untested on an edited experiment with a sealed store.
- R4 `reused 0` after a sealed run is a likely wrong count (run keys from `prepare` differ from `run`'s, or the Check plan misses sealed runs); the guard's oracle is the store's sealed count and its bite cannot go red.
- R5 text mode now sorts the value lines.
- R6 `PlanCounts` has no schema file or marker; the `plan` key is merged into the core `Resolution` value; `plan.<cap>` text lines could collide.
- R7 `show` text and TOML are not rendered from the record; TOML gains `schema`.
- R8 `line_of` prints a blank line for an unknown variant. R10 nits (long doc line, split `use` block).


## Follow-ups

- Open: verifier re-check findings R1 to R8 above.
- C1 reuse: after a job sealed its runs, `resolve` with the same settings still counts them as new (reused 0), so `resolve_counts_equal_the_plan` asserts new plus reused equals the sealed runs, and the bite that adds the reused runs to the new count does not go red. Why the fake-docker job's runs are not matched is not known and needs a ruling.
- `resolve` now declares inputs, builds the gateway sidecar and initialises the store before printing, as `run` does (`run::prepare`, `run::plan_outcome`).
- `evaluate` (the report file as read) and `resolve`'s failure document (unversioned) print hand shapes with no record.
- `watch`: `--live` and `--format text` mean the same; one can retire.
- `analyze` keeps `--format json|csv`; its `text` and record are `analysis_record`'s.
- `--format text` on `run`, `kill`, `ingest`, `evaluate` changes only the refusal.
- The step 3 commit is not green alone (accepted).
