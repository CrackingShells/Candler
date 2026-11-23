# Candler CLI surface inventory for M7 (MCP server)

Repo: detached at 5199afc7. Sources: colgrep over `crates/candler/src`, `crates/candler-core/src`, plus `docs/contract/*.md` and `codes.toml`. Paths below are relative to the repo root. Line numbers are those the search returned (span starts) and may be off by a few lines inside a span.

Global flags (`main.rs:34-47`): `--store <dir>` (env `CANDLER_STORE`), `--error-format text|json` (default text). Exit codes (`main.rs:1-33`, `commands/mod.rs:27-39`): 0 success, 1 = `diff` found changes, 2 = any error.

## 1. Operation table

Verbs are enumerated in `main.rs:57-88`. Output column: what stdout carries; refusals go to stderr (see section 3).

| Verb | Purpose | Required inputs | Output | Chainable ids | Effect |
|---|---|---|---|---|---|
| `run <experiment.toml> [--set PATH=VALUE]... [--force]` | Resolve, plan, start a detached supervisor job (`run.rs:146-200`, `309-341`) | experiment file; `candler.toml` found from cwd; Docker; secrets named by `secrets[]` in env | stdout is one line: the job id (`run.rs:339`). No plan, cell count or cost shown | job id | MUTATING and COSTLY: spends tokens, money and wall clock. No dry-run flag. `--force` accepts drift against stored runs |
| `resolve <experiment.toml> [--set] [--json]` | Run the same preflight as `run`, print every setting with its source, stop (`resolve.rs:1-37`, `223-299`) | experiment file; Docker for the preflight | text lines `failed<TAB>check.id<TAB>explanation`, or `{"failures":[{id,explanation}]}` with `--json` (`resolve.rs:202-221`). Source printer at `resolve.rs:79-97` | none | Read-only on the store (writes no job). Does NOT plan runs, so budget, store-drift and cell-level plan refusals are not exercised. Check ids are not `codes.toml` codes (`resolve.rs:6-9`) |
| `ps` | List jobs | none | TSV: `id spec_digest state done/started what` (`ps.rs:30-46`, `283-296`). Exit non-zero if a row failed. No JSON | job id | read-only |
| `watch <job-id> [--json\|--live]` | Follow a job's event log | job id | one JSON event per line, returns when `job.ended` is seen (`watch.rs:98-209`); `--live` is a progress line | run keys inside events | read-only; may trigger lost-job recovery |
| `kill <id> \| --all [-y]` | Stop jobs | id or `--all`; `-y` or an interactive yes (`config.kill_not_confirmed`) | text | none | MUTATING (stops work) |
| `ingest <paths>... [--rules rules.toml]` (alias `import`) | Raw trace dirs, or legacy result folders with rules, into the store | paths; rules TOML for legacy folders (`ingest.rs:25-36`) | not confirmed what it prints | run keys | MUTATING store (free) |
| `evaluate <suite.toml> [<job-id> \| --runs GLOB]` | Grade stored runs with plug-ins (`evaluate.rs:1-40`, `275-400`) | suite file; `plugins.dirs` in `candler.toml`; `slots.evaluators` != 0; plug-in env built | job id on STDERR, blocks until done, report as one JSON document on stdout; exit 2 if any `failed_requests` | none in stdout | MUTATING store; wall clock, possibly tokens if evaluators call models. Default selection is runs with no complete outcome set |
| `show [--rates] [--evaluator name@ver] [--format json\|toml]` | Print stored outcomes or per-cell rates (`show.rs`) | store; evaluator must be unambiguous | JSON or TOML | run keys, cell digests | read-only |
| `analyze [--evaluator] [--experiment file] [--format json\|csv] [--table arms\|comparisons\|summary\|runs]` | Paired statistics (`analyze.rs:25-75`, `329-357`) | store with outcomes; one evaluator or `--evaluator` | JSON doc: `evaluator, alpha, i4_violations, arms[], comparisons[], families[], pairs_not_one_factor, runs[]`. Per-pair `verdict` field; per-expectation `p, p_holm, reject, diff, diff_lo, diff_hi, reading` (`analyze.rs:258-289`) | cell, run keys | read-only |
| `diff --from A --to B [--equivalence file] [--format ...]` | Compare two evaluator versions (`diff.rs:10-28`) | two evaluator refs; optional equivalence JSON | text or JSON; exit 1 when rows changed | none | read-only |
| `inspect <run-dir \| run-key-prefix(>=6) \| job-id>` | One run or job as JSON (`inspect.rs:205-351`) | target | always JSON. Run: `run, sealed, integrity, experiment, job, started_at, ended_at, host, provenance, measured, outcomes+evidence`. Job: `id, state, runs[{key, integrity, outcomes status counts}]` | run keys from a job id | read-only |
| `ls [paths...]` | List experiment files and stored-run match counts (`ls.rs:33-231`) | none (scans `experiments/`) | TSV `path<TAB>cells<TAB>sealed_runs`, or `path<TAB>skipped<TAB>reason` | none | read-only; cheap cell count with no Docker |
| `supervise --job` | Hidden; started by `run` | - | - | - | internal |

Observations: `run` and `evaluate` are the only verbs that spend; `run` returns before the work and prints only the job id, `evaluate` blocks. Of the read verbs, only `inspect` and `analyze` are always JSON; `ps`, `ls`, `resolve` (default) are TSV or text.

## 2. Hand-authored inputs

Source: `docs/contract/experiment-file.md`, `suite-file.md`, `plugin-manifest.md`, `evaluator-protocol.md` (511 lines, plug-in authors only). Each doc has a generated Fields table and a Refusals table.

**Experiment file** (`experiment/1`): about 30 field paths.
- Top level: `schema`, `replicates`, `budget` (`per_run.wall_s` is the one required leaf; `max_cost_usd`, `max_parallel` default 1, `max_runs`, `max_tokens`, `wall_s`, `per_run.turns`, `per_run.max_cost_usd`), `harness{name, version?}`, `treatment{name, source?, tag?, commit?}`, `factors[{path, levels[]}]`, `fixed{}` map of dotted setting paths, `inputs[{kind, path, at?, audience}]`, `secrets[]`.
- Derivable or defaultable: `schema` tag is a constant; `budget.max_parallel` and `audience` already default; `harness.version` is optional; `treatment.commit` is optional and derivable from `tag` (the plan re-checks tag-to-commit, `plan.rs:1053-1204`); input digests are computed by Candler (`experiment/inputs.rs:1-22`); paths are relative to the file. Nothing the author writes is a hash.
- Not derivable: `replicates`, budget numbers, factor levels, `fixed` setting paths and values (an arbitrary dotted-path namespace with no listing command), harness name (the error lists registered names).
- Ceremony: `replicates` and `budget.per_run.wall_s` are required with no default, so even a one-run smoke test needs them. A wrong setting path gives `experiment.unknown_path` naming the path but not the valid ones (`refusal.rs:~49`, message "is not a path of the experiment surface; it is written in {location}"). `factor_without_levels` and `setting_as_factor` (a factor cannot vary `budget.*`, `replicates`, `request.*`, `isolation.build.*` and similar; those go in `[fixed]`; `layers.rs:27-58`) are classes of mistake an agent will hit.
- Errors: `experiment.document_invalid` with a JSON pointer in `path` for field errors; unknown key gives `experiment.unknown_field` with the key as pointer; wrong-form values print `experiment.document_invalid`, never `format.*` (doc note line 108).

**Suite file** (`suite/1`): `schema`, `source?`, `expectations[]` with `id, kind, role (gate|quality|descriptive), evaluator{name, range}, params{}, depends_on[] (required even if empty), applies_to[]?, deadline_ms?, group?`. Per entry 7 to 9 fields. `evaluator.range` (semver string) is derivable from the installed plug-in; `kind` and `params` are only discoverable from the plug-in's `describe` and its params schema. Refusals: dependency cycle, unknown dependency, id repeated, `params_invalid` (with a pointer inside `params`), `kind_unknown`, `evaluator_version_mismatch`.

**Plug-in manifest** (`plugin-manifest/1`): `schema, name, version, protocol{major,minor}, command[], environment{kind uv|none, project, lock}, agent_environments{}`. `version`, `protocol` and the uv `project/lock` are derivable from the plug-in (it must also match `describe`, else `plugin.describe_mismatch`). The manifest is one of two filenames and never both (`plugin.manifest_ambiguous`).

**Import rules** (`ingest --rules`, `ingest.rs:38-87`): `format, instrument, suite_kind, entry_params[], metadata[]?, [harness]{name, version_pointer}, [treatment]{pointer, resolved}, [[snapshot]]{kind, pointer}, [[marker]]{pattern, code|capture}`. Unknown fields are refused; no scaffold. Cause is documented as a hand-written input ("cannot be derived", `ingest.rs:30`).

**Project config** `candler.toml`: `plugins.dirs` is required by `evaluate` (`config.no_plugin_dirs`). `store.root`, `slots.*` are optional.

**Evaluator protocol**: JSON-RPC style `describe`/`evaluate` over stdio frames; only plug-in authors touch it. The Python helper (`evaluator-helper.md`) removes most of the burden.

## 3. Error and refusal surface

- Every `candler` failure is rendered by `render` (`commands/refusal.rs:492-502`). `--error-format json` writes one `refusal/1` record on one line: `code, side, message, optional path (JSON pointer), causes[]` (`refusal.rs:433-472`). Parser errors get the same record as `cli.usage_invalid` (`refusal.rs:474-487`).
- `docs/contract/codes.toml` registers about 190 stable codes in sections `schemas, resolve, experiment, runner, isolation, store, supervise, harness, cli, jobs, evaluation, import, manifest, analysis, helper`, with `side` (who must fix: experiment, suite, plugin, config, environment, candler). A code is never renamed or removed; retired and lifted rows are kept (`codes.toml:1-30`).
- A second `[internal]` group (`codes.toml:1364-1760`) lists errors that are Candler's own faults, which print `candler.internal`. Those, and the carrier types `CliError::Refused/Failed/Runtime`, are an agent's dead end: `CliError::Refused` "reports as candler.internal until that verb converts it" (`codes.toml:1591-1594`). Several `RuntimeError` variants are text-only (`jobs.runtime.RuntimeError::Setup/Body`: "the cause is a string, so no code can be derived").
- Messages say what to fix in many cases, but there is no separate "how to fix" field. Fix advice lives in the free text or in `when`/`at` sentences of the contract tables ("written for a coding agent", `experiment/refusal.rs:1-6`).

Examples:
1. `config.ambiguous_run`: message "`{target}` starts {count} runs: {listed}{tail}; give more of the key" (`refusal.rs:18-140` span, variant AmbiguousRun). Actionable, lists candidates.
2. `experiment.unknown_harness`: "`{name}` is not a harness this binary ships: the registered ones are {registered}" (same span). Lists valid values.
3. `config.no_plugin_dirs`: "list one under `plugins.dirs` in a `candler.toml` at or above the working directory" (same span; table row `refusal.rs:233-337`). Names the file and key.
4. `experiment.unknown_path`: names the misspelt path and where it was written, with a JSON pointer via `cli_refusal_path` (`refusal.rs:208-231`), but does NOT list valid paths or suggest the nearest. Not actionable without outside knowledge.
5. `runner.fact_changed`: "a re-checked fact differs from the stored cell: {deviations}" (`runner/plan.rs:180-263`). Does not say that `--force` is the accepting path; `side = environment`.
6. Weak spots: `experiment.input_unusable` has `at` pointing at the whole `inputs` array rather than an element index (`experiment/refusal.rs:25-39`). Many rows in `refusal.rs:233-337` have path `-`. `candler.internal` carries no guidance.

## 4. Computable chores, ranked

Evidence is what the code and docs show; "not found" means the search did not surface it.

1. **Plan or dry-run without spending (`run --plan`)**. Today `resolve` stops before planning and `run` starts the job at once. `plan_runs` already runs with `PlanMode::Check` and no experiment lock in the foreground (`run.rs:268-307`), so the data exists: cells, replicates, stored runs replayed vs new, drift deviations. A plan verb would return cell count, new vs reused runs, per-run caps and the job caps, as JSON. This is the single biggest gap, since every other check is hostage to Docker plus a real run.
2. **Cost and budget estimate**. No pre-run estimate was found. Only in-flight cap checks exist (`jobs/orchestrate/caps.rs:136-162`: runs, tokens, cost, wall in that order). From the plan, code can compute worst-case bounds: `runs_new * per_run.wall_s / max_parallel` wall time and `runs_new * per_run.max_cost_usd` money, flagging when they exceed the job caps. A warning when per-run caps are unset is a cheap guard.
3. **Machine-readable status and one-call verdict**. To learn "did the run pass" today: `run` (id) then `watch <id>` (parse events for `job.ended`) then `inspect <id>` (run keys, status counts) then `evaluate` then `analyze --format json` then dig `comparisons[].entries[].reject`. There is no `ps --json`, no `run --wait`, no overall verdict, and `analyze` returns raw statistics (per-pair `verdict` plus per-expectation `reject/reading`) that an agent must fold. A `summary` verb or an option on `analyze`/`inspect <job>` should return `{job_state, runs_valid, gates_pass_rate, per-pair verdict, reasons not computed}`.
4. **Scaffold an experiment file from a question** (harness, treatment source, factors and levels, replicates, per-run budget). No scaffold verb exists. Derivable: schema tag, treatment commit from tag, harness version, defaults. A scaffold plus a structured "valid setting paths and types" listing removes the guessing behind `unknown_path`.
5. **List valid settings, factor levels and harness names.** `unknown_path` and `unknown_request_key` do not list valid paths. The harness list is only in the error text. A `schema`-style verb would give the setting namespace, types and registered harnesses.
6. **Validate without Docker.** `resolve` requires the Docker preflight (`run.rs`, `resolve.rs:223-299`). Pure document checks (TOML shape, unknown fields, factor/fixed path overlap, `setting_as_factor`) could run with no daemon. `ls` already reads files without Docker but only reports a cell count or `skipped<TAB>reason`.
7. **Scaffold a suite file or manifest.** `evaluator.range`, `depends_on` default, kinds and params schema come from the plug-in's `describe`. No generator found.
8. **Locate a run's ids.** `inspect <job-id>` lists run keys; `inspect <prefix>` needs 6+ characters; `evaluate` accepts a job id. This works but `run` prints only the job id, and run keys appear only in events or inspect.
9. **Import legacy folders.** `ingest --rules` needs a hand-written rules file (4 required scalar fields plus sections); an `ingest --infer-rules` helper is a plausible chore. Importer is deterministic once rules exist.
10. **Diff two experiments.** Not found. `run --force` accepts drift; `Changed(Vec<Deviation>)` (`plan.rs:180-263`) is the data, shown only as a refusal. Nothing compares two experiment files or one file against the store except `analyze --experiment`, which checks recorded factors and refuses on difference.

## 5. Speed and many-step chores

- "Did it pass?": five commands plus a manual fold (see chore 3).
- "How many runs will this launch and what could it cost?": no command; run `ls` for cell count, multiply by `replicates` by hand, and nothing gives money.
- "Is this experiment valid?": `resolve` needs Docker, may build the sidecar-related checks, and its output is TSV unless `--json`, which only covers failures (the successful resolution print path is in a separate renderer). Mix of check ids and `codes.toml` codes.
- `evaluate` blocks until finished (job id only on stderr), unlike `run` which detaches; an agent must choose different wait strategies for the two.
- Wall clock waiting: `watch` is the only way to wait and it streams events; no `--until-ended` summary.
- `ls` reads every file under `experiments/` and prints human TSV with colour when attached to a terminal.

## Unverified or not checked

- Exact stdout of `ingest`, `kill`, `diff` (text vs json default) and the success print path of `resolve`.
- `candler-schemas` source for the `ExperimentFile` struct (outside the allowed search paths); the experiment fields come from the generated table in `docs/contract/experiment-file.md`.
- Whether `analyze` reports cost or tokens (none of the JSON keys found do).
