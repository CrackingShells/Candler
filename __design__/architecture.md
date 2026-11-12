# Candler — Architecture

The design of Candler. The wire schemas and milestones M1 (contracts), M2a (evaluation parity)
and M2b (execution parity) are built; what a later milestone owns names it. Where built code
still falls short of a rule stated here, the rule stands and the shortfall is a code defect. Points marked *provisional* are expected
to move once code meets real harnesses; the rest stand unless implementation proves them wrong, and
then this document changes together with the code. The reasoning behind the design, and its review,
are in the `candler_core` reports of mcp-python-ingestion (`__reports__/candler_core/` on branch
`feat/skill-htn-creator`).

## Executive Summary

- **Problem.** Running and evaluating LLM agents is welded into one domain-specific Python harness
  (`htn_creator_eval`, 22k lines): a closed grader dispatch over 10 `grading_spec.type` values,
  trace parsing for Claude Code only, a CLI whose flags are mostly derivable, and one Python process
  spawned per expectation per run.
- **Proposed change.** A standalone Rust core with two equal halves behind one operation API:
  - **execution:** run agents through shipped harness drivers under controlled conditions;
  - **evaluation:** grade traces and artifacts, fresh or on disk, through out-of-process evaluator
    plug-ins speaking one protocol.

  The library, a lean CLI and (later) an MCP server are thin surfaces over the API. Domain science
  enters only as plug-ins. The core depends on no project.
- **Non-goals.** Domain science (HTN tiers, PlanTrace, the `check_*` scripts stay in their project);
  LLM-as-judge gates; adversarial security isolation; dashboards and figures.
- **Biggest risks.** A trace model that drops signal; a driver trait shaped like Claude Code;
  supervision leaks (orphans, detached descendants); parity failures that are really instrument
  drift; fork safety in the Python helper on macOS; the team's first Rust project.
- **Validation.** Golden-regrade parity on the 240-run corpus (the 8,880 Tier A–C rows of its
  10,320; Tier D uncompared); execution
  parity with today's `run` on a smoke matrix; a second harness (OpenCode v2) and a second plug-in
  (`waiting-on-processes`) before the contract freezes at 1.0.

## Current State

```mermaid
graph TD
  subgraph htn_creator_eval [htn_creator_eval: one Python package, 22k lines]
    CLI[CLI: run / grade / show / inspect / run-config] --> SUITE[suite: orchestration, resume with wipe, results layout]
    SUITE --> SB[sandbox build + confinement hook]
    SUITE --> AG[claude --print, killed by pid only]
    AG --> SP[stream_profiler: Claude Code events only]
    SUITE --> GR[grader: closed grading_spec.type dispatch]
    GR -->|one python -m per expectation, 60 s each, stdout regex| SK[skill's check_* scripts, editable working tree]
    GR --> TD[check_tier_d: per-run servers since 0.4.2]
    SPEC[eval.json: 43 expectations] --> GR
    GR --> OC[_outcome.py: pass, fail, blocked counted; not_measured, not_applicable excluded]
  end
```

Measured on the eval env (2026-10-28): a bare interpreter starts in 34 ms; a spawn plus checker
import costs 40–285 ms. 18 expectations per run are graded by spawned skill scripts, so a 240-run
regrade spends roughly 5–15 CPU-minutes on startup alone.

## Proposed State

```mermaid
graph TD
  subgraph Surfaces
    LIB[library] --> API
    CLI[lean CLI] --> API
    MCP[MCP server: later] --> API
  end
  API[operation API: resolve, jobs, budgets] --> JOB
  JOB[job supervisor: one detached process per job] --> EXE
  JOB --> EVA
  API --> ANA
  SUP[process supervisor: spawn, child records, deadlines, stop escalation, stray sweep, slots]
  subgraph EXE [Execution]
    ORCH[orchestrator: matrix, run identity, resume, budgets] --> HD{harness driver: Claude Code, OpenCode v2, ...}
    ORCH --> ISO{isolation: host or container}
    ORCH --> CAT[catalog providers: snapshot per run]
    HD -.describes.-> SUP
  end
  subgraph ING [Ingestion]
    RAW[raw traces with receive times] --> TR[trace readers] --> TM[normalised trace model]
    LEG[legacy importer] --> TM
  end
  SUP --> RAW
  EXE --> STORE[(run store: files-first)]
  TM --> STORE
  subgraph EVA [Evaluation]
    REG[registry: plug-in manifests + suites] --> ENG[evaluation engine + outcome algebra]
    ENG -->|JSON-RPC on stdio| EVP[evaluator process per request]
    ENG --> OUT[(outcome store: one file per request)]
  end
  STORE --> ENG
  EVP -.spawned by.-> SUP
  JOB --> EVT[(job dir: job.json, events.jsonl, children, lock)]
  subgraph ANA [Analysis]
    AGG[selection rule, rates over weights, stats: tables out]
  end
  OUT --> AGG
  subgraph PLUG [Domain plug-in, e.g. htn, Python]
    MAN[plug-in manifest] --> REG
    SS[suite file] --> REG
    EV[evaluator command + pinned checker library + Python helper] -.declared by.-> MAN
    FX[fixtures, inputs, catalog providers] --> ORCH
  end
```

### Layer responsibilities

| Layer | Owns | Never owns |
|---|---|---|
| Operation API | The operation set, argument resolution with the source of every value, jobs, budgets, confirmations | Any domain notion |
| Job supervisor | Job identity, the job directory, the event stream, cancellation | What a run or an outcome means |
| Process supervisor | Every process Candler starts (agents, evaluators): spawn into its own process group, child records, deadlines, stop escalation, stray sweep, machine-wide slots | Harness or evaluator semantics |
| Execution | Experiment matrix, run identity, resume, isolation, catalog snapshots, artifact collection | What the artifacts mean |
| Harness drivers | Harness-specific data: config files, command line, stop action and signal, exit classification, what to collect, version detection | Starting, timing or killing processes |
| Ingestion | Trace readers, the normalised trace model, the legacy importer | Judgement |
| Run store | Run identity, content-addressed artifacts, the workspace view, manifest, integrity, the invalid-run ledger | Grades |
| Evaluation | Plug-in registry, suite resolution, `applies_to` and dependency `blocked`, evaluator requests, the outcome store | Domain logic |
| Evaluators (plug-in) | Domain science over trace + artifacts + snapshots + inputs + metadata | Layout, statistics, which runs an expectation applies to |
| Analysis | The selection rule, rates, tests, intervals, multiplicity; JSON or CSV tables | Re-deciding any outcome; drawing figures |

### Workspace

crates.io has one flat namespace without organisation scopes, so crates carry the project prefix
(`candler-*`, hyphenated; Rust code sees `candler_core`), not the organisation's name. The
organisation owns them through a crates.io team backed by the CrackingShells GitHub organisation.
All names below were free on 2026-10-28.

| Crate | Holds | Release line |
|---|---|---|
| `candler-schemas` | Every wire type as a serde struct; generated JSON Schema files committed beside them | `schemas@x.y.z` |
| `candler-core` | API, resolution, job and process supervisors, orchestrator, store, evaluation engine, outcome algebra, analysis (`statrs`) | core `vx.y.z` |
| `candler-harness` | The public `HarnessDriver`, `TraceReader`, `SessionTracker` and `SkillTree` traits, the settings table types, and the shared driver fixture suite (feature `conformance`) | core |
| `candler-harness-claude-code`, `candler-harness-opencode` | Shipped drivers, readers and skill trees, one crate per harness | core |
| `candler` | The binary: CLI, and later the MCP server as a subcommand | core |

**Python helper for plug-in authors** (`python/candler-evaluator` in this repo, outside the cargo
workspace, published to PyPI as `candler-evaluator` on its own tag line; the protocol does not
require it, the HTN plug-in uses it): the JSON-RPC loop; the file-descriptor stdout guard; one forked child per
expectation with its deadline; instrument digests and dirty flags; models generated from the JSON
Schema.

Tooling: Rust 2024 edition, Grit `rust-conventions`, `release-plz` (per-crate versions and
changelogs from Conventional Commits; git-only, releasing from `main` on every push with a
tag and a GitHub release per new version, and with publishing to crates.io off until it is
enabled), `cocogitto` (`cog verify` in a
`commit-msg` hook and in CI). `serde_json` with `float_roundtrip`.

### Workspace targets

Candler builds and runs on Linux and macOS. On Windows it runs inside WSL2, where it is Linux;
Docker Desktop on Windows already runs on WSL2. The binary refuses to compile on a non-unix target,
with one `compile_error!` whose message names WSL2, so no stub arm compiles nowhere. `candler-schemas`
and the Python helper stay portable.

A native Windows port is a milestone of its own, after M7. It starts from the committed inventory
`crates/candler-core/tests/vectors/platform_inventory.toml`, which lists every file that calls a
unix-only API (`std::os::unix`, `rustix`, `libc`): 21 at the time of writing, each with what it uses
and its Windows counterpart. A guard fails when a file outside the list starts to use one, so new
code goes through the existing seams (`Signaller`, `Supervisor`, the job lock and the slots) or
extends the list deliberately. The counterparts:

| Candler needs | Unix | Windows |
|---|---|---|
| Stop a process group, with escalation | process groups, `killpg` | Job Objects |
| Catch descendants that escape | subreaper and marker sweep | Job Objects (no sweep) |
| Never signal a reused pid | pid plus start time | a held process handle |
| Job lock, machine-wide slots | `flock` | `LockFileEx` |
| Detached supervisor | `setsid`, re-exec | `DETACHED_PROCESS` |
| Agent isolation | Docker CLI | the same CLI, with host path forms for mounts |
| The helper's isolation of each expectation | `os.fork` | a spawn mode |

## Operation API

| Operation | Input | Output | Long-running? |
|---|---|---|---|
| `resolve` | experiment or suite + overrides | the resolved experiment, every value with its source; preflight results | no |
| `run` | resolved experiment + budget | job id | yes (job) |
| `ingest` | raw trace dirs or a legacy results folder | run keys | no: runs in the caller, repeats to the same store, safe to interrupt |
| `evaluate` | suite + run selection | job id | yes (job) |
| `analyze` | analysis spec + outcome selection | tables (JSON or CSV), intervals, tests | no |
| `jobs` / `events` / `cancel` | job selection | job records, the event stream, cancellation | no |
| `show` / `inspect` | run, cell or experiment selection | outcomes with evidence, provenance, integrity | no |
| `diff` | two evaluator versions or two outcome sets | per-expectation changes | no |

**Resolution order (all surfaces):** explicit argument > experiment or suite file > project config
> user config > derived value > core default. `resolve` reports each value's source and the
manifest records it. Values that can be known are derived, never asked for. `resolve` is the real
preflight: skill checkout at the pinned tag, isolation build, plug-in environments (built once per
lock digest), evaluator `describe`, harness version and capabilities, secrets present, gateway auth
probe. No tokens are spent.

**Value sources** (`ValueSource`, recorded per value): `explicit_argument`, `experiment_file`,
`project_config`, `user_config`, `derived {step}` (the preflight step that derived it),
`core_default`, `imported {file}` (the legacy importer, naming the run-relative source file), and
`unrecorded`, which carries no value: the source system did not record it, and nothing guesses it.

**Preflight steps** are registered with the resolver, which runs them in registration order. Each
step sees the configuration, the caller's working directory and the values resolved so far
(including what earlier steps derived); a duplicate step name is refused, and so is a path that
two steps both derive. Every failed check is
reported before resolution fails. The milestones that own a check (evaluation, execution) register
its step; `resolve` itself knows none.

### Configuration

| File | Where | Keys |
|---|---|---|
| Project config | `candler.toml` in the project root, the nearest ancestor of the working directory holding one | `store.root` (default `<project root>/.candler/store`; with no project, `$XDG_STATE_HOME/candler/store`, else `~/.local/state/candler/store`), `plugins.dirs[]`, the directories `evaluate` finds plug-ins in |
| User config | `$XDG_CONFIG_HOME/candler/config.toml`, else `~/.config/candler/config.toml` | `slots.count`, `slots.evaluators` |

Unknown keys are refused, naming the file and key. A machine-wide setting (`slots.count`, `slots.evaluators`) is
refused in a project config. A relative path written in a config file resolves against that file's
directory; a relative path given as an argument resolves against the caller's working directory
(I14).

**Formats.** Documents people write (configs, experiment and suite files, plug-in manifests) are
TOML; records and messages are JSON. Either is validated, after parsing, against its JSON Schema.
Keys with a unit carry it: `deadline_ms`, `grace_ms`, `wall_s`, `max_cost_usd`.

### Budget

| Field | Meaning | On reaching it |
|---|---|---|
| `per_run.wall`, `per_run.turns?`, `per_run.cost?` | Caps on one agent run; a treatment condition, the same for every arm | The run ends, `ended_by = cap`; the run is valid |
| `max_runs?`, `max_tokens?`, `max_cost?`, `wall?` | Caps on the job | Runs in flight end `invalid(budget_stop)`; no new run starts |
| `max_parallel` | Concurrent agent runs or evaluator processes in this job; default 1 | — |

Across jobs, without a daemon, there are two machine-wide pools of slot files, because an agent run
holds its slot for minutes or hours while an evaluator is a short, CPU-bound process. Each agent
process takes one slot file of the agent pool (`~/.local/state/candler/slots/<n>.lock`, N from
`slots.count`). Each evaluator process takes one slot file of its own pool
(`~/.local/state/candler/evaluator-slots/<n>.lock`, N from `slots.evaluators`), so an evaluation
never waits behind agent runs. Both are taken with `flock` and released by the kernel when the
holder dies. The default of `slots.evaluators` is derived from the CPU count: half of it, rounded
down, and at least 1. The default of `slots.count` is *provisional*.

### Experiment file (`experiment/1`, on the `schemas@` line)

A TOML document: `schema`; `fixed` (field path → value, the same in every cell); `factors` (field
path → its levels, in order); `replicates`; `budget` (the fields above, with units in the keys:
`per_run.wall_s`, `per_run.turns`, `per_run.max_cost_usd`, `max_runs`, `max_tokens`,
`max_cost_usd`, `wall_s`, `max_parallel`); `harness`; `treatment`; `secrets[]` (variable names
only); `inputs[]`. A level is a value, or a table that fills the values under its path (a model
level sets the model id and its provider settings together). The cells are the cartesian product
of the factor levels, in the order the factors and levels are written; listing them is a pure
function of the file (`ExperimentFile::cells`), and running them belongs to the orchestrator. A
field path is dotted: `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$`.

- **Table-valued paths.** Any other table in `fixed` or in a level flattens into one value per
  leaf. A path whose value is a table kept whole does not: `candler_schemas::execution::
  TABLE_VALUED_PATHS` lists them, and holds `prompt.values`. Every value written at or below such a
  path, in `fixed` or in a factor's levels, nested (`[fixed.prompt.values]`) or quoted
  (`"prompt.values.x"`), folds into one object value at that path, placed where the first of them
  stood, so a cell holds one `prompt.values`. Two values that give one leaf, or one leaf below
  another, are refused (`Duplicate`, `PathOverlap`), and so is a table there that holds no value
  (`EmptyTable`), written in TOML or in a hand-built JSON document. The rule changes the cells of
  no file that has no table under a table-valued path, and no TOML file that was valid becomes
  invalid. `cells()` stays in `candler-schemas`, where the list lives.
- **Treatment.** `[treatment] {name, tag?, commit?, source?}`. `source` is the path of a local
  checkout of the treatment. Written in the experiment file it resolves against that file's
  directory; given by `--set request.treatment` it resolves against the current directory (I14).
  A treatment with no source is refused, naming `treatment.source`. A tag or a commit is resolved
  in that checkout before the cell is hashed, so a treatment that names one carries the commit it
  resolves to; a tag and a commit that disagree are refused, and a tag that no longer names the
  recorded commit is refused at resume, naming `treatment` with both commits. The source is
  recorded in `sources[]` as an absolute path, normalised lexically (no `.` segment, each `..`
  applied to the text, no trailing slash, no symlink followed), and never enters the digest: two
  checkouts of one commit give one cell. The retired `request` setting for the source is
  refused by `--set`, by name. Candler fetches nothing from a remote.
- **Inputs.** `[[inputs]] {kind, path, at?, audience}` declares a file or a directory a run reads.
  `kind` is a name, unique in the file; `build_inputs` is reserved for the files an environment
  build reads. `path` is relative to the experiment file (I14). `at` is the workspace directory
  the input is mounted under: relative, with no `..` segment, `.` when absent. `audience` is
  `agent` (the default) or `evaluators`. Each file of an agent input is mounted read-only at
  `at/<path>`, from its blob, and the input's digest enters the cell. An input for `evaluators`
  never enters the isolation (I8): it reaches an evaluator through its context only, and its
  digest stays outside the cell (§Run manifest). The files are read before any job directory
  exists, and the declaration is refused when a path is empty, absolute or holds `..`; a path or
  anything under it is a symlink, a special file or a name that is not UTF-8; a directory holds no
  file; a name or `at` holds a comma, a newline, a carriage return or a NUL; two mounts collide
  (one workspace path, or one inside the other); a kind repeats; or the kind is `build_inputs`.
  An empty file is a valid input.
- **Agent environment.** The experiment declares the environment's contents in two settings under
  `isolation`, next to `isolation.build.requirements`, the sources of the env lock. They are
  settings, not cell values (§Agent environment and secrets), and `resolve` validates both before
  any docker call.
  - `isolation.build.prune` lists file names the env build deletes after the install, each
    `^[A-Za-z0-9._-]+$` and neither `.` nor `..`. It is part of the env image's key: with nothing
    pruned the key is the lock's digest, otherwise the sha256 of the lock's hex digest followed by
    a NUL byte and each distinct name, sorted. The image's own id enters the cell as
    `isolation.image`.
  - `isolation.record_packages` lists import package names, each `^[a-z_][a-z0-9_]*$`. Each run
    reads the distribution and version of each from its env image and records them under
    `provenance.packages` (§Run manifest).

### CLI surface over the jobs

| Command | Does |
|---|---|
| `candler run <experiment> [--<field> <value> ...]` | Resolves (file first, CLI fields override), then starts a job; prints the job id |
| `candler ls` | Lists experiment files under `<project root>/experiments/`, recursively, plus any path given; with no project, under `./experiments/`. A TOML file that does not parse as `experiment/1` is listed as skipped, with the reason |
| `candler ps` | Lists jobs with id, experiment, state, progress |
| `candler kill <id> \| --all` | Stops jobs; asks for confirmation unless `--yes` / `-y` |
| `candler watch <id> [--json \| --live]` | Follows a job's events: `--json` (default) prints event records, `--live` draws progress |
| `candler ingest <path>...` | Turns raw harness trace directories (one run per stream; the registry's drivers recognise the stream and supply the reader) into runs; with `--rules <file>`, imports a legacy results folder under the import rules the TOML file states. It runs to completion in the caller, repeats to the same store and is safe to interrupt; `candler import` stays as a hidden alias for a results folder |
| `candler resolve <experiment \| suite>` | Prints every resolved value with its source and runs the preflight; starts no job |
| `candler inspect <run dir \| key prefix \| job id>` | Shows the run, or the job's runs, with outcomes, evidence, provenance and integrity, deriving the store from a run directory |
| `candler analyze [--evaluator <name@version \| version>] [--experiment <file>] [--format json\|csv]` | Reports rates, Wilson intervals and pairwise tests (§Analysis statistics). It finds the experiment file by the digest the runs' manifests name, under `<project root>/experiments/`; `--experiment` names it when it lives elsewhere |
| `candler evaluate <suite> [<job-id> \| --runs <glob>]` | Starts the evaluate job, prints its id on stderr, follows it and prints the report; Ctrl-C cancels it. With no selection it grades the runs with no complete outcome set for the suite's evaluator versions |
| `--error-format text\|json` | Global option: the form of a refusal on stderr (below) |
| `candler show`, `diff` | The other operations |

Every verb reads one store root: `--store <dir>`, given anywhere on the line, else `CANDLER_STORE`,
else the configuration's `store.root`, else the default above, as `resolve` layers them. A verb
that finds none refuses, naming the three ways to give one.

**Terminals.** `watch` prints event records (`--json`) by default, and `--live` draws progress.
Colour is on only when stdout is a terminal and `NO_COLOR` is unset; there is no `--no-color`. The
binary embeds the container assets with `include_bytes!` and writes them to a cache keyed by their
digest, so it runs from any directory.

The global option `--error-format text|json` defaults to `text`; `json` writes one `refusal/1`
record per line on stderr. It is neither derived nor a file setting: what parses the caller's
stderr is not visible from the terminal, and a refusal can come before any file is read. Deriving
it from whether stderr is a terminal would hand JSON to a human who pipes and text to an agent
behind a pty.

"Experiment" names the file that declares factors, levels, replicates and budget (today's
`run_config` YAML). The experiment is never called a plan, which collides with HTN planning: the
file, its schema and every user-facing name say "experiment". The ban covers that configuration
only; what execution derives from an experiment (the runner's `RunPlan`, a cell's
`ContainerPlan`) and the `plan.*` keys of an imported legacy `experiment.json` keep their own
names.

## Jobs, runs and supervision

### Job

- **No daemon.** Each job is one detached supervisor process. When no job runs, nothing of Candler
  runs.
- **Job directory** `<store>/jobs/<job-id>/`: `job.json` (id, supervisor pid and start time, spec
  digest, state), an append-only `events.jsonl`, `children/`, and a `lock` the supervisor holds with
  `flock`. `ps` reports a job `lost` when its lock is free and no `job.ended` event exists.
- **Event envelope** (on the `schemas@` line): `{v, seq, at, job_id, run?, kind, payload}`.
  Consumers skip kinds they do not know.
- **First event set** (*provisional*): `job.started`, `job.ended`; `run.started`, `run.ended` (with
  `ended_by` and exit status), `run.failed` (reason code), `run.stray` (survivors found by the
  sweep); `evaluation.outcome`; `process.spawned`, `process.stop_step`, `process.exited`. The job
  record (`job.json`), the envelope and these payloads are on the `schemas@` line.
- **Evaluate job.** An evaluation is a job like a run job: the same record, lock, children,
  subreaper and sweep. Its events add `evaluation.planned {outcomes}`, the number of records the job
  will try to commit, before its first outcome, and `evaluation.outcome` for each committed record.
  `JobSpec` stays the run spec; the job directory holds a `JobSpecFile` (`Run` or `Evaluate`), read
  and written at the job-directory boundary only. The evaluate body has its own serve path, with no
  container. `candler evaluate` starts the job at once and waits for it; the job form is what lets
  `kill` and the recovery of a lost job reap evaluator processes after a crash, and lets one
  `events.jsonl` serve `watch`.
  A finished evaluate job leaves `report.json`, or, when an error ended it, `refusal.json`: the
  `refusal/1` record of that error, written before `job.ended`, which the command prints in place
  of a report. The command stops the job on Ctrl-C only: SIGTERM or SIGHUP of the command leaves
  the job running, as it does for any job. Two evaluations over the same runs at once may grade a
  run twice; the readers (`show`, `analyze`, `diff`, the pending selection) take the current
  record under each I4 key, so the run counts once. A claim per run is not in place.
- **Order of delivery:** `run` + events + `ps` + `kill` → `watch --json` → `ls` + `watch --live`.

### Run identity and resume

- **Run key** = `(cell digest, replicate, attempt)`. The cell digest hashes the cell's comparability
  fields (see the run manifest). Replicates count valid attempts only.
- **One supervisor per experiment:** `<store>/experiments/<digest>/lock`, held with `flock`.
- **Completion** = `run.done`, written (fsync, then rename) after collect and ingest. It seals the
  run: nothing in its directory changes afterwards, and the manifest's `integrity` says whether the
  run counts. The manifest may be rewritten until then.
- **Resume** replays the stored resolved values, never re-derives them. If a value that is re-checked
  differs (a harness that auto-updated, a missing secret, a moved tag), resume refuses unless forced,
  and a forced deviation is recorded on every run it affects. Before reusing a run, resume reaps the
  recorded children of any lost job.
- An attempt without `run.done` is sealed `invalid(interrupted)` and stays in the ledger; the next
  attempt gets the next number. Nothing is ever deleted. A forced deviation is listed in the
  manifest's `deviations[] {path, stored?, observed?}`, outside the hash.

### Ends

| End | Run status |
|---|---|
| Completed, agent error, or a per-run cap | valid; `ended_by` measured |
| Gateway or provider error (connection refused, 0 tokens, or an error result carrying a provider status) | `invalid(gateway)`; retried once as a new attempt |
| Job budget, `candler kill`, supervisor lost | `invalid(budget_stop \| killed \| interrupted)`; never charged to an arm |
| Input passed off as output | `invalid(input_leak)` |
| Collection failed, or the reader cannot parse the stream or does not know its format version | `invalid(collection_error \| reader_error \| format_unknown)`; raw kept |
| Infrastructure: the daemon failed (`docker run` exit 125, or the container never started), the container was OOM-killed or killed by someone else (137 with no stop of ours), or the trace is silent | `invalid(infrastructure)`; `ended_by` unknown; never retried |

The driver's `classify_exit` sees the parsed sessions, so an exit 0 with an error result, or a run
with zero tokens, is classified from the trace, not from the exit status.
An error result with no provider status, such as a usage-policy refusal after the agent has
worked, is an agent error: the run is valid and graded, never retried as a gateway failure.
In a container the end also reads `docker inspect` (`State.OOMKilled`, `ExitCode`). The entrypoint's
own `timeout` (exit 124) is the wall cap, as is the supervisor's deadline. When several signals
hold, the first match wins: our stop (`budget_stop`, `killed`), daemon facts, OOM, a foreign 137,
the gateway, the reader, the trace's gateway signals, collection, input leak, then the valid ends.

### Process supervision

- Every child is spawned into its own process group with `CANDLER_RUN=<run key>` (or the request id
  for evaluators) in its environment. Its pid, pgid and start time are written to `children/` as soon
  as the spawn returns.
- **Stop escalation:** the driver's pre-stop action, if any (an HTTP call or a command the core
  runs) → the request's stop hook, if any → the driver's stop signal and grace → terminate the
  process group → kill it → **stray sweep** → the stop hook's destroy step. Every step is an event.
- **Containers (Docker, through the `docker` CLI).** The supervised child is the `docker run`
  client; its stdout is the stream. Killing the client does not stop the container, so a run in a
  container carries a **stop hook**, attached per spawn request: at pre-stop it reads the cidfile,
  re-verifies the container and runs `docker stop -t <grace>`; its destroy step runs only when the
  escalation ended the child. A run that exits by itself is inspected, collected, then destroyed.
  Agent and gateway containers never use `--rm`, so the exit facts survive for the inspect.
- **Container records.** Each container gets `children/container-<cid12>.json`
  (`container-record/1`: `{cid, labels {job, run, experiment}, created_at, image_id, role:
  agent|gateway, spawned_at}`) and the labels `candler.job`, `candler.run`, `candler.experiment`
  and `candler.role` (`agent` or `gateway`, which the destroy guard reads). A control container
  (a version readout, a catalog capture, the image build) is the exception: `--rm`, only the label
  `candler.role=control`, and no network except for the image build, which reaches the package
  index over the default bridge; no sweep meets it and the guard never destroys it.
  The entrypoint runs under `timeout <wall_s + 60 s>`, so an orphaned container expires by itself
  on any engine.
- **Stray sweep.** Harnesses start descendants in new sessions (Claude Code 2.1.304 spawns its Bash
  shell and its hooks detached), so a group kill misses them. Inside a container they die with the
  container. On the host, on Linux, the job supervisor is a child subreaper and scans `/proc` for
  the marker; orphans it adopts are signalled one pid at a time after their parent (itself) and
  start time are re-verified, and reaped with `waitpid(pid)`, never `waitpid(-1)`. macOS has no
  host scan and no subreaper: a descendant that leaves its group (an evaluator's `setsid` child)
  survives a `kill` and the end of the job, and the gap is recorded. The docker client and
  first-party commands are the only host children the design vouches for there, and an evaluator
  is neither: its stray survives on macOS, so the end of an evaluate job is complete on Linux
  only. The container sweep lists the job's labelled containers and destroys the strays
  among them. Survivors are evented as `run.stray`, then killed.
- **Supervisor death.** A job is lost when its lock is free and its events have no `job.ended`.
  `ps`, `kill` and `run` reap a lost job's recorded children, its labelled containers, its network
  and its strays; until one of them runs, an orphaned container runs at most to its own `timeout`.
  A lost evaluate job owns no container: its recovery builds no docker client, so `ps` and `kill`
  recover it on a machine with no `docker`. It ends the evaluators' groups and seals the job; it
  does not end a descendant that left its group with `setsid`, because once the supervisor is dead
  such a process has no Candler parent and I22 signals only a process whose parent is the caller.
  Containment by cgroup or pidfd would close that gap and is not in place.
  There is no parent-death signal: setting one needs a `pre_exec` hook, which
  `forbid(unsafe_code)` excludes, and a container outlives its client anyway. Labels, self-expiry
  and the Linux child subreaper (safe through `rustix`) bound orphans instead.
- **Child record** (`child-record/1`, on the `schemas@` line): `{pid, pgid, start, marker,
  spawned_at}`, written and fsynced in `children/` before `spawn` returns. `start` is `{kind: linux,
  boot_id, ticks}` (field 22 of `/proc/<pid>/stat`), `{kind: darwin, sec, usec}`, or `{kind:
  unavailable}`. A liveness check compares the pid *and* `start`, so a reused pid never looks alive;
  `unavailable` never reads as alive.
- **Exit status** `{kind: code, code}` or `{kind: signal, signal}`, the same type in the driver's
  `ProcessExit` and in the `process.exited` payload.
- **Reaping from a record.** `reap(record, stop)` stops a group from its child record alone, so
  resume and `kill` reap a lost job's children with the same escalation.
- **Signalling (I22).** Every signal goes through one choke point, which denies by default. It
  signals only a pid or group this supervisor spawned and still holds in its live child table,
  re-verified (leader pid and `start` match the child record) right before the signal. It also
  refuses a pid or group of 1 or less, its own pid and its own process group, since `kill(-1)`
  signals every process the user owns. A group read back from disk (`reap`, the stray sweep, a
  container census by marker) passes the same re-verification; a mismatch is refused and
  reported, never signalled. Delivery sits behind a `Signaller` seam: the guard and the
  escalation logic are tested against a recording fake, and real-signal tests only check end to
  end that spawned children die. The guard also signals two kinds of process this supervisor did
  not spawn, the class of call behind the `kill(-1)` incident, so their rules are stricter:
  - a host stray found by the marker scan (`signal_marked`), and an orphan the subreaper adopted
    (`signal_adopted`, its parent is this process);
  - right before the signal, the guard re-verifies the target's pid, its `start` and the run
    marker read again from that process (on Linux, `/proc/<pid>/environ`); a marker match from
    an earlier scan never suffices;
  - it refuses a pid of 1 or less, its own pid, its own process group, a process of another uid,
    and a target that no longer carries the marker. A refusal is a recorded outcome, never a
    fallback to a wider signal;
  - each call names one pid. No path takes a pattern, a name or "all": `kill --all` walks the
    job records one verified target at a time;
  - a group is adopted for stopping only when the job's `children/` records list it and its
    verification passes, never on a marker scan alone;
  - an orphan the subreaper adopted is signalled only if, right before each signal, its parent
    is this process, its `start` is not earlier than the supervisor's own (the same boot; a Linux
    start is counted in 10 ms ticks, so a descendant can share the supervisor's tick), the marker
    re-read from it matches and its uid is ours. The guard signals that pid; it signals the
    pid's group only if the group's leader passes the same check, never because one member
    passed. A reparented process without the marker is refused and left alive.
- **Destroying containers (I22 for containers).** Destroying a container or a network is as
  dangerous as a signal, so one guard holds the only destroy capability. It never destroys by name,
  pattern or "all". A container is destroyable if the job's `children/` records it, or if it is
  labelled for the current job or a lost one (its lock is free) with the matching run, and
  `docker inspect` re-verifies its labels and creation time right before `stop` or `rm`. A
  mismatch is refused and reported. The gateway sidecar and the per-job network follow the same
  rule. The destroy calls sit behind a seam with a recording fake, as `Signaller` does.
- **Pre-stop over HTTP** reaches loopback only: the host is parsed, never matched as text, and must
  be `localhost`, `127.0.0.0/8` or `::1`; redirects are not followed. A failed or timed-out pre-stop
  continues the escalation.
- **Supervision events:** `process.spawned`, `process.stop_step` (which step, and when),
  `process.exited`. The sink that receives them owns the envelope's `seq`, `job_id` and `at`.

### Agent environment and secrets

- The core builds each child's environment from an allowlist (`PATH`, `LANG`, an isolated `HOME`
  and XDG directories, `TMPDIR`) plus the driver's overlay. Nothing else is inherited (today's code
  removes `CLAUDECODE` and the conda and venv variables explicitly; under an allowlist they never
  arrive).
- An evaluator is the exception: an evaluate job's supervisor and the evaluator processes inherit
  the caller's environment, passed as the process environment and written to no file, event or
  error. Agent jobs inherit nothing.
- Secrets are referenced by variable name in the experiment, resolved when the process is spawned,
  checked at `resolve` and again at resume, and recorded as `{name, present}`, never as a value.
- **In a container** the environment is built from nothing and passed with `-e` only; the docker
  client itself gets its own allowlist (`DOCKER_HOST`, `DOCKER_CONTEXT`, its config dir). The
  container runs non-root, with a read-only root, tmpfs `/tmp`, no capabilities,
  `no-new-privileges`, and pids, memory and CPU limits. Only the workspace, a per-run HOME (a
  host directory outside the workspace, so sub-agent transcripts survive), the tmpfs `/tmp` and
  the agent environment at `/opt/env` are writable; inputs are mounted read-only at their
  workspace paths. `/opt/env` is an anonymous volume with no host source, which docker fills from
  the locked image for each container, so the agent can install into its own environment while
  every run starts from the same copy and the root stays read-only.
- **The model token never enters the agent container.** The agent sits alone on a per-job
  internal network with a gateway sidecar, which holds the token (passed through stdin into a
  tmpfs file, never env or argv) and forwards to the provider. The agent gets a base URL and a
  dummy token that mean nothing off that network. The sidecar reaches the provider on the job's
  second network, a user-defined egress network. A cell may ask for `bridge`
  (`isolation.network`) when the agent needs the internet: its container then joins the egress
  network as well (two `--network` flags on one `docker run`, Docker Engine 25 or newer).
- **The configuration surface.** Each path is a cell value (hashed and compared), a setting
  (dropped from `Comparable`) or a typed root (filled by derived steps, never by the file):

  | Path | Class |
  |---|---|
  | `model.id`, `model.endpoint`, `model.effort` | cell value |
  | `tools.allowed`, `tools.disallowed`, `mcp.*`, `permission_mode` | cell value |
  | `prompt.template`, `prompt.values` | cell value |
  | `isolation.image` (the derived local image ID), `isolation.network`, `isolation.env_lock`, `isolation.limits.*` | cell value |
  | `budget.per_run.{wall_s, turns, max_cost_usd}` | cell value |
  | `runner.mode` (`headless` or `session`; no core default, so an absent value is headless and every existing cell keeps its digest), `runner.idle_s` (whole seconds or `none`; session mode only) | cell value |
  | `isolation.build.{dockerfile, context}` (its content enters `inputs[]`); `isolation.build.{requirements, lock}` (the lock they give enters the cell as `isolation.env_lock`); `isolation.build.prune` (part of the env image's key, so it reaches the cell through `isolation.image`) | setting |
  | `isolation.record_packages` (read from the env image, recorded in `measured[]`, never in the cell) | setting |
  | `request.harness`, `request.treatment`, `request.inputs`, `model.token_secret` | setting |
  | `harness`, `treatment`, `inputs[]`, `snapshots`, `environments` (reserved) | typed root |

  A CLI path resolves against the caller's cwd, a path in the experiment file against that
  file's directory (I14).

### Raw traces

The supervisor archives each harness output stream as a framed file. That file is the raw input of
I3, so event times are reproducible even where the harness emits none.

**`raw-frames/1`** (JSONL, on the `schemas@` line):

- **Header**, the first line: `{kind: header, schema, stream, clock, source?}`. `stream` is a stream
  name (`stdout`, `stderr`, or a collected file's name). `clock` is `{kind: monotonic, anchor{wall,
  mono_ns}}`, one wall-clock reading paired with the monotonic clock, or `{kind: unrecorded}`.
  `source` is the digest of the file the frames were cut from, as that file is stored, when they
  were cut after the fact: for an imported agent stream, the gzipped legacy file, though the
  frames hold its decompressed bytes.
- **Frames**, the following lines: `{kind: frame, t_mono_ns?, data}`, with `data` either `{kind: text,
  text}` (valid UTF-8) or `{kind: bytes, base64}`. Frame indices count the lines after the header,
  from 0.
- **Line framing.** One frame per line of the stream, its `\n` kept. A trailing line without `\n` is
  its own frame. Concatenating the frames' data reproduces the stream byte for byte. A stream read
  by a `Document` reader is framed whole instead: one frame holding the stream.
- **Writing.** The file is written plain and flushed after each frame; on close it is gzipped (header
  mtime 0) and renamed into place. A stream framed after the fact (an imported legacy stream) has
  `clock: unrecorded` and no `t_mono_ns`.
- **Times.** `Event.at`, `Session.started_at` and `Session.ended_at` are optional. Each comes from
  the harness's timestamp, else from the frame time; neither is ever interpolated.

## Harness drivers

### What the runner needs from a harness

| Phase | Needed | Varies between harnesses |
|---|---|---|
| Prepare | Model, endpoint, skill or plug-in, MCP servers, permissions and instructions written into the workspace | File names and formats; model ids and provider configuration |
| Confine | An in-harness guard where one exists (Claude Code's PreToolUse hook) | Some have none; the container is then the only boundary |
| Launch | Headless command line, environment overlay, working directory | Flags; budget options |
| Observe | Live events, normalised; the session ids seen | Streamed on stdout, or kept in a local database |
| Stop | An optional pre-stop action, then a signal and grace period | Execution may live in a server outside the child (OpenCode v2) |
| Classify the exit | Exit status, output tail and the parsed sessions → `ended_by` | Codes, messages, error results |
| Collect | Files, or the output of an export command | Home-dir transcripts, stdout, a SQLite store |
| Read offline | Parse a trace on disk, no process | Line streams or whole documents; format versions |
| Census | The tool catalog the agent actually had | How to list built-in and MCP tools |

### Traits (signatures only)

Every item added for the second harness (`settings_table`, `gateway`, `state_paths`,
`census_stream`, `session`, `session_tracker`, `classify_end`) has a default body, so a driver
implements only what its harness has.

- `HarnessDriver`
  - `id() -> HarnessId`; `supported() -> VersionReq`
  - `version_probe() -> CommandSpec`; `parse_version(output) -> Version`; the core runs the probe
    in a control container of the base image
  - `capabilities() -> Capabilities { graceful_stop, live_events, budgets{turns, cost}, in_harness_guard, census, execution_outside_child, session }`
  - `prepare(cell, workspace) -> Prepared { files[], env_overlay }`, where a file is rooted in the
    workspace or in the agent's home (`FileRoot`)
  - `command(cell, workspace) -> CommandSpec { argv, env_overlay, cwd }`
  - `settings_table() -> SettingRow[]`: one row per cell setting, value or name the driver knows,
    `Mapped` (with the harness construct it writes) or `Refused` (with the reason). The driver
    builds its command and files from the same rows. A setting the harness cannot honour is
    refused with `experiment.setting_unsupported`, naming the setting's pointer, never dropped
  - `gateway() -> Option<GatewayEnv { base_url, auth_token, reserved[] }>`: the names of the two
    variables through which the core hands the gateway to the agent. The core sets the values; a
    harness configured by file refers to the names (`prepare`), so no value passes through the
    driver. No default names exist: a driver that names none is refused at run time
  - `state_paths() -> StatePath { env, kind: Dir | File, subpath }[]`: the places the harness
    keeps state under the agent's home. The core makes each per run and sets its variable
  - `census_stream() -> Option<StreamName>`: the raw stream the gateway's census is stored under,
    for a harness with no init event. The core reads the gateway census itself
  - `session(cell, workspace) -> Option<SessionSpec { command, input, close, hooks }>` and
    `session_tracker() -> Option<Box<dyn SessionTracker>>`
  - `stop() -> StopSpec { pre_stop: Option<Action>, signal, grace }`, with
    `Action = Command(CommandSpec) | Exec(CommandSpec) | Http { method, url, body }`. `Exec` runs
    inside the agent's container, with the agent's user, environment and mounts, and may hold the
    placeholder `{session_id}`, which the core fills from the live reader's observed root session
    id (an id outside `[A-Za-z0-9._-]`, or none yet, skips the action and the signal follows)
  - `live_reader() -> Option<Box<dyn TraceReader>>`; `offline_reader(stream) -> Option<Box<dyn TraceReader>>`
    for a stream the core collected (an export)
  - `classify_exit(exit {status, ended, tail}, sessions) -> EndedBy`, `ended` saying whether the
    process exited, hit its deadline or was stopped; `classify_end(exit, sessions, observed)`,
    which the core calls, defaults to `classify_exit` and lets a harness whose exit status covers
    several endings read the live reader's last word or the export
  - `collect(workspace, observed) -> CollectSpec[]`, with
    `CollectSpec = File(glob) | Command(CommandSpec, name) | Exec(CommandSpec, name)`. The core
    runs `Exec` after the agent exited, in a throwaway container over the run's mounts and state
    variables (`candler.role=control`, no network, no gateway name), and archives its stdout as
    the stream `name`
- `TraceReader`
  - `id() -> name@version`, written into the trace header; the version is bumped whenever the
    reader's output changes
  - `framing() -> Lines | Document`; the runner frames the raw stream by it
  - `detect(sample) -> Option<(FormatVersion, Confidence)>`
  - `feed(FrameIn {raw_ref, at?, frame}) -> Event[]` (one line, or the whole document); the core
    supplies the frame's place and its wall time from the raw header's clock
  - `observed() -> Observed { session_ids[], init?, end?, totals? }`: the ids, the init report
    (the tool inventory, labelled by its source: `Harness`, `Gateway` or `Collected`; never an
    empty one), the stream's last word (`Finished`, `Error{kind}` or `Aborted`) and the run
    totals (usage, cost, turns) when the harness reports them
  - `finish() -> Result<Session[], ReaderError>` (root first; children linked by `parent_session`)
- `SessionTracker`: `feed(record: Stream(frame) | Hook(line))` and
  `state() -> PendingState { turns, turn_ended, known, pending[] }`. It holds no clock; the core
  decides when to close. `known` is false until the harness's pending work has been read, and
  again after anything unreadable: unread pending work is never "nothing"
- `SkillTree` (one implementation per driver crate): `archive_command(treatment, source)` and
  `files(treatment, archive)`. Claude Code lays a treatment's skills out under
  `.claude/skills/<name>`, OpenCode under `.opencode/skills/<name>`

Drivers return data and never act on processes; the core runs every command and HTTP call they
describe. The reader is synchronous, so the core owns all async I/O and drivers stay usable as
`dyn` trait objects. A reader can exist without a driver. Missing capabilities are refused at
`resolve`.

**What the core owns and what a driver supplies.** The core names no harness. A driver supplies
the version probe and its parse, the gateway variable names, the state paths, the census source,
the skill layout, the session surface, the stop and collect actions and the exit classification.
The core supplies the values of the gateway and state variables, the hardening, the stop
escalation, the raw archive, the census read at the gateway, the session close rule and every
process.

**The registry.** The binary holds the drivers in one table (`crates/candler/src/drivers.rs`),
keyed by the name a driver gives. Each entry holds the driver, its skill tree, the directory
under `assets/docker` that builds its base image, the streams it collects for offline reading, its
reader and the recogniser of its raw streams. `request.harness` selects the entry before any
image or resolver is built, and an unknown name is refused (`experiment.unknown_harness`), never
run as another harness. `comparable.harness.driver` is `None` for live cells. The label
`candler.cc_version` on a base image holds the harness version, whatever the harness.

**The shared fixture suite.** `candler-harness`, under the feature `conformance`, holds one suite
that every driver crate runs over its recorded streams. It checks that gateway names, state paths,
actions and written files are well formed (no secret in argv, every `{env:NAME}` a written file
refers to is declared, no `pgrep -f`), that a declared capability has what it needs (a session has
a spec, a census has a source), and that an export, where there is one, decides the end.

The core feeds the child's stdout to `live_reader()` whenever the driver returns one, and the ids
that `collect` receives are that reader's `observed()`. A driver whose harness writes its events
elsewhere (OpenCode v2) returns a Lines reader over stdout that emits no events and only reports
the session ids it sees and its last word; `live_events` is then false. The export it collects
goes to `offline_reader`, and the sessions read from the export are authoritative for the trace
and the end. A run the experimenter stopped (job budget, `candler kill`) is classified by the
core, whatever `classify_end` returns (I20).

Drivers are compiled in, one crate each, selected by the experiment at runtime. A third-party
driver is a crate against `candler-harness`, contributed upstream or built in. The trait is part of
the 1.0 freeze.

**Model levels.** An experiment's model level maps, per harness, to a model id plus the provider and
endpoint configuration the driver writes in `prepare`. A level may be a table under `model.*`
(for example a reasoning-effort flag a gateway model requires); each cell's table reaches its
driver's command line.

### OpenCode v2 (second driver)

The driver is written against OpenCode 2.2.7 and accepts exactly that version: `opencode
--version` prints `opencode v2.2.7`, and `parse_version` refuses any other. The binary is one file,
`opencode`, installed only inside a Candler image (`assets/docker/base-opencode`), pinned by
version and by the sha256 of the release archive of each architecture.

- **Command.** `opencode run --standalone --format json --auto -m candler/<model>[#<effort>] -- <prompt>`.
  `--standalone` keeps the session out of a shared background server (without it a run also writes
  a service file with a password). `--auto` is the only permission behaviour, so `bypassPermissions`
  is the one mode accepted. `--` keeps a prompt that starts with `-` a message. `run` has no
  turn, cost or port option.
- **Provider.** `prepare` writes `opencode.json` under `$XDG_CONFIG_HOME/opencode` in the agent's
  home. The provider `candler` (package `@ai-sdk/anthropic`) takes its base URL and key from
  `{env:CANDLER_GATEWAY_URL}/v1` and `{env:CANDLER_GATEWAY_TOKEN}`, which the core sets, so the
  token never enters a file. Each MCP server is written in the V2 shape `mcp.servers.<name>` with
  `codemode` always explicit: MCP code mode is on by default in OpenCode and bundles a server's
  tools into one `execute` tool, so an absent cell value is written as on, and `codemode` is a cell
  value under `mcp.*`, a treatment factor (I7). A server shape the driver cannot translate is
  refused.
- **State.** `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME` and `XDG_STATE_HOME` are
  directories under the agent's home and `OPENCODE_DB` is the file `.xdg/data/oc.db` (SQLite in WAL
  mode); `/tmp/opencode` needs only the noexec tmpfs. OpenCode also reads skills from
  `$HOME/.claude/skills`, so the home is per run.
- **Events.** The live NDJSON has six event kinds (`step_start`, `text`, `tool_use`,
  `step_finish`, `retry`, `error`), every one carrying `sessionID`, the first on the first stdout
  line. A `tool_use` event comes only after the call completed, so tool calls come from the export,
  not the stream. The live reader emits no event: it reports the session id and the last word
  (`error` event type).
- **Export.** `opencode session export --standalone <id>` prints JSON on stdout (`info` with an
  `outcome` of `succeeded`, `interrupted` or `failed`, and `messages`), after any ending, signals
  included. The driver collects it with `CollectSpec::Exec` for the first session id the live reader reports, as the
  stream `export`, framed `Document` and read by `offline_reader`. The export is authoritative for
  the trace and the end; a child session is linked to its parent.
- **Exit codes and signals.** Exit 0 is `succeeded`. Exit 1 covers three endings (a provider or
  agent error, an interrupt through the API, a failure), told apart only by the last `error` event's
  `type` or the export's `outcome`. SIGTERM ends the run in under 100 ms with exit 143 and SIGINT
  with exit 130, killing the server child and the running tool; no escalation is needed. The
  driver classifies from the export: `provider.auth`, `provider.invalid-request` and
  `provider.transport` are gateway errors (R01 §Ends), `provider.no-route` is an agent error, and
  an interrupt or signal is `unknown`, since the core's stop or cap explains it. An exit with no
  trace is never completed. A transport failure retries for about 85 s, so the wall cap ends it.
- **Stop.** The execution runs in a `serve --stdio --port 0` child, so a signal to the client
  alone does not stop the tool. The pre-stop is an `Exec` of `python3 -I -c <script>
  {session_id}`: the script finds the server child by its whole argument vector and its parent
  (`opencode run`), reads its loopback port from `/proc/net/tcp` and its random password from its
  environment, and calls `session.interrupt` through `opencode api`. The run then exits 1 with
  `aborted`. SIGTERM follows, with a 10 s grace. A stop before the first stdout line has no session
  id, and the signal alone runs.
- **Budgets.** There is no cost cap, and `agents.<id>.steps` is not a turn cap (it ends the run
  `succeeded` with a tool-less last request), so `budgets` is false and
  `budget.per_run.{turns, max_cost_usd}` are refused. The same session id also appears in one
  tool-less title request per session, so `info.tokens` counts one more request than the stream.
- **Settings.** The driver's settings table maps what OpenCode can honour and refuses the rest:
  `permission_mode` `bypassPermissions` is `--auto` and every other mode is refused;
  `model.effort` (`low` to `max`, on a Claude opus, sonnet, fable or mythos of 4.6 or later) is a
  config-declared model variant (`xhigh` from 4.7); `tools.allowed` and `tools.disallowed` are
  `permissions` rows per tool name (`Write` and `Edit` are the one action `edit`, so they are listed together or refused; MCP names are
  `<server>_<tool>`), the deny rows last. A name with no OpenCode action is refused.
- **Census.** No event, command or export lists OpenCode's tools. The tool names in the request the
  agent sends to the model are the inventory, so the gateway sidecar records the names of each
  request, and the core takes the first request that carries tools (`census_stream`,
  `census.source = gateway`). Servers are not seen, so `census.mcp_servers` is unrecorded for this
  source, and a run whose requests carry no tools records no inventory. The verdict never takes a
  gateway inventory for a harness's own.
- **Capabilities.** `graceful_stop`, `census` and `execution_outside_child`. Not `session`.
- **Not exercised.** Reasoning events (`--thinking`), a code-mode `execute` call in a recorded run,
  permission prompts without `--auto`, resume, and the interrupt and a mid-tool SIGTERM against a
  real model: the recorded cases come from the scripted fake model.

### Session runner

A headless run (`claude --print <prompt>`) ends at the agent's first final answer, which kills
background tasks, so behaviour between turns (a job finishing, its notification, a monitor event, a
wake-up) never happens in it. **Session mode** is an opt-in way to run a cell: the prompt goes
through stdin and stdin stays open, so a notification starts a further turn.

- **Mode.** `runner.mode` is `headless` or `session`. It has no core default, so an absent value
  runs headless and every cell keeps its digest; an explicit `headless` is a cell of its own.
  `runner.idle_s` (whole seconds or `none`) is refused unless the mode is `session`; in session mode
  a preflight step derives 60 s when the cell gives none. The rule is one function, applied to the
  experiment-wide values and to each cell, so a session cell is the same cell whether `runner.mode`
  is fixed or a factor level. A driver without `capabilities.session` is refused
  (`experiment.session_refused`). The container runs with `docker run -i`.
- **Close rule.** The core closes the input only when all three hold: the agent's latest turn has
  ended, read from stdout (Claude Code's `result` event), which the agent cannot forge; the
  driver's tracker reports that nothing pending could wake the agent (a background task, a monitor,
  a cron, a wake-up) and has read it (`known`); and `runner.idle_s` seconds have passed since the
  session became quiet. A new turn, a turn end, pending work or an unreadable state restarts the
  wait. Idle is never counted mid-turn, and `none` never closes. The rule cuts no behaviour short: it
  never closes while anything is pending, so no notification, wake-up or cron is dropped. After the close
  the core waits for the harness to exit; past the driver's stop grace it stops the harness with
  the normal escalation (`session.close_forced`). A driver that closes by an action instead of end of
  input is refused at run time. The turn cap ends the run when the session's turns exceed
  `budget.per_run.turns`, counted by the core, since a harness's own turn cap ends one turn, not the
  session; the wall and the budget end it as in a headless run.
- **Claude Code.** The command keeps the headless arguments and adds `--input-format stream-json`
  and `--settings`. The pending list comes from the Stop hook payload (`background_tasks`,
  `session_crons`), which is the only source that lists wake-ups and crons; a stream
  `background_tasks_changed` event lists background tasks only. A Stop payload is tied to its turn
  by order and by `prompt_id`, so the state is known only when every turn that needs a Stop payload
  has one. A turn that ends with no Stop payload (a turn cap error, an API error) leaves the state
  unknown, and the session runs to its cap rather than closing.
- **Hooks.** The driver installs observation hooks outside the workspace (a settings file in the
  agent's home, loaded under `--setting-sources project,local`). The hook command appends the Stop
  or SessionEnd payload with the container's clock to a log, prints nothing and always exits 0, so
  it never blocks or decides (I9). The log is archived as the raw stream `hooks`, so the session
  measures can be re-derived (I3). Hooks are never trusted for the turn end.
- **Measured, never filtered.** The agent's own stopping is recorded as `measured[]` paths:
  `session.idle_s`, `session.turns` (turns that ended), `session.yielded_at_ms` (from the runner's
  preparation of the session to the end of the last turn), `session.pending_at_end` (a list of
  `{kind, id}`; absent when it could not be read, never an empty list), `session.end` (`closed`,
  `cap`, `budget`, `harness_exit` or `hooks_incomplete`) and `session.close_forced`; the control
  record a run relied on is `session.control`. `run.num_turns` is the sum over the session's
  results, `run.usage` the sum of their usage and `run.cost_usd` the harness's cumulative cost. A
  session that reaches the wall with an ended turn whose Stop payload was never read is
  `invalid(infrastructure)` with `session.end = hooks_incomplete`: the instrument failed, not the
  agent. An agent that never stops runs to its cap, and that is a result. An end the experimenter
  caused (budget stop, kill) is the core's and records no `session.end`.
- **Controls validate the instrument, never the model.** Control C1 (the agent starts a background
  job and must be woken by its notification: the session must wait) and control C2 (the agent
  must stop: the session must close) run the real harness in the cell's base image, through the
  production gateway sidecar, against a scripted fake model that speaks TLS and spends no tokens.
  The fake model is embedded in the binary as an asset and built as `candler-fakemodel:src-<digest>`.
  The check runs when a job opens, once and before any run, if any fresh cell is a session cell;
  each cell's mode is known only after planning, because `runner.mode` can be a factor level, and a
  job with no session cell never runs a control. Control runs go to a scratch store and never
  reach `runs/`. The verdict is a pure function over the control run's own manifest: the turns,
  `session.end`, `ended_by`, the recorded `idle_s`, C1 still waiting when its job ends, and the
  harness exiting between `idle_s` and `idle_s + 1 s` after the turn end. The controls prove the close rule at the control's own short `idle_s` only; the
  run-loop guards cover the other values. The record is keyed by the digest of the base image's
  build sources and a control digest (the fake model's files, the control's parameters and
  Candler's version, which holds the verdict and the close rule), so a changed script, parameter
  or release re-runs the controls with no manual version. It is written once and never rewritten
  (I11), and a failing verdict is not written. A failure refuses the whole job before any run with
  `jobs.session_controls_failed` (environment side), naming the harness version and the failed
  control. Every session run records `session.control` (the record's path and digest) in
  `measured[]`, outside the cell digest. A model's behaviour never refuses a run.
- **Tool inventory.** The census (`census.tools`, `census.init_tools`) is captured per run; when
  `census.init_tools` differs between arms of one stratum that no declared `tools.*` or `mcp.*`
  factor explains, that stratum is refused (I7). Claude Code reports the list at each turn's start
  (`init` is emitted again every turn), so the core takes the first init that lists tools.

## Plug-ins

### Plug-in manifest (static; on the `schemas@` line)

| Field | Meaning |
|---|---|
| `name`, `version` | Must equal what `describe` returns |
| `protocol` | Protocol major.minor it speaks |
| `command` | argv, relative to the plug-in directory |
| `environment` | `{kind: uv \| none, project, lock}`; the core builds one environment per lock digest and reuses it, and runs `command` inside it (`VIRTUAL_ENV` set, its `bin/` first on `PATH`) |

The core reads manifests without running anything. Plug-ins come from the directories of
`plugins.dirs[]`, which `evaluate` reads; each plug-in directory holds one manifest, named
`plugin.toml` or `candler-plugin.toml`, and a directory with both, with neither, or with a manifest
only in a subdirectory is refused. A suite names an evaluator as `name@range`.

**What the core knows of a domain.** Nothing (I1). An evaluator reports the packages of its own
environment through `describe`, in `environment.packages{}`. The agent's environment is declared
by the experiment: its requirements, the files it prunes and the import packages it records
(§Experiment file). The core records a domain's facts only under names the experiment gives, as
`provenance.packages.<import>.*` (§Run manifest), and no plug-in adds a domain field to the core.
Every field of the experiment file and the run manifest that these rules add is optional and
absent when unset, so `experiment/1` and `run-manifest/1` keep their tags and the cell-digest
function is unchanged.

### Suite file (on the `schemas@` line)

Each expectation: `id`, `group`, `role` (gate / quality / descriptive), `evaluator` (`name@range`),
`kind`, `params`, `depends_on[]`, `applies_to` (arms or conditions), `deadline_ms?`.

- The core decides `applies_to`: an expectation that does not apply to a run is recorded
  `not_applicable` with cause `{reason: not_applicable_to_arm}` and is not sent.
- The core decides dependency `blocked`: when a dependency is `fail` or `blocked`, the dependant is
  recorded `blocked` with cause `{expectation: id}` and is not sent.
- Dependencies graded by the same evaluator go in the same request (C6 uses C1 and C5); outcomes of
  dependencies graded elsewhere are passed in `prior[]`.
- Parameters are not templates. Evaluators read run facts from `metadata` and `inputs` (today's
  `{domain_name}` and `{servers_dir}` substitutions become declared needs).

## Evaluator protocol

JSON-RPC 2.0, one message per line on the evaluator's stdin and stdout. stderr is free text, stored
as the evaluator's log.

| Message | Direction | Kind | Content |
|---|---|---|---|
| `describe` | core → evaluator | request | none |
| ↳ result | | response | `protocol`, `name`, `version`, `instruments[] {name, version, digest, dirty}`, `environment {lock_digest, packages{}}`, `kind` (deterministic / probabilistic), `kinds[] {kind, params_schema, needs, default_deadline_ms}`, `reuse` (reserved) |
| `evaluate` | core → evaluator | request | `context` (below), `items[] {expectation_id, kind, params, deadline_ms}`, `prior[]`, `scratch` |
| `outcome` | evaluator → core | notification | one outcome record, as soon as known |
| `progress`, `log` | evaluator → core | notification | free payload, forwarded into job events |
| ↳ result | | response | `{reported: n}` |
| `cancel` | core → evaluator | notification | request id; the evaluator answers `evaluate` with error `-32800` (request cancelled) |

**`needs`** = `{trace: bool, artifacts[] (run-relative globs), snapshots[] (kinds), inputs[]
(kinds), metadata[] (run-manifest field paths)}`. `resolve` checks every need the core can meet
without a run; an `inputs` need for `build_inputs` is refused there, by name. A need a run cannot
meet, such as a kind the run holds in neither list, is refused for that run, by name.

**Run context:** `run` (key); `root` (a read-only, workspace-shaped view of the run: content-
addressed blobs materialised under their workspace paths by hard links or clones, so `domain.py`
sits beside `problems.py`); `trace` (normalised trace file); `raw[]`; `artifacts[] {path
(run-relative), abs, sha256}`; `snapshots[] {kind, path, digest}`; `inputs[] {kind, path, digest}`
(a need for kind K is served from `comparable.inputs` or from `evaluation_inputs`, with `path` a
read-only tree: its files read-only, its directories writable, as under `outputs/`; an input only
the evaluators see never enters the isolation, I8); `metadata` (the declared manifest fields). Absolute paths are valid where the
evaluator runs (host or container) and exist only in requests. `scratch` is the evaluator's; the run
store is read-only to it.

**Digests of a request.** Both are `sha256` over RFC 8785 JSON, so any language recomputes them.

- `context_digest` covers the projection `{artifacts: [{path, sha256}], snapshots: [{kind,
  digest}], inputs: [{kind, digest}], metadata: {<field path>: value}}`: lists sorted by `path` or
  by `(kind, digest)`, every key present (lists possibly empty). Absolute paths, `root`, `trace`,
  `raw[]` and `scratch` are outside it: they say where the run is, not what was graded.
- `suite_entry_digest` covers `{kind, params, role, depends_on, applies_to?}` of one suite entry:
  `depends_on` sorted, `applies_to` omitted when absent. `id`, `group`, `evaluator` and
  `deadline_ms` are outside it, so renaming an expectation or moving its deadline keeps its digest.

**Lifetime.** The core starts one evaluator process per request, in its own process group, sends
`describe` then `evaluate`, and keeps stdin open until the `evaluate` result arrives; it then closes
stdin. End of input means "finish in-flight work, then exit". On cancel: send `cancel`, close stdin,
then escalate as for any child. Reusing a worker across runs later is a core setting plus `reuse`;
the protocol does not change.

**Deadlines and isolation per expectation.** Each item carries `deadline_ms` (suite value, else the
kind's default; 60 s for the HTN checkers, as today). An evaluator may run children inside its own
request: the Python helper imports the checker libraries once, then forks one child per item and
enforces that item's deadline, so a hanging or exiting agent `domain.py` costs one expectation, not
the run, and planner state never leaks between expectations. The core enforces the request deadline
(the sum of item deadlines plus a margin) by killing the evaluator's process group.

**Stdout guard.** The helper duplicates file descriptor 1 to a private protocol descriptor and
points descriptor 1 at stderr before any import, so C extensions, `os.write(1, …)` and subprocesses
cannot reach the protocol pipe. On the core side, a line that is not JSON is logged and counted and
never aborts a request; a JSON line that is not a valid protocol message is a malformed message.

**Version.** The core accepts a plug-in whose `protocol` has its own major and a minor no newer than
its own; a mismatch fails `resolve` and names the side to upgrade. `describe` must match the
manifest's name and version.

**Failures.** Every item the evaluator did not report becomes `not_measured` with cause
`{reason: instrument_error}` (or `{reason: deadline}`), never `fail`. Outcomes already received are
kept.

## Data Model

```mermaid
erDiagram
  EXPERIMENT ||--o{ CELL : "expands into"
  CELL ||--o{ RUN : "replicate x attempt"
  JOB ||--o{ RUN : executes
  JOB ||--o{ JOB_EVENT : emits
  RUN ||--o{ SESSION : produced
  SESSION ||--o{ EVENT : ordered
  SESSION ||--o{ SESSION : "spawned (subagents)"
  RUN ||--o{ ARTIFACT : "left behind"
  RUN ||--o{ CATALOG_SNAPSHOT : targeted
  RUN ||--|| MANIFEST : provenance
  PLUGIN ||--o{ EVALUATOR_VERSION : provides
  SUITE ||--o{ EXPECTATION : declares
  EXPECTATION }o--|| PLUGIN : "evaluated by"
  RUN ||--o{ REQUEST : "evaluated in"
  REQUEST ||--o{ OUTCOME : streams
  EXPECTATION ||--o{ OUTCOME : graded
```

### Run store (files-first)

| Path | Holds |
|---|---|
| `runs/<run key>/` | `manifest.json`, `run.done`, `raw/` (`raw-frames/1`), `trace.jsonl` (`normalised-trace/1`), `tree.json` (`run-tree/1`: `{schema, entries[]}`, each file entry `{path, sha256, executable}` and each empty directory its own entry, sorted by path), and under container isolation the kept `workspace/` and the per-run `home/` |
| `blobs/<sha256>` | Artifact contents, once |
| `outcomes/<run key>/<evaluator@version>/<request id>.jsonl` | One request's outcomes; written as `.partial`, renamed when the result arrives; one writer, no locks |
| `jobs/<job id>/` | `job.json`, `events.jsonl`, `children/`, `lock` |
| `experiments/<digest>/lock` | One supervisor per experiment |

An index can always be rebuilt from these files. Stored paths are relative to the run root (I17).

- **Marker.** `store.json` `{schema: candler-store/1}` at the root; a directory without it is not
  opened as a store. The default root is `<project root>/.candler/store`; outside any project it is
  `$XDG_STATE_HOME/candler/store`, else `~/.local/state/candler/store`, beside the slot files, so a
  command finds the same store from any working directory.
- **Index.** `index/runs.jsonl` and `index/outcomes.jsonl` are a cache, internal to the core:
  appended under `index/lock`, rewritten atomically by `rebuild_index`, and read tolerating a torn
  last line.
- **Blobs** are written to a temporary file, verified against their digest, renamed, and made
  read-only (0444). A blob that exists is never rewritten.
- **Outcome files** are written as `.partial`, fsynced, and renamed when the request ends. Readers
  never see a `.partial`; a final file is never replaced.
- **Runs.** `raw/` holds finished `raw-frames/1` files (`<stream>.jsonl.gz`); the store does no
  framing itself. `run.done` is written last.

### Run manifest (`run-manifest/1`, on the `schemas@` line)

`{run, experiment, factors[], comparable, sources[], secrets[], measured[], started_at?, ended_at?,
host?, job?, integrity, import?, evaluation_inputs[]?}`.

- **`comparable`** `{values[] {path, value?}, harness {name, version, driver?}, treatment,
  snapshots[], inputs[], environments[]}`: every resolved value by field path (isolation under
  `isolation.*`), the harness, the treatment reference (name, tag and commit), the catalog snapshot
  and input digests, and the plug-in environment lock digests. A value without `value` was
  `unrecorded`. Every list rises strictly by its key, so it is sorted and holds no duplicate:
  `values` by path, `snapshots` and `inputs` by kind then digest, `environments` by plug-in then
  lock digest. A reader refuses a list that does not.
  `Comparable::from_resolved` builds it from the resolved values: the roots `harness`, `treatment`,
  `snapshots`, `inputs` and `environments` fill the typed fields, every other path goes to
  `values`. Operational settings (config keys, explicit paths) are resolved apart and never enter
  it, and a string value that is an absolute path is refused: a file enters a cell as an `inputs[]`
  digest. The per-run caps are cell values under `budget.per_run.*`; the job caps (every other
  `budget.*` path) and `replicates` are operational settings, which `resolve` classifies by path. A
  snapshot's digest is that of its RFC 8785 form without `captured_at`; an imported snapshot's
  digest is the bare sha256 its legacy provenance recorded, which a live snapshot's never equals.
- **The treatment reference** `{name, tag?, commit?}` holds no source. A live run's carries the
  commit its source resolved to whenever the treatment names a tag or a commit; an imported run's
  carries none, since the legacy folder records none.
- **Inputs.** Every file of an input is a store blob under its sha256. The input's list
  `[{path, sha256}]`, sorted by `path` and relative to the input's root (a file is a list of one
  entry, named by its file name), is a blob whose bytes are its RFC 8785 form, and the input's
  digest is that blob's key, so any language recomputes it. `comparable.inputs[]` holds one
  `{kind, digest}` for each build input and each agent input, sorted by kind then digest (an
  imported run may repeat a kind). An input for the evaluators is in `evaluation_inputs[] {kind,
  digest}`, sorted by kind and digest, outside the hash, and its blobs are in the store. What the agent's container can see (the argv, the mounts,
  the workspace, `tree.json` and `comparable`) never names it. The job record may hold its
  `{kind, digest}`, never its content, and the job directory is never mounted. The mount
  points of agent inputs are not outputs, and an output that matches an input only by being empty
  is no leak.
- **Cell digest** = `sha256` of the RFC 8785 form of `comparable`, computed by one function
  (`RunManifest::cell_digest`). Changing the digest an existing manifest yields requires a new
  schema tag.
- **Outside the hash:** `sources[] {path, source}` (where each value came from, I12: one entry for
  every path in `values` and for every typed root present, or a path under it such as
  `harness.version`; settings have no entry, since they never enter the manifest; moving a value
  from the command line to the experiment file keeps its cell; a value a preflight step derived
  carries the step and, in `from`, the location it read, such as a treatment's `source`, as an
  absolute, lexically normalised path), `evaluation_inputs[] {kind, digest}`, `secrets[] {name, present}` (I19),
  `measured[] {path, value, source}` (values observed during the run, the run's end under
  `run.ended_by` and `run.exit`), `deviations[]`, timestamps, host, job id and the attempt. This nuisance set is part of the schema, not an implementer's choice. Stored paths are
  run-relative (I17).
- **`factors[]`** names the field paths that vary across the experiment's cells; **`experiment`** is
  the digest of the sorted cell digests. One function computes each.
- **`integrity`** `{kind: valid}` or `{kind: invalid, reason}`. **`import`** records how an imported
  run was read: format label, base commit, the legacy integrity block, `sources[] {path,
  sha256}`, each stored as a blob, and `requests[] {evaluator, request}`, the outcome files it
  wrote, by which a diff finds the imported reference.

**Provenance.** The run records where its measurement came from as `measured[]` entries under
`provenance.`. A value the build or the run cannot know is left out, never recorded as `false` or
as an empty string. A verdict reads three of them (the gated paths); the rest describe the run.

| Path | Meaning | Gates a stratum |
|---|---|---|
| `provenance.eval_infra.commit` | The commit the binary was built from, 40 hexadecimal digits | Yes: every arm records it as a non-empty string, and the arms agree |
| `provenance.eval_infra.dirty` | Whether the sources the binary is built from differed from that commit | Yes: every arm records it, and it is `false` |
| `provenance.sandbox_contract` | The container contract (`name@version`) the agent ran under | Yes: every arm records it, and the arms agree |
| `provenance.eval_infra.tag` | The release tag of the build: the nearest `v<digit>` tag in a git checkout, else `v<version>` | No |
| `provenance.eval_infra.commits_ahead` | The commits between the tag and the commit; known from a git checkout only | No |
| `provenance.packages.<import>.{distribution,version}` | The distribution and version of each import package named in `isolation.record_packages`, read from the env image | No |

A gated path an arm did not record is refused by name; it is never equal to another arm's, and
never equal to itself. A git build and a packaged build of one commit differ in `tag` and
`commits_ahead` and still pool. The packages gate nothing: the env lock is already a cell value,
and the analysis reads them. The build's identity comes from the first origin that has one:

1. a git checkout: `git rev-parse HEAD`, in full; `dirty` is whether `git status` finds a
   difference in the files the binary is built from, and stays unrecorded when `git status`
   fails;
2. a packaged crate's `.cargo_vcs_info.json`: its `git.sha1`, with `dirty` true only when the file
   holds it and false otherwise, and the tag `v<version>`. The file wins over a git checkout
   that surrounds the package;
3. the packager's `CANDLER_EVAL_INFRA_TAG`, `CANDLER_EVAL_INFRA_COMMIT` and
   `CANDLER_EVAL_INFRA_DIRTY`, inputs to the build: the build records only what it can prove,
   parsed strictly. A commit is 40 lowercase hexadecimal digits and `dirty` is `true` or
   `false`; anything else, or a name left unset, is unrecorded, and the verdict refuses it. The
   tag defaults to `v<version>`;
4. otherwise the tag `v<version>` alone, with `commit` and `dirty` unrecorded.

**Imported strata.** An arm is imported if and only if its manifest holds `import`. A stratum that
mixes imported and live arms is refused, naming `import`. A stratum of imported arms swaps the
gate on `measured[]` provenance for the import's own record, since the legacy folder recorded the
build in `comparable.values` and no run measured it. The stratum is refused unless every arm
records the digest of its import rules and the arms agree (`import.rules`; an import without
rules is refused), and every arm records `provenance.eval_infra.commit` (a non-empty string),
`provenance.eval_infra.dirty` and `provenance.sandbox_contract` in `comparable.values` with equal
values. The verdict compares these three itself, so no declared factor pools arms of two legacy
builds. A dirty legacy build does not refuse: arms that agree on commit, flag, contract and rules
pool, on the assumption that one legacy launch is one checkout. Live strata keep the gate above.

**Session and census measures.** A session run records the `session.*` values of §Session runner
and `session.control`. A census whose source is not the harness's own init event also records
`census.source` (`gateway` or `collected`); a record without it is the harness's own, and a verdict
never compares an inventory with one of another source. `census.mcp_servers` is recorded only when
the source sees servers.

### Normalised trace model (`normalised-trace/1`, on the `schemas@` line)

`Session {id, source{harness, harness_version, reader, reader_version}, model, started_at?,
ended_at?, ended_by, parent_session?, raw_ref}`; `Event {seq, at?, kind, span?, raw_ref, ext}` with
kinds `user_message`, `assistant_message`, `tool_call` (`origin`, `argv?`, `wrapped_calls[]?` for a
call that bundles several MCP calls), `tool_result` (`is_error`, `denied?`), `notification`, `usage`
(`scope: message | session_total`). `at` comes from the harness when it emits one, else from the
frame time in the raw file, else it is absent.

- **File.** JSONL: a header `{schema, reader: name@version, raw[]}`; then the sessions, root first and
  each parent before its children; then the events `{session, event}` in feed order.
- `seq` counts from 0 in each session, without gaps. `span` is the harness's id pairing a call with
  its result (Claude Code's `tool_use` id). `raw_ref` is `{stream, frame}`.
- **Session ids.** A child session's id is `<parent id>/<tool_use id>`; ids hold no `#` or white
  space, so they read as the session part of an `event:` evidence ref.
- **Usage.** A message split over several lines reports its usage once, on the first line carrying
  its message id. A `session_total` is the session's running total, so a later one supersedes an
  earlier one. For Claude Code it sums the usage of the session's result lines; it does not
  reconcile with the result's `modelUsage`, which accumulates across the whole process, and only
  the cost agrees between the two. An unknown line becomes a `notification` with the line in
  `ext`, and readers skip kinds they do not know (I18).
- **Reproducibility.** The trace is a function of the raw files and the reader at its version (I3):
  feeding the same frames, however they are chunked, gives the same file byte for byte.

### Wire schema: outcome record

| Field | Type | Notes |
|---|---|---|
| `run` | run key | |
| `expectation_id` | string | must exist in the suite |
| `role` | gate / quality / descriptive | copied from the suite entry when graded |
| `evaluator` | `name@version` | |
| `instruments[]` | `{name, version, digest, dirty}` | I6: a `gate` or `quality` record has no dirty instrument |
| `suite_entry_digest` | hash | of the expectation's `kind`, `params`, `role`, `applies_to`, `depends_on` |
| `context_digest` | hash | of the artifacts, snapshots, inputs and metadata sent |
| `request_id` | id | |
| `status` | `pass` / `fail` / `blocked` / `not_measured` / `not_applicable` | five-state algebra |
| `value?` | number / fraction `{num, den}` / count / text / named counts | a fraction carries its counts |
| `weight?` | `{num, den}`, `1 ≤ den`, `num ≤ den`; `0/0` only on a `pass` with a legacy cause | contribution to a rate; defaults: pass 1/1, fail 0/1, blocked 0/1; a `pass` has `num = den`, a `fail` `num < den`, a `blocked` `num = 0`; absent on `not_measured` and `not_applicable`, which have no effective weight |
| `probability?` | `{p, calibration_ref}`, `p` in [0, 1] | reserved; never set by deterministic evaluators, so never on a `gate` or `quality` record (I5) |
| `evidence[]` | `{ref?, note?}`, at least one | ref = `event:<session>#<seq>` or `path:<run-relative path>` |
| `cause?` | `{expectation: id}` / `{artifact: path}` / `{reason: code}` / `{legacy: code}` (deprecated) | required for `blocked`, `not_measured`, `not_applicable`; absent on `pass` and `fail` except `legacy` |

**Identity key:** `(run, expectation_id, evaluator@version, instruments digest, suite_entry_digest,
context_digest)`.

**Rates.** Per role, a rate is Σ `weight.num` / Σ `weight.den` over the `pass`, `fail` and `blocked`
outcomes, summed per expectation, never per eval. `blocked` is a failure, attributed to its cause:
the agent under-produced, and that must not vanish. `not_measured` and `not_applicable` are excluded
and always reported as counts with their causes.

**Selection rule (analysis, `show`, `diff`).** An analysis selects one evaluator version per
expectation; mixing versions is refused unless asked for. Among outcomes with the same identity key,
the latest complete request wins. Differing outcomes under one key, other than an
`instrument_error` or `deadline` replaced by a measured one, are reported as I4 violations. Roles
and weights are read from the outcomes, never from today's suite.

### Analysis statistics

`analyze` reports Candler's default statistics. The unit of replication is the run, and an arm is a
cell: the outcomes of one run share the agent's behaviour, so they are not independent.

- **Rate.** Per role, Σnum/Σden as defined above, reported with its counts and with no interval,
  because an interval over correlated outcomes is falsely narrow.
- **Interval.** The Wilson 95 % interval, per (arm, expectation), on k passes over n graded runs. It
  applies only where every weight is 0/1; otherwise it is omitted, with the reason `weighted`. Wilson
  keeps its coverage at small n and at k = 0 or k = n, where the Wald interval fails. Its exact
  coverage dips below 95 % at small n (0.9185 at n = 5, p = 0.1) and stays at or above 0.91 for the
  sizes replicates give; the implementation equals a second engine's table.
- **Comparison.** For every pair of cells in one comparable stratum that differ in exactly one
  declared factor, per expectation: Fisher's exact test (two-sided; the p-value sums the tables no
  more likely than the observed one, within a relative tolerance of 1e-7, as R's `fisher.test`
  does) and the difference with its Newcombe interval, the hybrid score interval, consistent with
  Wilson. Fisher's test needs no large-sample condition, so it is valid at the few runs a replicate
  count gives.
- **Multiplicity.** Holm's adjustment runs over every comparison and expectation of one role in one
  analysis. It controls the family-wise error rate with no independence assumption.
- **What a run counts for.** An invalid run is in neither k nor n; it is reported per arm with its
  reason, and an end the experimenter caused is never charged to an arm. A pair the stratum verdict
  refuses is reported as refused, with its cause and its rates.
- **Constants.** α is 0.05, recorded with the source `core_default`; there is no option.

These are Candler's defaults, not the analysis: every count is carried, so an author can analyse the
same results another way. `docs/statistics.md` says what each method answers, where it stops being
valid and what Candler does not claim.

### Wire schema: tool-catalog snapshot

What the MCP servers a run targets actually offered, captured per run by a catalog provider. HTN's
Tier D grades a mapping against it, the run manifest records its digest as a comparability field
(I7), and evaluators receive it through `needs.snapshots[]`.

`schema: "tool-catalog-snapshot/1"`, `captured_at`, `provider@version`, `environment?` (string →
string: versions the capture depended on), `ext?` (opaque, provider-specific), `servers[] {name,
source_path, source_digest, transport (stdio / http / sse), tools[]}`. Tools are in MCP
`tools/list` shape, camelCase kept: `name`, `inputSchema`, `title?`, `description?`,
`outputSchema?`, `annotations?`, and every other key preserved as given. HTN's
`tool-catalog-snapshot/0` is this shape with `servers_dir` and `stubbed_modules` at the top level;
its capture script and loader move them under `ext`.

Server names are unique within a snapshot and tool names within a server, compared byte for
byte; both are non-empty. A reader keeps every key of a tool, `ext` and `environment`, and every
value, except: keys not defined for a snapshot or a server are dropped; of repeated keys the last
is kept; object keys are sorted; numbers are held as `i64`, `u64` or `f64`, so an integer beyond
the 64-bit range becomes a float, a decimal beyond `f64` precision is rounded and a magnitude
beyond `f64` is refused; and text nested deeper than about 120 levels is refused.

### Wire encodings

The rules every wire type of the `schemas@` line follows.

| Topic | Rule |
|---|---|
| Unions | Internally tagged on `kind`, with the payload named after the variant: `{"kind": "number", "number": 1.46}`, `{"kind": "count", "count": 14}`, `{"kind": "fraction", "num": 0, "den": 14}`, `{"kind": "text", "text": "…"}`, `{"kind": "counts", "counts": {"measured": 3}}`. A value fraction has `den ≥ 1`; a number is finite (no NaN or infinity); counts are 64-bit unsigned. |
| Causes | One-key objects, as in the outcome record table. Reason codes match `^[a-z][a-z0-9_]*$`; the core reserves `not_applicable_to_arm`, `instrument_error` and `deadline`. |
| Legacy marker | `{legacy: code}` marks a record whose status, weight or cause the other causes cannot state exactly, such as the zero weight `0/0` of a gate pass in the golden corpus; the code says which case (`zero_weight`, `no_replay_target`, `zero_mappings`, `unrecorded`). It is deprecated (`deprecated: true` in the schema, `#[deprecated]` in Rust), written only by the legacy importer and refused from evaluators. It is the only cause a `pass` or `fail` may carry, and a `0/0` weight, allowed only on a `pass`, requires it. It is removed at the first schema major after M2a's parity is signed off. |
| Absent vs null | An optional field is omitted when absent. `null` is never written, and both the schema and the Rust reader reject it: the generator strips the `null` type schemars gives `Option`, and record deserialisation refuses an explicit `null`. Collections are always present, possibly empty. |
| Unknown fields | Schemas are strict (`additionalProperties: false`): they define what a writer at that version emits. Readers of the records Candler writes ignore unknown fields, so that an older reader reads a newer record. Readers of the three author documents (experiment, suite, plug-in manifest) refuse an unknown field with `<surface>.unknown_field` and its pointer (§Refusals). Every reader reads objects only, never a struct written as an array. MCP tool objects, `inputSchema`, `outputSchema` and `ext` are open. |
| Run key | `{"cell": "sha256:<hex>", "replicate": n, "attempt": m}` in records, both counters from 1. Paths and `CANDLER_RUN` use `<cell hex>-r<n>-a<m>`. |
| Digests | `sha256:` + 64 lowercase hex; blob paths use the bare hex. An instrument digest may also be `git:<40 or 64 hex>`, a commit of a clean tree, valid only with `dirty: false`. Digests of JSON values (`suite_entry_digest`, `context_digest`, the cell digest) are taken over RFC 8785 canonical JSON. |
| `name@version` | Split at the last `@`; the name has no `@` or whitespace. Evaluator and instrument versions are SemVer 2.0; a catalog provider's version is a free token `[0-9A-Za-z.+-]+`. |
| Ids, paths, times | `request_id`: UUIDv7, lowercase hyphenated. `expectation_id`: `^[A-Za-z0-9][A-Za-z0-9._-]*$`. Run-relative paths: POSIX, non-empty, no leading `/`, no empty, `.` or `..` segment, no `\`. Timestamps: RFC 3339 with an uppercase `T`, and `Z` or a `±HH:MM` offset, naming a real instant, kept as written. |
| Evidence refs | `event:<session-id>#<seq>`: the session id is non-empty, without `#` or white space; `seq` is a decimal without leading zeros (`0` allowed). `path:<relpath>` optionally followed by `#L<n>` or `#L<n>-<m>`: lines count from 1, `n ≤ m`, and the path holds no `#`. |
| Version marker | A standalone document (the catalog snapshot) carries its `schema` tag. Records that travel inside versioned messages or files (the outcome record) carry none. |
| Schema files | JSON Schema 2020-12, generated by `schemars` from the structs, one self-contained file per root in `crates/candler-schemas/schema/<name>.schema.json`, `$id` `urn:candler:schemas:<name>`, no crate version inside. Cross-field rules are `if`/`then` clauses; rules JSON Schema cannot state (`num ≤ den`, the pass and fail weight rules, unique names, 64- and 32-bit integer limits) are enforced in Rust and stated in a `$comment`. Patterns end with `(?![\s\S])` in place of `$`, and spell white space as Unicode's `White_Space` set plus the byte-order mark in place of `\s`: Python's `re`, which Python `jsonschema` uses, lets `$` match before a final newline and gives `\s` a different set, so only this form reads alike under ECMA 262, Rust and Python. The regexes this document writes with `$` (reason codes, ids, field paths) are published in this form. A test refuses any published pattern that uses `$` or `.` outside a character class, or a shorthand class or assertion (`\s`, `\d`, `\w`, `\b`, `\A`, `\Z`, `\p` and their negations) anywhere but in the end anchor, and requires every pattern to end with the anchor except one inside an `if`, such as the prefix `^git:`, which only tells variants apart; the other refusals still apply to it. `///` docs become `description`s, so a doc edit on a wire type is a schema patch. |
| Drift check | `tests/schema_drift.rs` compares every root in `generated_schemas()` byte for byte with its committed file; `CANDLER_BLESS_SCHEMAS=1` rewrites them. |
| Rust shape | Wire structs are `#[non_exhaustive]`, built through validating builders and deserialised through `try_from` from a JSON object only (never an array), so a record is valid however it was made and a new optional field is a minor in Rust as in the schema. Evolving enums are `#[non_exhaustive]`; `Status` and `Role` are closed. `serde_json` has `float_roundtrip`. JSON text holding an unpaired surrogate escape is refused by the Rust reader. |
| Source files | Source says what is: comments, docs, `#[deprecated]` notes and `$comment`s carry no history, origin or meta-discourse. Reasons live in `__design__/` and `__reports__/`. |

### The `schemas@` line holds

Outcome record · catalog snapshot · raw frames · normalised trace (header, session, event records)
· workspace tree (`run-tree/1`) · child record · evaluator messages (`describe` result, `evaluate`
params, notifications, and the request side: method names, `{reported}` and `-32800`) · plug-in manifest · suite file · run manifest · job record, event envelope
and event payloads · experiment file. All are generated from `candler-schemas`.

## Key Flows

### Live: run, then evaluate

```mermaid
sequenceDiagram
  participant S as Surface
  participant A as API
  participant J as Job supervisor
  participant D as Harness driver
  participant P as Process supervisor
  S->>A: resolve(experiment + overrides)
  A->>D: version_probe, capabilities
  A-->>S: resolved experiment, sources, preflight
  S->>A: run(resolved, budget)
  A->>J: start detached job (experiment lock)
  J-->>S: job id
  loop each cell x replicate (resumable)
    J->>D: prepare, command, stop, live_reader
    J->>P: take slot, spawn(CommandSpec) in isolation, record child
    P-->>J: framed stdout -> reader -> events, session ids
    P-->>J: exit status
    J->>P: stray sweep
    J->>D: classify_exit(status, tail, sessions) -> ended_by
    J->>D: collect(workspace, observed)
    J->>J: collect, census, ingest, run.done
  end
  S->>A: evaluate(suite, the job's runs)
```

### Evaluation of one run by one evaluator

```mermaid
sequenceDiagram
  participant E as Evaluation engine
  participant P as Process supervisor
  participant V as Evaluator (helper)
  participant C as Forked child per item
  E->>E: applies_to, dependency blocked, build context
  E->>P: take slot, spawn(manifest command), request deadline
  E->>V: describe
  V-->>E: protocol, versions, instruments
  E->>V: evaluate(context, items, prior, scratch)
  loop each item
    V->>C: fork after imports, item deadline
    C-->>V: result or timeout
    V-->>E: outcome
  end
  V-->>E: result {reported: n}
  E->>V: close stdin
  V-->>P: exit
  E->>E: unreported -> not_measured(instrument_error); rename .partial
```

Post-hoc, legacy and regrade flows: `ingest → evaluate → analyze` on disk. A regrade writes new
request files under the new evaluator version; `diff` compares versions.

## Contracts & Invariants

### Other interfaces (signatures only)

- `CatalogProvider.snapshot(cell) -> catalog_snapshot`
- `Isolation`, one container per run: `plan(cell, workspace) -> ContainerPlan`;
  `run_request(plan, command, marker, run key, outputs) -> IsolatedRequest` (the spawn request,
  with the run's stop hook attached, and the handle that reports the end facts);
  `collect_and_destroy(plan, workspace, exit, run key) -> ContainerOutcome` (the final inspect,
  then the destroy through the guard). A long-lived handle (`create`, `exec`, `inject`, a
  whole-container `census`, `destroy`) is M5's, beside this one.
- `LegacyImporter.import(results_folder) -> runs[]`

### Invariants

| # | Invariant |
|---|---|
| I1 | The core never names a domain. |
| I2 | A status is never projected onto a number. Rates always carry their counts. |
| I3 | The normalised trace is a pure function of `(framed raw, reader@version)`; the raw is kept. |
| I4 | A deterministic evaluator is a pure function of `(context, params, evaluator@version, instruments)`. A regrade of unchanged inputs reproduces `(status, value, weight, cause)` exactly; evidence text is not compared. |
| I5 | Only deterministic evaluators may hold the `gate` or `quality` role. |
| I6 | **Instrument pinning.** A checker shared by the treatment and the evaluator is a library with its own version. The evaluator depends on a pinned version and reports it in `describe` with a content digest and a dirty flag; every outcome records them. Dirty instruments cannot grade `gate` or `quality` expectations. One experiment grades all its arms with one instrument version unless it says otherwise. |
| I7 | Arms differ only in declared treatment factors. The diff of comparability fields proves it. |
| I8 | Ground truth never exists inside the isolation boundary, and no surface exposes it while its run is live. |
| I9 | Observation never alters behaviour. |
| I10 | An invalid run is never a failure. Invalid runs are reported per arm, with a reason. |
| I11 | Outcomes, events and run attempts are append-only. A sealed record (a run with `run.done`, a committed outcome file, a written event) is never replaced or deleted; what never sealed, such as a `.partial` or an unfinished import, is not a record. |
| I12 | Every resolved argument records its source. Nothing is defaulted silently. |
| I13 | A `run` without a budget is refused on the MCP surface. |
| I14 | One front door, one environment. Plug-in environments are provisioned by the core from their manifests. Relative paths given as arguments resolve against the caller's working directory; relative paths written in a config file resolve against that file's directory. |
| I15 | The core supervises every process it starts, and reaps it and its descendants. Drivers never act on processes. An evaluator may run children only inside its own request; they die with it. |
| I16 | Nothing of Candler runs when no job runs. |
| I17 | Every path-typed field of a stored record is relative: to the run root, or, for what an import read, to the imported cell or folder. Content a harness wrote is kept as written (I3), so a raw stream or a trace event's `ext` may hold absolute paths. |
| I18 | Event and message consumers skip kinds they do not know. |
| I19 | Secret values are never written to the store, a job directory, an event or a log. |
| I20 | An end caused by the experimenter (job budget, kill, lost supervisor) is never charged to an arm. |
| I21 | Analysis reads roles and weights from outcomes, never from the current suite. |
| I22 | The core never signals a process group or pid it did not spawn and re-verify. |

### Error model

| Failure | Surfaces as |
|---|---|
| Reader cannot parse, or the format version is unknown | Run `invalid` (`reader_error` / `format_unknown`); raw kept |
| Harness version outside the supported range, a missing capability, an unmet `need`, a missing secret | `resolve` fails; no run starts |
| Plug-in protocol major differs, or `describe` disagrees with the manifest | `resolve` fails, naming the side to fix |
| Artifact the agent never produced | The evaluator emits `blocked` with cause `{artifact}`, or `fail` when the expectation is about that artifact's existence |
| Artifact missing because collection failed | Run `invalid` (`collection_error`) |
| Evaluator crash, item or request deadline, malformed message, outcome for an item it was not sent | Unreported items `not_measured` (`instrument_error` / `deadline`); stderr kept |
| Gateway or provider error (a transport failure, or an error result carrying a provider status) | Run `invalid(gateway)`; retried once as a new attempt |
| Per-run cap reached | Valid run; `ended_by = cap` |
| Job budget, kill, supervisor lost | Run `invalid` (`budget_stop` / `killed` / `interrupted`) |
| Resume finds a re-checked value changed | Refused unless forced; a forced deviation is recorded per run |
| Arms differ outside declared factors | The stratum is invalid; comparisons are refused |
| Dirty instrument asked to grade a gate or quality expectation | `resolve` fails |

### Refusals

A refusal is an input the caller can fix: a field in a document they wrote, a flag, a setting, a
missing prerequisite. Each has three parts, so that a coding agent can match on it without parsing
prose.

- **Code.** A stable dotted name, `<surface>.<reason>` (for example `manifest.unknown_field`,
  `suite.params_invalid`, `experiment.setting_as_factor`). Numeric codes are unreadable to an agent,
  and a code kept only in a message cannot be matched.
- **Side.** Whose input is at fault: `plugin`, `suite`, `experiment`, `config`, `environment` or
  `candler`. `candler` is never the side of an author's error; it marks a defect in Candler.
- **Path.** An RFC 6901 JSON pointer into the parsed document, or a file. A pointer reads the same
  for TOML and for JSON documents. A parse error gets its pointer through `serde_path_to_error`.

The record is `refusal/1`, `{schema, code, side, path?, file?, message, causes[]}`, on the
`schemas@` line, and the trait `candler_schemas::refusal::Refusal` (`code`, `side`, `path`) is
implemented by each error enum an author can reach. The three documents an author writes
(experiment, suite, plug-in manifest) refuse unknown fields, naming the field; the records Candler
writes stay lenient (§Wire encodings).

The registry is `docs/contract/codes.toml`, hand-written and append-only: a released code is never
renamed or removed, which makes stability checkable. It is the oracle of the tests that read the
codes, so no test takes its expectation from the code it checks. Every other error that reaches the
CLI carries the fallback code `candler.internal`, side `candler`, and is listed in the registry's
`[internal]` allowlist; `CliError::failed` keeps its bound, and `CliError::refused_by` takes a
`Refusal`. Exit codes stay 0, 1 and 2.

### Contract pages

The pages of `docs/contract/` tell a coding agent what each surface expects: its fields, their
classes and the refusals it can raise. A field's class comes from the committed schema files, the
files plug-in authors validate against: a property in `required` is `required`; one with a default
or optional is `optional`. Only `candler` (written by Candler, never set by an author) and `free`
(stored, never read) are declared, as the annotation keyword `x-candler-class`. A class table kept
outside the types would drift from them.

The pages are generated by the drift test `contract_pages`, which mirrors `schema_drift` and is
regenerated with `CANDLER_BLESS_CONTRACT=1`. It reads the committed schema files, never the Rust
types. The code-side catalogue is `candler_core::refusal::catalog()`; the helper's codes are the
`[helper]` section of `codes.toml`, read by the helper's tests and by the generator. The page of
the driver traits (`driver-traits.md`) is written by hand by the session-runner milestone and
folded into the generator by whichever milestone merges to `main` second; until then the generator
lists it in the index and leaves its file alone.

## Golden corpus (M2a exit gate)

**Reference, done 2026-10-28.** The 240 runs of 2026-08-06 (12 cells: Haiku 4.5, Sonnet 4.6, Sonnet
5 × skill v0.2.3–0.2.6, 20 runs each, rikyu domain), regraded by `htn-creator-eval@0.4.2`
(mcp-python-ingestion `28be2c15`, clean tree) against rikyu's own catalog snapshot. Frozen in
`agenticai4hpc26-artifact` on branch `regrade/htn-creator-eval-0.4.2`, commit `7efa1b7`, tag
`candler-m2a-golden`; the published `main` is untouched. `expectations.jsonl` holds one row per
`(run, expectation)`: 10,320 rows.

**Legacy import mapping.** `score` / `max_score` → `weight` (omitted when it equals the default,
dropped on `not_measured`); `blocked_by` → `cause {artifact}`; an integer value → `count`, a float
→ `number`, text → `text`; per-row `role` kept; evaluator `htn-creator-eval@0.4.2`, instrument
`htn_creator_scripts` `0.4.2` with digest `git:28be2c15…` and `dirty: false`. What the clean model
cannot state takes the legacy marker: a `not_measured` row has no cause column, so its cause is
`{legacy: <code>}` with the `reason=` of its `TIER_D_REPLAY` marker (`no_replay_target`, 181 runs ×
4), `zero_mappings` (the mapping declares no entries, 3 runs), or `unrecorded`; a `pass` weighing
`0/0` (209 gate rows) carries `{legacy: zero_weight}`. `expectations.jsonl` carries `status`,
`value`, `score`, `max_score` and `role` only: `blocked_by` and the evidence are in each run's
`eval-N-grading.json`, so the importer reads those (half of those blobs are AppleDouble `._*`
files, which it skips). In this corpus no row has a text value (every
D-E1 and D-E2 row is `blocked` or `not_measured`) and no row is `not_applicable`; fixtures for those
cases are synthetic. Integrity `incomplete`
(`missing_genuine_output`, the agent never produced the file; 56 runs) is a **valid** run;
`contaminated` is `invalid(input_leak)`. Invalid runs are never sent to `evaluate`.

**Parity test (scope: Tiers A–C).** Candler plus the HTN plug-in `htn-creator-eval@0.5.2`, with
its checker pinned at `28be2c15` and run one forked child per expectation with a 60 s deadline,
regrades the corpus, and `diff htn-creator-eval@0.4.2 htn-creator-eval@0.5.2` reproduces `(status,
value, weight, cause)` on the 8,880 rows of the 37 Tier A–C expectations: exactly on clean rows, and
on the one legacy-marked kind through an equivalence file. The 1,440 Tier D rows (D-E1…D-E6 × 240)
are **uncompared**: `diff` lists them with their counts, and their parity is work on the HTN side
after M2a. The plug-in emits clean records only, so a `pass` weighing `0/0` (`{legacy:
zero_weight}`, 209 gate rows) is live `not_applicable` with cause `{reason: empty_population}`, an
evaluator reason code (outcome reasons are free strings; the core's own are `not_applicable_to_arm`,
`instrument_error` and `deadline`). The equivalence file names each of those 209 rows by run key and
expectation; a listed row is excused only when it shows exactly that change, excused rows are counted
apart from unchanged ones, and the file's digest is recorded in the diff. The reference's known
defects are reproduced too: C-M2's bijection check false-fires on 88 of 89 failing runs. Statuses
over all rows: pass 8,291, blocked 802, not_measured 742, fail 485; over Tiers A–C: pass 7,999,
blocked 466, fail 415. Rates are compared per cell for quality and for the A–C gates. Fixes come
after parity, as evaluator version bumps, visible through `diff`.

**Mismatch triage, in order:** the instrument digest differs from the pin (re-pin, rerun) → an
input the legacy importer reconstructed (importer defect) → anything left is a core defect.

**Importing the corpus (M1).**

- **Extraction.** The corpus is read from a `git archive` of the tag, with the empty directories the
  archive drops recreated; nothing is written in the artifact repository.
- **Manifest fields.**

| Field | From | Rule |
|---|---|---|
| `comparable.values` | `experiment.json` `plan.*`, `provenance.*`, and the top-level key the import rules name for the version the treatment resolved to (`resolved_skill_version` here) | Each JSON leaf becomes a dotted field path with source `imported{experiment.json}`; a `null` leaf has no value and is `unrecorded`. `plan.out_dir` goes to `import`. |
| `comparable.harness` | the harness name the import rules give, and the version at their JSON pointer into the stream's first line (`claude_code_version` here) | Must agree across a cell, else the import is refused |
| `comparable.treatment` | `experiment.json`, at the JSON pointer the import rules name | A string written `<name>@<tag>`, else the import is refused; it gives the name and tag, and no commit, which the legacy folder does not record |
| `comparable.snapshots[]` | `PROVENANCE.json`, at the JSON pointers the import rules name | One entry per snapshot rule: the rule's kind and the bare sha256 the legacy provenance recorded at its pointer, not the RFC 8785 form a live snapshot's digest is taken over |
| `comparable.inputs[]` | each run's `timing.json` `copied_inputs[] {role, sha256}` | `role` is the kind; the lists must agree across a cell, else the import is refused |
| `comparable.environments[]` | — | Empty: the legacy folder records no plug-in environment |
| `measured` | each run's `eval_metadata.json` `provenance.*`, where the cell-level value is `null` | Source `imported{<run-relative path>}`; the copies within one run must agree, else the import is refused |
| `factors` | — | The field paths whose values differ across the imported cells |
| `experiment` | — | The digest of the sorted cell digests |
| `run` | — | `(cell digest, N, 1)`, N from the run folder |
| `started_at`, `ended_at` | `progress.jsonl` `run_start` + `elapsed_seconds`, `run_complete.completed_at` | |
| `integrity` | `PROVENANCE.json` `runs[].original_integrity.verdict` | `clean` and `incomplete` → `valid`; `contaminated` → `invalid{input_leak}`; any other value is refused; cross-checked with the cell's `contamination_catalog.jsonl` |
| `import` | — | The format label, base commit, legacy integrity block, and every source file read with its sha256, each kept as a blob |

- **Evaluator identity** comes from `PROVENANCE.json` `pinned` (`describe`, `commit`, `dirty`). The
  instrument name, the marker patterns, the format label, the harness name, the pointers to the
  harness version, the treatment and the snapshots, and the key of the resolved treatment version
  come from import rules the caller supplies, so the core names no domain (I1).
- **Outcome identity.** One request per run per evaluator. `request_id` is a UUIDv7 whose 48-bit
  time is the regrade's finish time in milliseconds (`PROVENANCE.json` `regraded_at.finished`) and
  whose 74 random bits are the first 74 bits of `sha256(<run key path form> 0x00 <evaluator
  name@version>)` (12 bits of `rand_a`, then 62 of `rand_b`), so a re-import yields the same id. `suite_entry_digest` is computed over a synthesised suite entry and
  `context_digest` over the context projection, with the functions live runs use. Imported digests
  never equal live ones, so M2a joins imported and live outcomes on `(run, expectation_id,
  evaluator@version)`.
- **Edges.** An invalid run gets no outcomes; its grading files stay as blobs. A reader failure lands
  the run without a trace and is listed in the import report. A run without `outputs/` is refused.
- **Re-import (I11).** A byte-identical re-import is a no-op. Every finished run is compared with the
  folder, manifest and workspace tree, before the first write, and one that differs is refused, so a
  refused import writes nothing. A run that began importing but has no `run.done` is moved aside,
  its moved-aside copy is deleted, and it is imported again: it never sealed, so I11 does not hold it.

**Known gaps:** the corpus is `with_skill` only, so parity exercises `not_applicable` only through
the 209 `empty_population` rows, and never a contaminated run. The sandbox planner version is `null` in `experiment.json`, which is written when
the experiment is planned; the value each run actually used is in its `eval_metadata.json`, written
by the original artifact commit `36290ab`, and is imported under `measured`. At cell level it stays
`unrecorded`.

## Alternatives Considered

| Decision | Options | Chosen | Tradeoff accepted |
|---|---|---|---|
| Language | Python + uv / **Rust** | Rust | First Rust project; the first plug-in stays Python, so the protocol is the main path |
| Wire schema source | Hand-written JSON Schema / **serde structs, schema generated** | structs | Schema shape follows Rust types; CI checks the generated files match |
| Protocol framing | MessagePack / gRPC / **JSON-RPC 2.0 lines on stdio** | JSON-RPC | Text parsing, negligible against small messages; every language speaks it |
| Evaluator isolation | Process per expectation / **process per request, forked child per expectation** / long-lived worker | fork | One interpreter start per run per evaluator; fork safety must hold on macOS; long-lived workers stay a future feature |
| Paths | Inline payloads / **file paths into a workspace-shaped view** | paths | Evaluators need filesystem access; links cost nothing |
| Outcome storage | One shared JSONL / **one file per request** | per request | More files; no locks, no races, partial requests visible |
| I6 | Independent re-implementation / **one pinned checker library** | pinning | The agent may hold the same checker as its grader; whether it used it is measured separately (instructed coverage) |
| `blocked` in rates | Excluded / **counted as a failure** | counted | An arm that produces less cannot look better |
| Harness support | External protocol / **compiled-in drivers behind public traits** | compiled in | A new driver needs a rebuild or an upstream PR |
| Jobs | Daemon / blocking CLI / **detached supervisor per job, kernel-released locks** | per job | `ps` reconstructs state from job directories; orphan reaping on macOS waits for the next command |
| Figures | In core / **tables out** | tables | Users bring their own plotting |

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| The trace model drops signal (side-chains, denials, hook payloads, bundled MCP calls) | `ext`, `raw_ref`, `wrapped_calls`; a fixture corpus per harness format version |
| The driver trait ends up shaped like Claude Code | OpenCode v2 runs on the same traits before the freeze; the core names no harness and a shared fixture suite runs every driver |
| Detached descendants or orphans survive (Claude Code starts its shells in new sessions) | Marker sweep after every group kill; child records; Linux child subreaper; reaping from records by `ps`, `kill` and `run`; a test kills a supervisor mid-run and checks no marked process survives. A Linux parent-death signal is not in place: the execution milestone decides between an isolated `unsafe` module and the subreaper plus marker sweep alone |
| `fork` after imports is unsafe on macOS when threads or system frameworks are loaded | The helper refuses to fork when more than one thread exists and falls back to one process per item; tested on macOS in M2a |
| Parity mismatches are instrument drift | Instrument digests and dirty flags; the pinned reference; the triage order |
| The legacy importer reconstructs inputs wrongly | Importer fixtures; triage step 2 |
| Harness churn (formats, flags, auto-updates) | Supported ranges; unknown formats `invalid`; resume refuses a changed harness version |
| OpenCode behaviour outside the recorded runs | The driver accepts one validated version, declares what it cannot do and refuses the settings it cannot honour; the unexercised cases are listed in §OpenCode v2 |
| First Rust project | Grit conventions and their reviewer; small crates; schemas and protocol first |
| The lean CLI regrows flags | Every new option must say why it cannot be derived or live in the experiment file |
| An agent driving the MCP surface spends without bound or reads ground truth | Mandatory budgets (I13); read/mutate split; I8 at the surface |

## Deliberately open

Decided during development: pairing keys, execution order and a declared blocking factor (M6; start
times are recorded from M2b); a `without_skill` fixture for the parity suite; reporting
`instrument_error` and `deadline` counts per arm; event kinds beyond the first set; the slot count
default; a macOS orphan watchdog; long-lived workers; the MCP tool shapes; the store root default;
`snapshots[]` and `inputs[]` needs (unimplemented, refused by the registry). The container backend
is decided (M2b): Docker through its CLI, one `docker run` per run; `Isolation` turns a plan into a
spawn request, then collects and destroys. The long-lived handle's `exec`, `inject` and
whole-container `census` are M5's, as are a host backend and an egress allowlist.

## Implementation

The milestones, their order, exit gates and scope are in the meta-roadmap
[`__roadmap__/candler-v1/`](../__roadmap__/candler-v1/README.md). Each milestone is planned in
detail by its own session, as its own campaign under `__roadmap__/`.

HTN lane: HTN Phase 4.5 reads the catalog snapshot; Phase 7 writes its Tier C logic as library
functions; after M2a the checker library becomes the plug-in and `eval.json` becomes a suite file
(no templates); Phase 8 waits for M5. The reasoning is in
`/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion/__reports__/candler_core/02-findings_lane_overlap_v0.md`.
