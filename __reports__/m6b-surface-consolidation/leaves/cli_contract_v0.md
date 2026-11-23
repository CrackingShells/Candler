# CLI contract verdicts (leaf cli_contract, round 0)

Scope: D1 (one output contract) and ten verdict records. Code read through colgrep at the tip `a0df4d48`; all paths are relative to the repo root. Nothing here builds anything.

## Outputs today

| Verb | Formats | Default | Shape owner | Schema | Drift test |
|:--|:--|:--|:--|:--|:--|
| `run` | none (`--set`, `--force` only; prints the job id) | text | CLI crate (not examined beyond its args, `crates/candler/src/commands/run.rs:72-85`) | none | none |
| `ps` | none; TSV lines | TSV | `Row` and `println!` in the CLI; `what` is whitespace-squashed with `_` so the TSV stays parseable (`ps.rs:283-296`) | none (`job-record/1` is the stored record, not this output) | none |
| `kill` | none | text | CLI crate (args only examined, `kill.rs:27-41`) | none | none |
| `watch` | `--json` or `--live` | JSON event lines (`watch.rs:24-37`) | the job's event records; `event-envelope.schema.json` exists, whether `watch` prints that type was not examined | `event-envelope` (probably) | `schema_drift` on the envelope |
| `ingest` | none | text | CLI crate (`ingest.rs:25-36`) | none | none |
| `evaluate` | none; prints the report | text | CLI crate (`evaluate.rs:51-63`, `298-400`, doc says it prints the report) | none | none |
| `show` | `--format json\|toml`, required (`show.rs:11-36`) | none | not examined beyond its args; `outcome-record.schema.json` exists | outcome rows: probably `outcome-record/1`; `--rates`: none | outcome record only |
| `diff` | `--format json\|text`, required (`diff.rs:10-37`) | none | CLI crate (`render_text`, `diff.rs:84-113`) | none | none |
| `resolve` | `--json` flag (`resolve.rs:39-52`) | text lines with sources (`resolve.rs:288-298`) | `Resolution`, `Serialize` in `candler-core` (`crates/candler-core/src/resolve/layers.rs:246-259`) | none in `crates/candler-schemas/schema/` | none |
| `ls` | none; TSV | TSV | `Listing` in the CLI (`ls.rs:63-82`) | none | none |
| `analyze` | `--format json\|csv`, `--table arms\|comparisons\|summary\|runs` (`analyze.rs:51-75`) | JSON | hand-built `json!` in the CLI: `arm_json` (`analyze.rs:162-213`), `entry_json` (`258-289`), `comparison_json` (`291-304`), `run_json` (`306-315`), `render_json` (`329-357`); the core `Report` is not `Serialize` (`crates/candler-core/src/analysis/report.rs:466-485`) | none | none |
| `inspect` | none; always JSON | JSON | a `serde_json::Value` built in `render_run` (`inspect.rs:205-275`) and `render_job` (`311-337`), printed at `339-351` | none | none |
| `--error-format` (stderr) | `text\|json` | text (`main.rs:34-55`) | `refusal/1` record in `candler-schemas` (`crates/candler-schemas/src/refusal.rs:488-502`), rendered at `crates/candler/src/commands/refusal.rs:492-502` | `refusal-record` | `schema_drift` |

Facts the table rests on. `candler-schemas` holds 19 committed schema files, among them `experiment`, `job-record`, `outcome-record`, `run-manifest`, `event-envelope` and `refusal-record`; none is a CLI output other than the refusal and, probably, the event envelope and outcome rows. The drift check blesses and verifies the whole directory (`crates/candler-schemas/src/generate.rs:488-518`; regeneration command at `docs/contract/index.md:95-97`), so a new record costs no new drift test code. Every option's help must end with `cannot be derived: ` or `not an experiment setting: ` (guard U2, `crates/candler/tests/it/guard_cli_surface.rs:136-171`), which is R01's "every new option must say why" made executable. Exit codes: `Exit::Success` is 0, `Exit::Changed` (`diff`) is 1 (`crates/candler/src/commands/mod.rs:27-39`); a refusal is a `CliError`; `resolve` says its preflight failure exits 2 (`resolve.rs:223-229`).

Corrections to the inputs: the README and this leaf's "Csv and Table" is wrong, `Table` is the selector of `--format csv`, not a third format (`analyze.rs:61-75`); X1's "`resolve` prints nothing on success" is wrong, it prints every value with its source and has `--json` (`resolve.rs:288-298`).

## D1

**The two questions.** (1) Should every verb take one format option, `text` a rendering of the same JSON data? (2) If JSON becomes the default, should `candler-schemas` hold the ground truth?

**What the tip shows.** Defaults are already split three ways. JSON by default: `watch`, `analyze`, `inspect`. Text by default: `resolve` (JSON opt-in), `ps` and `ls` (TSV, no JSON at all). No default: `show` and `diff` (a required `--format` whose values differ: `json|toml`, `json|text`). So a consumer must learn each verb. The only shared, schema-backed output is the refusal on stderr. `ps` squashes whitespace into `_` to keep its TSV splittable (`ps.rs:283-296`): the text format is already being used as a machine format, which is the symptom a JSON option removes.

### Options

| Option | What it is | Gains | Costs |
|:--|:--|:--|:--|
| O1 status quo, fill holes | JSON added to `ps`/`ls` only; schemas for `analyze` only | smallest | stays unpredictable: three defaults, three flag spellings (`--json`, `--format`, none) |
| O2 uniform option, today's defaults | every data verb takes `--format json\|text`; each keeps its present default | one spelling; no behaviour change for current readers | the default still depends on the verb, so "predictable" is only half true |
| **O3 uniform option, JSON default** | every data verb takes `--format json\|text`, default `json` everywhere | one spelling and one default; `show`/`diff` need no required option (X10 dissolves); matches the three verbs that are already JSON | a person typing `candler ps` gets JSON; humans type `--format text` |
| O4 uniform option, text default | as O3, default `text` | friendlier at a terminal | consumers must always pass the option, as they must pass `--error-format json` today; the default then favours the minority reader |

Recommendation: O3. Reason: stdout is data and the interactive surfaces' reader is a program (R04's ruling on the interactive surfaces), while stderr is a message to a person. That asymmetry is why `--error-format` defaults to `text` and `--format` should default to `json`. No external users exist, so flipping `resolve`, `ps` and `ls` is a compatibility finding that goes to the supervising session, not a blocker.

### What the option is, and what it replaces

- One shared `OutputArgs { format }` flattened into each data verb's `Args`, with a per-verb value set so clap rejects an unsupported value at parse time (a usage record, `refusal.rs:474-487`, no new `codes.toml` row). `json` and `text` are in every verb; verb-only extras stay verb-only: `csv` (with `--table`) on `analyze`, and `show`'s `toml` (kept as `show`'s readable rendering; to settle at build).
- `watch --json` becomes `--format json` and stays the default; `--live` stays a mode that draws progress (it is not a rendering of a record).
- `resolve --json` becomes `--format json`. `ps` and `ls` TSV become their `text` rendering. `analyze --format json|csv` gains `text`. `diff`'s `text` is already one. `inspect` gains `text`; JSON is its default today. `run`, `kill`, `ingest`, `evaluate` print a job id, a report or nothing: left out of the option in a first build (an `evaluate` report would join when its report is a record).
- **R01 idiom ("every new option must say why it cannot be derived or live in the experiment file").** Why not derived: what parses stdout is invisible from a terminal, the same reason R01 gives for `--error-format` (`main.rs:34-47` help text). Why not in the experiment file: `ps`, `ls`, `show`, `diff`, `analyze` read no experiment file, and the format is how output is read, not a setting of a cell. The guard U2 stamp (`not an experiment setting: it is how the output is read.`, already the text on `show` and `diff`) satisfies the idiom in the code.
- **Default versus "never derived from a terminal".** A fixed default (`json`) is not derivation. Auto-switching to text on a TTY would break the same rule R01 states for stderr, so D1 forbids it. `NO_COLOR`-style colour detection stays what it is: colour only decorates text output (`Paint::detect`, `resolve.rs:293`).
- **`--error-format` stays separate.** A refusal can come before any file is read, and the refusal record is already the one versioned output. Open choice for Eliott: should an explicit `--format json` also set the error format unless `--error-format` is given? That is deriving one option from another the caller typed, not from a terminal, so it does not break R01; it makes the `stdout json, stderr text` default pair disappear for agents. Not needed in the first build.

### Ground truth in `candler-schemas`

Yes for every JSON output that a consumer parses, no for the computation behind it. The pattern already exists: the refusal is a typed record in `candler-schemas` (`refusal.rs:488-502`) rendered by the CLI (`commands/refusal.rs:492-502`), and the core `Report` is a different, richer type with no `Serialize` (`report.rs:466-485`). So the split is: `candler-core` computes and returns domain types; `candler-schemas` holds the wire record, derives `JsonSchema`, and carries the `name/N` marker; a conversion at the core boundary builds the record; the CLI only renders it.

| Output | Moves into `candler-schemas` | Stays in the operation API or CLI |
|:--|:--|:--|
| `analyze` report | yes: `analysis-report/1` (arms, entries, comparisons, runs, summary: today's `arm_json`, `entry_json`, `comparison_json`, `run_json`) | the statistics and `Report` in core; CSV tables render from the record |
| `inspect` run and job documents | yes: `inspect-run/1`, `inspect-job/1`, composed of the existing `outcome-record` and `run-manifest` shapes | the store reads |
| `ps` and `ls` | yes, small: a job summary list and an experiment listing; each includes the `store` root used (this answers part of G9's trap without a refusal) | the recovery logic in `ps.rs` |
| `diff` report, `show --rates` | yes, small | the comparison logic |
| `resolve` | later: `Resolution` is already `Serialize` in core (`layers.rs:246-259`); a wire record is worth it only if M7 or a consumer needs it. Deferred inside D1, to be judged at build | resolver layering |
| `show` outcome rows, `watch` events | already schema-backed (`outcome-record`, `event-envelope`); check at build that the CLI prints exactly those types | nothing |
| `Exit`, `CliError`, `Paint`, `--live` drawing | no | CLI crate |

**Blast radius.** Each new record is an additive file under `crates/candler-schemas/schema/`, covered by the existing drift check with no new test code. The existing records (`refusal/1`, `job-record/1`, `experiment/1`, `outcome-record`, `event-envelope`) do not change. By the idiom "schema changes are scoped `schemas` and touch no other crate", the records land as `schemas` commits and the CLI/core changes as separate commits. Cost in releases: one `schemas@` minor release if the records of one phase land together (suggested phases: `analysis-report/1` and `inspect-*` first, the small list/diff/rates records second), so one to two releases. A later change to a field is a `/2` marker and a release, which is the real price: F2 and F4 (`analysis_outputs`) change what `analysis-report` carries, so the record should be cut after those verdicts are built, not before. No digests are at risk (CLI outputs are not hashed; cell and run digests come from manifests). No `codes.toml` row is needed if clap enforces the value sets.

**Text stays a rendering, never a second source.** The rule: a renderer's only input is the record (`fn text(&Record) -> String`), so text cannot show a fact JSON lacks, and CSV is the same (the CSV tables read the record, not `Report`). `ps`'s `_` squashing disappears: text may be lossy because JSON is the lossless form. The enforcement is the type signature plus one guard (see X1's design suggestion), not a test per renderer.

**How CLI and M7's MCP share the types, without deciding M7.** The record types live in `candler-schemas`, the producer in the operation API; the CLI renders them. M7 may then serialise the same record as a tool result, wrap it, or choose another envelope: that decision stays at M7 planning. D1 fixes only that no output shape is defined in a surface crate, which is also what R05's "same results on both surfaces" gate needs. It does not decide tool names, resources or text rendering for the MCP.

**Questions for Eliott.** (a) O3 versus O4 (default `json` versus `text`). (b) Whether `--format json` should also set `--error-format json` unless overridden. (c) `show`'s `toml`: keep as its `text`, or drop. (d) Whether `resolve`'s JSON record joins phase one.

## Verdicts

### X1: JSON output for every verb
- **Verdict:** accept
- **Covers:** none
- **Live on the tip:** yes, in part: `ps` and `ls` print TSV only (`ps.rs:283-296`, `ls.rs:63-82`); but `resolve` prints values and has `--json` (`resolve.rs:39-52`, `288-298`), and `analyze`, `inspect`, `watch` are JSON already (`analyze.rs:51-59`, `inspect.rs:339-351`, `watch.rs:24-37`), so the premise "resolve prints nothing" is wrong.
- **The ask questioned:** the real problem is not "JSON everywhere" but three defaults and three flag spellings. Verbs with no data (`kill`, `ingest`, `run` printing an id) do not need it.
- **Responsibility:** Candler's CLI surface, as a thin renderer over the operation API (R01 Layer responsibilities); per D1.
- **Blast radius:** `crates/candler` (every data verb's `Args`), a shared `OutputArgs`; schemas per D1; no digest; no `codes.toml` row; guards U1, U2 touched.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | 0: one shared type to maintain | + one spelling replaces three | + one rule to learn |
  | scalable | + a new verb copies one pattern | + no per-verb format enum | + agents parse every verb alike |
  | efficient | 0: about 85 calls across phases | 0: a renderer per verb already exists for four | + no more TSV scraping |
- **Compromise:** flipping `resolve`, `ps`, `ls` defaults to JSON costs human convenience; consumption wins by D1's reasoning, and it is reversible by one default.
- **Idioms:** follows "every new option must say why" and U2; follows "never derived from a terminal"; follows thin surfaces. Breaks nothing; flags a compatibility finding to the supervisor.
- **Reason or owner:** the build is D1 (O3), phase by verb group.
- **Design suggestion:** files `crates/candler/src/commands/` (`mod.rs` for `OutputArgs`, each data verb) and the records of X4. Tests per D3: one guard in `crates/candler/tests/it/guard_cli_surface.rs`, a table over the U1 verb matrix: each data verb run with `--format json` prints a document that parses as the verb's record; role guard; question: does a verb drift back to a hand-built `println!` shape; no existing test answers it because U1 and U2 check which verbs and options exist, not what they print; sustainable and scalable because it is table-driven, one row per verb, and `schema_drift` already keeps the schema files honest. No test per renderer. Size: about 45 Sonnet calls for the option and the `ps`/`ls`/`resolve` verbs.

### X4: output schemas for analyze, show and inspect
- **Verdict:** accept
- **Covers:** none
- **Live on the tip:** yes for `analyze` and `inspect` (hand-built, `analyze.rs:162-357`, `inspect.rs:205-337`; no schema file among the 19 in `crates/candler-schemas/schema/`); `show`'s outcome rows probably already use `outcome-record`, only `--rates` is unschematised (not verified: `show.rs:38-110` not read in full).
- **The ask questioned:** R01 files "schemas for CLI outputs" under Future features (`__design__/architecture.md:1804`) with the trigger "the next change that touches one of them"; F5, F2, F4, X1 and M7's consumers all touch them now, so the trigger is met. The real problem is two shapes (core `Report`, hand-built JSON) with no record between them.
- **Responsibility:** `candler-schemas` for the wire record; core for the conversion; CLI renders (D1).
- **Blast radius:** `crates/candler-schemas` (new files, `schemas@` minor, one to two releases), `crates/candler-core/src/analysis`, `crates/candler/src/commands/analyze.rs` and `inspect.rs`; no digest; no `codes.toml` row; `schema_drift` blesses the new files.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | 0: a field change needs a version | + one source of shape | + a contract an agent can validate |
  | scalable | + MCP and CLI reuse it | + removes `json!` growth | + stable across surfaces |
  | efficient | - about 40 calls | 0 | 0 |
- **Compromise:** cut `analysis-report/1` only after `analysis_outputs` settles F2 and F4, since both change its fields; `inspect-*` and the small records can go first.
- **Idioms:** follows facts under drift tests, schema changes scoped `schemas`; follows "a refusal/record is the one source" pattern of `refusal/1`.
- **Reason or owner:** per D1.
- **Design suggestion:** files `crates/candler-schemas/src/` (new `analysis.rs`, `inspect.rs`, registered in `generate.rs`), core conversion in `analysis/`, CLI swaps `json!` for the record. Tests per D3: no new test code, since the committed schema files fall under the existing `schema_drift` check (role confirm, already present: it answers "does the Rust type match the committed schema"); the X1 guard covers the output. Size: about 40 Sonnet calls.

### X10: show and diff require --format and have no default
- **Verdict:** accept
- **Covers:** none
- **Live on the tip:** yes (`show.rs:11-27`, `diff.rs:10-28`: `format: ShowFormat` and `DiffFormat` without a default).
- **The ask questioned:** the requirement forces a choice a consumer should not have to make, and the value sets differ (`json|toml`, `json|text`). It is dissolved by D1, not a separate fix.
- **Responsibility:** the CLI surface, per D1.
- **Blast radius:** `show.rs`, `diff.rs`; guard U2 text; no schema or `codes.toml` change.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | 0 | + one format type | + fewer required options |
  | scalable | 0 | + | + |
  | efficient | + small, part of X1 | 0 | + one call fewer to learn |
- **Compromise:** none; differs from X1 only in that it is two files.
- **Idioms:** follows "values are derived, never asked for" (a default replaces a question).
- **Reason or owner:** per D1 (default `json`, `toml` kept for `show` as an extra).
- **Design suggestion:** files `show.rs`, `diff.rs` inside the X1 build. Tests per D3: none new; the X1 guard row for each verb covers it. Size: 4 Sonnet calls inside X1.

### G11: output shape for agents (stderr text by default, JSON only with --error-format json)
- **Verdict:** done
- **Covers:** none
- **Live on the tip:** yes, and as designed: `--error-format` is global, defaults to text, and its help says why it cannot be derived (`main.rs:34-55`); the refusal is a versioned record (`refusal.rs:488-502`); exit codes are 0, 1 (`diff` changes, `mod.rs:27-39`) and 2.
- **The ask questioned:** the "trap" is a deliberate rule in R01 §CLI surface (deriving it from a terminal would hand JSON to a person who pipes). What is left is one sentence for the skills, which is `docs_and_skills`' work, and D1's open question (b) on whether `--format json` implies it.
- **Responsibility:** Candler's CLI, already discharged; the advice "always pass `--error-format json`" is the consumer's or the skill's.
- **Blast radius:** none.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | + nothing to maintain | + one record | 0: a consumer must pass the flag |
  | scalable | + | + | + |
  | efficient | + | + | 0 |
- **Compromise:** none.
- **Idioms:** follows "`--error-format` is never derived from a terminal".
- **Reason or owner:** already true; see D1 for its relation to `--format`.
- **Design suggestion:** none.

### F5: analyze writes JSON only
- **Verdict:** accept
- **Covers:** H5
- **Live on the tip:** partly: `analyze` writes JSON or CSV with four tables (`analyze.rs:51-75`, `112-115`), so "JSON only" is wrong; it has no text rendering for a person (no `text` value in `Format`).
- **The ask questioned:** the problem is a human cannot read the comparisons without a tool; CSV is not that. Per D1 this is `--format text` on `analyze`, rendered from `analysis-report/1`, not a separate report.
- **Responsibility:** the CLI renders; `analysis_outputs` owns what the comparisons say (F2, F4, C6), so H5 is split with it: this record gives the format, theirs the content.
- **Blast radius:** `analyze.rs` (a renderer over the record); depends on X4; no `codes.toml` row.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | 0 | 0: one more renderer | + readable |
  | scalable | 0 | + shares the record | + |
  | efficient | + about 10 calls | 0 | + |
- **Compromise:** build after X4 and after F2 and F4, or the text is written twice.
- **Idioms:** follows "text is a rendering of the record" (D1); follows thin surfaces.
- **Reason or owner:** per D1.
- **Design suggestion:** files `crates/candler/src/commands/analyze.rs` (a `text(&AnalysisReport)` renderer). Tests per D3: none new; the X1 guard row for `analyze` covers `--format text` exits 0 and the record test covers the data; the rendering is not asserted line by line (low-value brittle test). Size: about 10 Sonnet calls.

### G9: store root fallback (a typo'd project dir quietly uses the default store)
- **Verdict:** done
- **Covers:** none
- **Live on the tip:** yes, and designed: `store_root` takes `--store` or `CANDLER_STORE`, else `store.root`, else the core default, "as the resolver layers them" (`crates/candler/src/commands/common.rs:106-116`), reading the root from the `Resolution` (`common.rs:89-104`), which records each value's source (I12). `config.no_store` is never raised, and the registry says so (`commands/refusal.rs:233-241`; `docs/contract/index.md:70`): the R06 claim is right.
- **The ask questioned:** is the fallback a trap? A refusal would break the zero-configuration run, which the default store exists for; the fallback is sourced, so `candler resolve` shows where the root came from. The real defect is in R01: §CLI surface says "a verb that finds none refuses, naming the three ways" (`__design__/architecture.md:333-335`), which the code does not do. That is a docs fix for `docs_and_skills`/X9, not a build.
- **Responsibility:** the resolver and CLI, already discharged.
- **Blast radius:** none for a verdict of `done`; D1's `ps`/`ls` records carry the store root used, so a consumer sees which store answered.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | + | + | 0: another project's jobs can be listed |
  | scalable | + | + | 0 |
  | efficient | + | + | + zero configuration |
- **Compromise:** consumption pays a small trap for zero-config; D1's `store` field in the records removes the silent part.
- **Idioms:** follows "each value records its source (I12)" and "values are derived".
- **Reason or owner:** the design is true on the tip; correct R01's sentence.
- **Design suggestion:** none.

### C11: remedies (valid values and the nearest match in a refusal)
- **Verdict:** reject
- **Covers:** none
- **Live on the tip:** partly: a refusal carries a message and `causes`; some messages already state valid values as facts, for example the unknown-request-key refusal lists the known keys (`crates/candler/src/commands/refusal.rs:192-199`, `249-250`).
- **The ask questioned:** two things are bundled. A remedy ("pass `--force`") is advice and goes stale; Eliott ruled no hints by default. Valid values are facts, and where a refusal already states them it needs no work. A systematic "valid values" field would change `refusal/1` (a `schemas` release for a record every consumer parses) for facts that belong in the message.
- **Responsibility:** nobody systematically; each refusal's author may state a fact in its message when the code is touched, which `codes_review` can note per code.
- **Blast radius:** a field would touch `candler-schemas` `refusal/1`, `codes.toml` and every producer; rejected so none.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | - values go stale | - 377 rows to keep | + one fix less to guess |
  | scalable | - per code | - | 0 |
  | efficient | - | - | 0 |
- **Compromise:** consumption's gain does not pay for a record change; facts stay in messages, advice stays out.
- **Idioms:** breaks "a refusal is a code, a side and a path" if a field is added; follows the no-remedy ruling (R04).
- **Reason or owner:** advice is ruled out; facts already live in messages where useful.
- **Design suggestion:** none.

### C13: plugin check
- **Verdict:** defer
- **Covers:** G7
- **Live on the tip:** the check exists in core and spawns nothing: `Registry::check_suite` verifies the manifest/`describe` agreement, the declared kind, the params schema and the needs (`crates/candler-core/src/evaluation/registry/suite.rs:39-80`); the `describe` call is `Client::describe` (`crates/candler-core/src/evaluation/client/mod.rs:214-248`). No verb exposes it without starting a job; `resolve` takes an experiment file only (`resolve.rs:39-52`), though R01 line 326 says `<experiment | suite>`. Lock pin and ancestry checks were not found.
- **The ask questioned:** G7's traps are mostly the plug-in author's (a `uv run` wrapper, `print` on stdout, a bad return value becoming `not_measured` with `instrument_error`) and are documented on the contract pages (`docs/contract/evaluator-helper.md:103`). A static check catches only the protocol side and cannot catch the runtime ones, so a verb named `plugin check` would suggest more safety than it gives.
- **Responsibility:** Candler for the protocol checks (already in core); the plug-in author for the rest.
- **Blast radius:** a new verb touches `crates/candler`, the operation API and the contract pages; `codes.toml` rows none new (refusals exist).
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | 0 | 0: a second entry to the same check | 0 |
  | scalable | 0 | 0 | + authors find faults before a job |
  | efficient | 0 | 0 | 0: `evaluate` already refuses a bad suite before grading, if it runs the check first (not verified) |
- **Compromise:** none; waits.
- **Idioms:** follows one operation API and "built only when its evidence trigger is met".
- **Reason or owner:** see the Future row.
- **Future row:** | Check a plug-in and a suite without a job | Exposes `check_suite` and `describe` as one operation | Fewer wasted job starts for plug-in authors | A plug-in author (or the HTN lane) loses a job start to a fault `check_suite` would have reported, shown in a report |

### C14: rules inference for ingest
- **Verdict:** reject
- **Covers:** G12
- **Live on the tip:** yes, the rules are an explicit input: `ImportRules` holds the harness, treatment, instrument, snapshot pointers, markers, suite kind and entry params (`crates/candler-core/src/import/rules.rs:93-117`); a folder is recognised only by `PROVENANCE.json` and `expectations.jsonl` at its root (`crates/candler-core/src/import/raw.rs:362-370`); a rules file that does not parse is refused (`crates/candler/src/commands/ingest.rs:108-177`).
- **The ask questioned:** inference would guess the domain strings of a legacy folder (which pointer is the treatment, which instrument graded it). The rules exist so the core names no domain (rules.rs doc, I1). A wrong guess would label cells wrongly and the store is append-only (I11), so it fails silently and stays.
- **Responsibility:** whoever owns the legacy results writes the rules; the importer checks them.
- **Blast radius:** none (rejected); inference would touch core `import/` and add domain knowledge to it.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | - | - domain in core | - silent wrong data |
  | scalable | - per legacy format | - | 0 |
  | efficient | - | 0 | + one file less to write |
- **Compromise:** consumption's saved file does not pay for silent wrong cells.
- **Idioms:** breaks I1 (no domain in core), I11 and AGENTS.md's fail list (code that can be wrong silently).
- **Reason or owner:** the rules are the author's input by design; a worked sample of a rules file belongs to `docs_and_skills` (X6).
- **Design suggestion:** none.

### X3: CLI help with value domains, examples and exit codes
- **Verdict:** accept
- **Covers:** none
- **Live on the tip:** partly: option help already ends with a stamp and is walked by a guard (`guard_cli_surface.rs:136-171`); value domains of a `ValueEnum` are printed by clap itself, with the variant docs (`show.rs:29-36`); no `after_help` or example text was found and no page states the exit codes (0, 1 for `diff` changes, 2: `mod.rs:27-39`, `resolve.rs:223-229`).
- **The ask questioned:** value domains are done. Examples would be hand-written prose in a second place that rots, and they belong in the docs. Exit codes are a fact of the binary, and since the CLI is self-contained and offline they must be in `--help` itself.
- **Responsibility:** the CLI, generated from one source.
- **Blast radius:** `crates/candler/src/main.rs` (`after_help`) and one contract-page generator row; no schema, no `codes.toml`.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | 0 | + one constant feeds help and page | + exit codes without a page |
  | scalable | 0 | + | + |
  | efficient | + about 6 calls | 0 | + |
- **Compromise:** examples are rejected for the second-source risk; exit codes and value domains only.
- **Idioms:** follows "facts for agents are generated under drift tests (`contract_pages`)"; follows the interactive surfaces being self-contained.
- **Reason or owner:** accept the narrow form.
- **Design suggestion:** files `crates/candler/src/main.rs` or `commands/mod.rs` (one `EXIT_CODES` table beside `Exit`), the contract-page generator. Tests per D3: none new, because the page row comes from the same constant and the existing `contract_pages` drift check covers it (role confirm, exists). Size: about 6 Sonnet calls.

## Seams

- D1 with `analysis_outputs`: `analysis-report/1` carries what F2, F4 and C6 decide (orientation, reading after Holm, the summary); cut the record after those are settled. F5's text rendering and H5 split along that line; C9 and C10's flags also belong in the record.
- D1 with `knowledge_as_data` (C3): a capability reference could use the same record-in-schemas pattern; no decision here.
- D1 and the error shape with `codes_review`: no field is added to `refusal/1` (C11 rejected); G11 stays `--error-format`. `codes_review` owns the `config.no_store` row, which is honest and not raised, against R01's sentence.
- G9 with `docs_and_skills` and X9: R01 §CLI surface (`architecture.md:333-335`) promises a refusal the code does not raise; the same sentence on `resolve <experiment | suite>` (line 326) does not match `resolve`'s arguments.
- X3's exit-code table with `docs_and_skills` (X9, X12): the contract-page row is generated, so the page owner should not hand-write it.
- C13 with `docs_and_skills`: the author traps (G7) are page content; the evaluator protocol's own checks are core.
- D1 with M7: the record types are shared; the MCP's envelope, tool names and rendering are decided at M7 planning.

## Spec defects found
- README D1 and this leaf say `analyze` has "`Csv` and `Table`"; `Table` selects the CSV table (`analyze.rs:61-75`).
- README X1 and this leaf's brief: "`resolve` prints nothing on success" is wrong (`resolve.rs:288-298`).
- R10's F5 "JSON only" is wrong (CSV exists).
- R01 §CLI surface lines 326 and 333-335 do not match the code (`resolve` arguments; the no-store refusal).
- The leaf's Step 1 table has a "Drift test" column but no `show`/`watch` row detail was checkable without reading code in full; those cells say "not examined".
