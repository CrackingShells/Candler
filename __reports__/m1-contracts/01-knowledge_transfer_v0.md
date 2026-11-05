# M1 Contracts — Knowledge Transfer (v0)

Date: 2026-11-05

This report is for whoever starts M2a (evaluation) or M2b (execution). Each of them reads the
Executive Summary, then the section with its own name. The design itself is in
[R01 `__design__/architecture.md`](../../__design__/architecture.md), and the campaign is in
[`__roadmap__/m1-contracts/`](../../__roadmap__/m1-contracts/README.md). This report links to
both and does not restate them.

## Executive Summary

- **What shipped.** The milestone delivered:
  - every remaining wire type on the `schemas@` line: 16 roots, listed in
    [`schema/README.md`](../../crates/candler-schemas/schema/README.md);
  - the `candler-harness` traits;
  - the Claude Code trace reader (`READER` 0.2.0);
  - the run store, the frames writer and reader, and the process supervisor core with the I22
    signal guard;
  - the `resolve` skeleton with its config loader;
  - the legacy importer.

  Each crate has a README that says what it holds and which guard file pins each part.
- **The milestone gates on the joined branch:**

  | Gate | Result |
  |---|---|
  | Golden import | 240 runs, 10,320 schema-valid outcome records, equal to the independent oracle |
  | Traces | 0 trace failures, 0 invalid runs |
  | Integrity | The 56 `incomplete` runs are valid; the synthetic contaminated run is `invalid{input_leak}` with no outcome |
  | Supervision | `guard_supervise` passes on macOS; the PR's CI decides Linux |
  | Drift | The 16 roots equal the hand-written [`expected_roots.txt`](../../__roadmap__/m1-contracts/interfaces/services/import/closeout/expected_roots.txt) |

  The workspace runs 269 tests: 266 pass and 3 are ignored. The golden import also reports 348
  usage diffs, 65 tree diffs and 25 skipped files, and each one carries an explanation.
- **The test suite.** Only cornerstone guards are committed: tests named `guard_*`, each with a
  `Guards:` line and a recorded bite mutation
  ([test definition](00-test_definition_v0.md)). The oracle and its vectors are written
  independently of the importer.
- **Review.** Each level had a Grit review. The supervisor, the reader and the importer each had
  an adversarial verifier. api_review closed L1 and re-reviewed the services API at L4, and found
  them FIT, with no open BLOCKER or MAJOR finding.

## For M2b (execution)

### I22 first: the supervisor must never signal a process it did not start

On 2026-11-05 a bite mutation (`.pgid(pid)` → `.pgid(1)`) made the cleanup in `guard_supervise`
run `kill(-1, SIGKILL)`, which killed 394 of the user's processes. Since then:

- **All signals go through one choke point.**
  - It signals only a group that this `Supervisor` spawned and holds in its live table, after
    re-checking the leader's pid and start time.
  - A second layer refuses pid or pgid ≤ 1, our own pgid and our own pid.
  - A group read from disk is re-checked first; on a mismatch the signal is refused and the
    refusal is reported (`ReapReport.refusal`).
- **Tests reach delivery only through a seam.** `Signaller`, `with_signaller`, `kill_all` and
  `holds_group` sit behind the `test-support` feature. Never enable that feature on a production
  dependency edge.
- **The test protocol (binding):**
  - Bites on pid or pgid values, or on the guard itself, run only against the non-spawning
    `binary(guard_signal)`.
  - Restore a bitten file with `git checkout --`, never with `mv`: `mv` keeps the old mtime, so
    cargo reuses the mutated build.
  - Before every spawning run, run
    `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)'`.
  - Never run `kill`, `pkill` or `killall`.
- **M2b builds on the guard.** The stray sweep, the slots and M5's container census, which
  kills by marker, all go through it. None of them adds a second path to `kill`.

### Deferred to M2b (M2b deliverables)

| Item | What M1 leaves |
|---|---|
| **A `ChildRecord` reader** (stray sweep, crash recovery, resume) | `ChildRecord` is a public wire type (`child-record/1`). The files are `record_path(dir, pid)` under `<record_dir>`, and `write_record` writes them. M1 has no reader, so M2b writes one. The schema is frozen at M1: changing a field is a change to the `schemas@` line, under the `schemas` scope. |
| **A live trace adapter** | `trace::normalise` runs **offline only**, from a stored raw file. The live seam is `OutputSpec::with_observer(Box<dyn FrameObserver>)`. M2b writes the adapter that feeds frames to `TraceReader::feed` and reads `observed()` during the run. |
| **An isolated HOME** in `Workspace` | Not started |
| **When the catalog is captured** | Not decided |
| **PDEATHSIG** | Left to M2b. It conflicts with `forbid(unsafe_code)`, so M1 depends on the process group and `reap` instead. |

### Scope moved from M1 to M2b

- **`StraySweeper`** ships as a trait with no implementation.
  - `Supervisor::new` installs no sweeper, so the sweep records "no sweeper".
  - `run.stray` means something only once M2b calls `with_sweeper`.
- **The setsid-descendant gate** (a descendant that leaves the group), on macOS and on Linux.
- **Resume replay** against `deviations[]`, using `seal_interrupted` and `next_attempt` (both in
  `store::run`).

### The entry points M2b uses

| Need | Entry point | Note |
|---|---|---|
| Start a child | `Supervisor::spawn(SpawnRequest)`, built with `SpawnRequest::builder().argv(..)…build()` | The builder takes no arguments. The child record is on disk before `spawn` returns. `Debug` shows environment keys only. |
| Pipes and stop | `Child::take_stdin` / `take_stdout` / `take_stderr`; `Child::stop_handle() -> StopHandle`; `wait()` and `stop()` return `Exit {status, ended, steps[], group_empty, raw, stderr_tail}` | The deadline counts from `spawn`. |
| After a crash | `async reap(&self, run: Option<&RunKey>, &ChildRecord, &StopSpec) -> ReapReport` | It emits no `process.exited` and adopts any record that verifies. |
| Frames | `frames::{FrameWriter, FrameReader}`, `OutputSpec::Framed`, `Exit.raw` (`Vec<RawFileInfo>`, standard output first) | |
| Store | `RunWriter::create(.., Existing)`, then `manifest` / `tree` / `raw` / `trace` / `done` | `create` refuses a key whose cell is not the manifest's (`ManifestMismatch`). `run.done` is written last and is an empty file. |
| Cell | `Comparable::from_resolved`, `Comparable::from_resolution(resolved, sources, settings: &[FieldPath])` | Build the settings with `resolution.settings.iter().map(\|v\| v.path().clone()).collect::<Vec<_>>()`. |
| Resolve | `Resolution.settings`, `Resolver::register`, `ProjectConfig::discover` | |
| Reader | `ClaudeCodeReader::new()?` | It returns `Result` and has no `Default`. `READER` is a `&str`. |

### Rulings and facts M2b must know

- **`harness` and `treatment` must be derived.** Nothing in `resolve/` emits them, so a preflight
  step has to derive them at those root paths, or `from_resolution` fails with `MissingField`.
  A step cannot derive `harness.version` as a value, and a `harness` request from a file layer
  outranks a derived one. M2b's glue decides how the harness request enters.
- **Store root with no project.** It is `$XDG_STATE_HOME/candler/store`, else
  `~/.local/state/candler/store`. This also applies in M3.
- **Platform.** `supervise` is `#[cfg(unix)]`, and the store is Unix only by ruling (file modes,
  hard links). On macOS, a non-root
  process reading pid 1's start time gets `Unavailable`.
- **Groups and the pre-stop.**
  - A group whose leader exits while members remain stays held until `stop` or `kill_all`.
  - The pre-stop command inherits the child's environment, secrets included. This is by design
    and documented.
  - The HTTP pre-stop reaches loopback only, decided on the parsed host, and follows no
    redirects.
- **How a run ended (`ended_by`).** `gateway_error` covers transport failures and error results
  that carry a provider status. A content refusal (`api_error` with a null status) is
  `agent_error`: the run is valid and graded.
- **Usage.** `session_total` sums the usage on result lines. It does not reconcile with Claude
  Code's `modelUsage`, which accumulates across the whole process; only the cost agrees between
  them (R01 D2).
- **OpenCode.** The ids it observes come from a Lines reader over stdout, with no trait change.
  `guard_two_shapes` proves the shapes can be expressed, not that they work.
- **Module paths.** No crate re-exports anything, so import by module path, for example
  `candler_core::store::run::RunWriter`.

## For M2a (evaluation)

### Deferred to M2a

- **Input-source declarations in the suite file.** `ValueSource` has no suite variant, so
  `FileKind` was dropped.
- **Attaching snapshot content at import.** The importer records snapshot digests, not their
  content.

### The `import.requests[]` join and the reference

- Imported and live digests never match, so join on `(run, expectation_id,
  evaluator@version)`:
  1. Find the reference outcomes through the run manifest's `import.requests[] {evaluator,
     request}`.
  2. List them with `Store::outcome_files(run)`, which returns `OutcomeFileRef {run, evaluator,
     request}`.
  3. Read them with `read_outcomes`. `indexed_outcomes` and `rebuild_index` are also public.
- The reference is `htn-creator-eval@0.4.2`, from `PROVENANCE.json` `pinned`. The original
  grader at run time, 0.3.4, survives only under `provenance.eval_infra.*`.
- The parity diff asserts that the reference is @0.4.2 and that the live plug-in is pinned at
  `28be2c15`. A mismatch is triage case 1: re-pin.
- The importer writes one outcome file per valid run (one request per run per evaluator). Live
  data may carry more, so join through `outcome_files` rather than assume one request per run.
- `OutcomeWriter::commit` never replaces a final file (`OutcomeExists`). Re-grading at the same
  evaluator version is refused; a new version gets its own directory.

### The protocol M2a speaks

- **Method names.** `METHOD_DESCRIBE`, `METHOD_EVALUATE`, the request side (`{reported}`) and
  `-32800` are published in `evaluator-request-side.schema.json`.
- **Result shapes.** Which result answers which method (describe → `DescribeResult`, evaluate →
  `EvaluateResult`), and each shape's JSON-RPC slot, are given only in R01's table.
- **Leniency.** The helper should tolerate absent `describe` params. The Rust reader of the
  request side is laxer than its schema: it ignores unknown fields.
- **Template variables.** `{servers_dir}` and `{domain_name}` stay literal in `params`. M2a binds
  them, taking `servers_dir` from the server-source `inputs` kind.
- **Legacy causes.** The rule that refuses a legacy cause in notification params is enforced in
  Rust only; the schema carries just a `$comment`.
- **Metadata needs.** An unrecorded metadata need has no `RunContext` form.
- **The suite source.** Translate it from the 0.4.2 instrument
  (`mcp-python-ingestion@htn-creator-eval@0.4.2`, `eval.json` sha256 `ca7f12af…`). The artifact
  repo's `harness/` tree is **not** the 0.4.2 instrument: it lacks `--servers-dir` on D-E, and the
  D-E1 and D-E2 `status_regex`.

### Facts from the import that M2a must know

- **Mapping details:**
  - `TreatmentRule::new(pointer, resolved: FieldPath)` refuses a dotted key.
  - The `RunContext` root of an imported run is `/legacy`.
  - A legacy run with no catalog line counts as `clean`.
  - The legacy format needs `tier`, `scale` and `text`, so a domain without `tier` is refused.
  - One suite digest per id holds across the real cells only.
- **Context digest (Cond 7).** It is pinned by
  [`context_digest_runs.toml`](../../crates/candler-core/tests/oracle/vectors/context_digest_runs.toml),
  computed with metadata keyed by the dotted path and carrying the manifest value, and with
  artifacts taken as every regular file under `outputs/`. Two cases are untested by the real
  runs: AppleDouble `._*` files under `outputs/`, and an empty `outputs/`.
- **Usage diffs.** `output_tokens` differs on all 240 runs, because of the reader's counting
  rule. Each diff is listed in `ImportReport.usage_diffs`. `UsageDiff.counter` stays a `String`.

## Wins

- **Hand-written oracles found real defects.** The independent oracle, the Python vectors and the
  tables written by hand caught a session-less frame that dropped its usage (a BLOCKER), an
  unguarded `context_digest`, and an I11 hole in replacing outcome files.
- **Bite records were the cheapest check on test strength.** Every verifier re-ran three bites
  plus one of its own. Surviving mutants (env_clear, O1, drop-metadata) became new guards.
- **The scaffold commit and pre-declared modules made sibling merges mechanical.** The only
  recurring conflict was `public_surface.rs`, which was solved by giving each leaf its own block.

## Pain Points and Root Causes

| Pain | Root cause | Change for the next cycle |
|---|---|---|
| A bite killed 394 user processes | A test's cleanup could signal any pgid; bites ran on a spawning binary | I22 guard and test protocol (above); put it in every brief that touches processes |
| A stale mutated build passed the tripwire | `mv` restore kept the mtime | `git checkout --` restores plus `cargo clean -p` before spawning runs |
| A rewind of `feat/m1-contracts` | A 75-character subject merged before SUBJ ran | Run SUBJ on the branch before merging, not after |
| The suite was first translated from the wrong `eval.json` | The artifact's `harness/` was assumed to be the 0.4.2 instrument | Check the source sha against `PROVENANCE.json` in the script |
| The G5 roots diff read the schema README as a root | The gate used `ls schema`, while the leaf put a README in the same directory | The gate now matches `*.schema.json`; when a leaf adds a file to a directory, check the gates that read that directory |

## Follow-ups (out of M1 scope)

- `outcome.rs:323` `Value` accepts the list form, which breaks the wire rule. The fix is
  reader-only, through `MapOnly` or `object_only!`, and changes no schema bytes.
- Give the semver text helpers in `execution` and `evaluation` one home, and add cross-engine
  `[name]` vectors for `NAME_PATTERN`.
- `TraceFile::write` skips a line whose serialisation fails. Revisit this if a record type ever
  gains a fallible `Serialize`.
- The importer holds the whole legacy folder in memory, about 279 MB for the golden corpus.
- An exception to AGENTS.md rule 3 for documented literal parses can be proposed to Eliott.

## Artifacts to Preserve

- [Test definition](00-test_definition_v0.md) and the `guard_*` convention.
- `crates/candler-core/tests/oracle/`: the oracle, the vectors and `gen_vectors.py`.
- [`expected_roots.txt`](../../__roadmap__/m1-contracts/interfaces/services/import/closeout/expected_roots.txt)
  and the crate READMEs.
- R01's D1–D11, I22, and the I11 and I17 wording (commit `bfb48e2`).
