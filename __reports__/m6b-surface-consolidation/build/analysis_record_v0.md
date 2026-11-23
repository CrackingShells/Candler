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
