# analysis_record build report

## First reads

### The trace

`TraceFile` (`candler-schemas/src/trace.rs`) holds a header, sessions and `SessionEvent`s (a session id and an `Event` with `seq`, optional `at`, `kind`, optional `span`, `raw_ref`, `ext`). It reads and writes one JSON object per line, tagged by `kind` (`header`, `session`, `event`) through `TraceRecord`. `Store::read_trace` reads `layout().trace(key)` and returns `StoreError::Io` when the file is missing. The registry has no refusal for a run without a stored trace (`store.io_failed` is an environment failure of any file), so step 4 appends `config.no_stored_trace`.

### "stratum" and "strata", by sense

| Where | Identifier or text | Sense | Becomes |
|:--|:--|:--|:--|
| `manifest/compare.rs` | `stratum_verdict` and the doc lines "verdict per stratum", "arms of a stratum", "imported stratum" | comparability (the runs sharing one experiment digest) | `class_verdict`, "comparability class" |
| `analysis/report.rs` | `UNKNOWN_STRATUM`, `Strata`, `strata_of`, locals `strata`/`stratum`, doc lines | comparability | `UNKNOWN_CLASS`, `Classes`, `classes_of`, `classes`/`class` |
| `analysis/summary.rs`, `analysis/refusal.rs` | doc lines and the two registry descriptions "stratum" | comparability | "comparability class" |
| `analysis/blocking.rs` | `Stratum` (one block of a test) | block | unchanged |
| `analysis/{cmh,midp,exact_strata,bca,permutation}.rs` | `Stratum`, `strata`, `exact_strata`, `stratified_*` | block | unchanged |
| `commands/analyze.rs` | printed key `stratum` (`{verdict, causes}`) | comparability | `comparability_class` |
| `commands/analyze.rs` | printed battery value `exact_strata` | block | unchanged |
| tests | `stratum_verdict_table`, `imported_stratum_*`, `a1_/a2_ ... strata` | comparability | renamed with the code they call |

### The keys `analyze` printed before this leaf (the record keeps them)

Top: `evaluator`, `alpha {value, source}`, `i4_violations`, `arms`, `comparisons`, `families [{role, m}]`, `pairs_not_one_factor`, `runs`, `summary {decisions, omitted}`.
Arm: `experiment`, `cell`, `runs {valid, unsealed, invalid, experimenter_ends}`, `rates {gate, quality}`, `status {pass, fail, blocked, not_measured, not_applicable}`, `instrument_error {runs, outcomes}`, `expectations [...]`, `spent {cost_usd, runs_counted, runs_without_total}`.
Comparison: `experiment`, `a`, `b`, `factor`, `levels`, `stratum {verdict, causes}`, `entries`.
Tested entry: `id`, `role`, `a {k, n}`, `b {k, n}`, `p`, `p_holm`, `reject`, `diff`, `diff_lo`, `diff_hi`, `battery`, `fallback`, `blocks`, `reading`, `or_mh`, `or_lo`, `or_hi`, `homogeneity_p`, `per_block`. Omitted entry: `id`, `role`, `omitted`.
Run: `run`, `cell`, `integrity`, `comparable`, `measured`.

### Defect in the brief

The brief says `analyze` already takes `OutputArgs`; on the tip it does not (it has its own `Format {Json, Csv}` and `--table`). This leaf makes it take `OutputArgs` (`text` only) and keeps `--format csv` out of `OutputArgs` as a second value of its own.

## Guards changed

| Guard | Change | Reason |
|:--|:--|:--|
| `guard_cli_surface::every_data_verb_prints_its_record` | new `analyze --evaluator ev@1.0.0` row (7 rows) | step 1 guard; the store holds two evaluators, so one is named |
| `guard_transcript::transcript_lists_every_event_in_order` | fixture sessions gained `ended_by`, the tool call `wrapped_calls` | the first fixture was not a valid trace file; the oracle lines are still hand-written |
| `guard_analyze`, `guard_golden_design`, `guard_analyze_golden` | `["stratum"]` to `["comparability_class"]`, `reading` to `interval_reading` (JSON key, CSV header, doc lines); `guard_analyze_golden` calls `analyze` without `--format json` | the renamed keys and the retired `json` value |
| `guard_comparability`, `guard_analysis_report`, `guard_census_gateway`, `guard_analyze` | `stratum_verdict`, `imported_stratum*`, `live_stratum*`, `*_in_two_strata`, `per_stratum` renamed; comparability-sense prose in comments | the identifiers of step 2 |
| `compare.rs` problem text | "the stratum has no arm" now "the comparability class has no arm" | a printed cause string of the comparability sense; no test asserted it |
| `analysis.experiment_disagrees`, `analysis.factor_broader_than_variation` | registry descriptions say "comparability class"; `docs/contract/index.md` regenerated | same sense, doc lines |

## Bites

Not made: the leaf stopped at its call cap before step 5.1. Left: drop one event kind or sort by kind in `render_transcript` (expect red on `transcript_lists_every_event_in_order`); print one key with `json!` beside the record (red on the `analyze` row); rename a block-sense identifier too (nothing in the targeted tests names `Stratum`, so likely nothing turns red). The step 1 guards were seen red on the tip (the core guard did not compile for want of `render_transcript`; the surface row failed on the missing `schema` marker).

## D3

- `transcript_lists_every_event_in_order`: question, does the transcript keep every stored event in order; oracle, the hand-written stored lines; bite, a renderer that skips or reorders; role guard.
- `analyze` row of `every_data_verb_prints_its_record`: question, is the printed document the record; oracle, `analysis-report/1` and its keys; bite, a hand-built document; role guard.

## Verifier

Not dispatched: the leaf reached its 77-call cap. The coordinator dispatches it with the leaf's brief and this branch.

## Follow-ups

- Brief defect: `analyze` did not take `OutputArgs` on the tip; it keeps its own `--format {text, csv}` because `OutputArgs` has no `csv` value.
- No CLI test of `inspect --transcript` (JSON, text, the `analysis.no_stored_trace` refusal for a run without a trace and for a job); the core guard covers the rendering only.
- No test of `--format text` content beyond exit 0 and not-JSON.
- The refusal for a missing trace lives in `ReportError` (`analysis.no_stored_trace`) because `core/src/refusal.rs` and `commands/refusal.rs` are not OWNED; a trace-specific error type may be preferred.
- `crates/candler-schemas/schema/README.md` has no row for `analysis-report.schema.json`; it is not OWNED and no test checks it.
- The record's role, cell, id and factor fields are strings, not the primitive types.
