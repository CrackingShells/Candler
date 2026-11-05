# Legacy import

**Goal**: `candler_core::import::import` lands a legacy results folder in a Candler store: artifacts as blobs with their workspace tree, streams framed and normalised, manifests and outcome files written so that a re-import is a no-op (I11). It produces an `ImportReport` in which every difference is explained, and the 240-run golden corpus imports equal to the oracle.
**Pre-conditions**:
- [ ] [cmd] DEP(supervisor), DEP(run_store), DEP(claude_code_reader) and DEP(legacy_mapping) hold: `for l in supervisor run_store claude_code_reader legacy_mapping; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`
- [ ] [nl] The implementer of this leaf did not write `golden_oracle.md`'s oracle, and does not edit `crates/candler-core/tests/oracle/`
- [ ] [cmd] `test -f "$GOLDEN/PROVENANCE.json" && test "$(find "$GOLDEN" -mindepth 4 -maxdepth 4 -type d -path '*/_run_cache/with_skill/run-*' | wc -l | tr -d ' ')" = 240`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_import_mini)'` passes, holding four B tests over the mini-corpus imported into a `tempfile` store and read back only through `Store` readers:
  - **through the store**: the stored outcome records, projected as in `guard_legacy_mapping`, equal `oracle::expected_rows`; the contaminated run is done, has no outcome file, and its grading files are blobs named in `import.sources`; every real run has a trace; the report's `runs` and `outcomes` equal hand-written counts, and every entry of every list has a non-empty `explanation`;
  - **determinism**: two copies of the mini-corpus at different temporary paths, imported into two stores, give byte-identical stores, compared by a test-side walk of relative paths and bytes;
  - **idempotence** (I11): three faulted states each re-import to a store byte-identical to a clean import: a reader that panics on the second run (caught with `catch_unwind`), an artifact made unreadable in the second run and restored, and a clean import whose third run's outcome file was replaced by a truncated `.partial`. Re-importing a finished store rewrites no file. A finished run whose source differs by one byte is refused naming the run and the file; the unchanged source is the positive control;
  - **paths** (I17): every path-typed field of every manifest, tree, trace and outcome record is relative, and `assert_clean` finds neither the temporary corpus root nor the store root anywhere in the store
- ⬜ [cmd] `CANDLER_GOLDEN_CORPUS="$GOLDEN" CANDLER_GOLDEN_REPORT="$TMPDIR/m1-golden-report.json" cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_golden)'` passes, holding two B tests:
  - **golden rows**: it panics when `CANDLER_GOLDEN_CORPUS` is unset, and asserts 240 runs and 10,320 oracle rows before comparing. Every stored outcome record validates against `schema/outcome-record.schema.json` with `jsonschema`, the projected records equal `oracle::expected_rows`, and the status counts equal R01's (pass 8,291, blocked 802, not_measured 742, fail 485). All 10,320 records, and every `import.requests[]` entry, name the evaluator `htn-creator-eval@0.4.2`, the literal written in the test from `PROVENANCE.json` `pinned.describe`, and none names `0.3.4`. The 0.3.4 values in a run's `experiment.json` and `eval_metadata.json` record the original grader and stay in the manifest under their own `provenance.eval_infra.*` paths;
  - **golden runs**: 240 done runs whose integrity, harness version and `measured` equal `oracle::expected_runs`, 12 distinct cell digests, and the report written to `CANDLER_GOLDEN_REPORT`. One pinned case is named in the test: run `20260806-035940_v0.2.4_claude-sonnet-5_model-claude-sonnet-5_skill_version-htn-creator-0.2.4/with_skill/run-8` (the usage-policy refusal, legacy verdict `incomplete`, 43 graded rows) is imported `valid` with 43 outcome records
- ⬜ [cmd] `jq -e '[.trace_failures, .usage_diffs, .tree_diffs, .skipped, .invalid_runs] | flatten | all(.explanation | length > 0)' "$TMPDIR/m1-golden-report.json"`
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_layering_i1)'` passes, holding three B tests:
  - **deny-list**: `assert_clean` finds none of `htn`, `HTN`, `gtpyhop`, `GTPyhop`, `rikyu`, `Rikyu`, `RIKYU`, `tier_d`, `TIER_D`, `D-E` and `mcp-python-ingestion` (exact bytes) in `crates/candler-core/src`, `crates/candler-harness/src` and the published `crates/candler-schemas/schema`; `candler-schemas/src` is not scanned, because its existing `mod tests` hold real legacy identities as fixtures by design;
  - **renamed domain**: a copy of the mini-corpus with every deny-listed string rewritten by the test (streams decompressed, rewritten and recompressed), imported with rules naming `acme-eval`, gives the same run count and per-status counts as the original, and `assert_clean` finds no deny-listed string in the store;
  - **codes come from the rules**: with `markers` empty, every `not_measured` record has cause `{legacy: unrecorded}`
- ⬜ [cmd] `! cargo tree -p candler-core -e normal --prefix none | grep -q '^candler-harness-claude-code '`
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(public_surface)'` passes, naming `import`, `ImportReport` and its entry types, `ReaderFactory`, and `trace::normalise`
- ⬜ [cmd] STD and SUBJ pass
- ⬜ [nl] The required verifier reports no open finding, having attacked oracle independence, the generality of the rule patterns, weight-omission edges, `blocked_by` paths, `sources` kept out of the hash, and I1 beyond the deny-list
- ⬜ [nl] The leaf report has a bite mutation per B test and the log of the ignored golden run
**References**: [R01 §Golden corpus (Importing the corpus), §Run store, §Raw traces, §Invariants (I1, I3, I11, I17)](../../../../../__design__/architecture.md) — every rule; [R03](/Users/hacker/Documents/src/External/PCfVW/agenticai4hpc26-artifact) — the corpus, read from `$GOLDEN`; [R04](../../../../../__reports__/m1-contracts/00-test_definition_v0.md) — `guard_import_mini`, `guard_golden`, `guard_layering_i1`

## Step 1: Import artifacts and their workspace tree
**Goal**: A run's `outputs/` becomes blobs and a `RunTree`, and every file the legacy grader collected but `outputs/` lacks, or the reverse, is explained.
**Implementation Logic**:
1. `import_assets(&Store, &LegacyRun) -> Result<ImportedAssets {tree: RunTree, tree_diffs: Vec<TreeDiff>}, ImportError>`: every file under `outputs/` is put with `Store::put_blob`, its tree entry `file {path relative to outputs/, sha256, executable}` from its mode bits; an empty directory is a `dir` entry; an empty `outputs/` gives an empty tree.
2. `TreeDiff {run: RunKey, path: RelPath, explanation: String}` for each name in `timing.json` `collected_outputs_paths` with no file in `outputs/`, and each file in `outputs/` not listed there; the explanation says which side holds it.
3. Every `SourceFile` of the run (and the cell and folder sources) is put as a blob; its digest is the one already in `import.sources`.
**Deliverables**: `crates/candler-core/src/import/assets.rs` (`import_assets`, `ImportedAssets`, `TreeDiff`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(importer): import artifacts and their workspace tree`

## Step 2: Frame legacy streams and normalise their traces
**Goal**: A legacy stream becomes a `raw-frames/1` file and, through the caller's reader, a trace file; a reader failure leaves the run without a trace and is reported.
**Implementation Logic**:
1. `ReaderFactory = dyn Fn() -> Box<dyn TraceReader> + Send + Sync`, supplied by the caller, so the core never depends on an adapter crate.
2. `frame_streams(&LegacyRun, dir: &Path) -> Result<Vec<RawFileInfo>, ImportError>`: `agent_stream.jsonl.gz`, decompressed, through `FrameWriter` as stream `stdout`, and `agent_stderr.txt` as stream `stderr`, each with `clock: unrecorded`, no `t_mono_ns`, and `source` the sha256 of the legacy file.
3. `candler_core::trace::normalise(reader: Box<dyn TraceReader>, raw: &RawFileInfo) -> Result<TraceFile, NormaliseError>`, public because M2b normalises live runs with it: `detect` on the stream's first bytes (`None` → an error); each frame from `FrameReader` to `feed` as a `FrameIn` (its `raw_ref`, and `at` from the header's clock anchor and `t_mono_ns` when the clock is monotonic); `finish`; then `TraceFile {header {reader: reader.id(), raw}, sessions, events}`. A `ReaderError`, a refused detection or a `TraceFile` the schema refuses is a `NormaliseError` naming the frame where one applies; the importer calls it with a fresh reader from the `ReaderFactory` and records an error as a `TraceFailure {run, explanation}`.
4. `UsageDiff {run, counter, trace, legacy, explanation}` for each token counter whose sum over the trace's `usage {scope: message}` events differs from `timing.json` (`input_tokens`, `output_tokens`, `cache_creation_tokens`, `cache_read_tokens`); the explanation names both numbers and both counting rules.
**Deliverables**: `crates/candler-core/src/import/streams.rs` (`ReaderFactory`, `frame_streams`, `TraceFailure`, `UsageDiff`); `crates/candler-core/src/trace/mod.rs` (`normalise`, `NormaliseError`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(importer): frame legacy streams and normalise their traces`

## Step 3: Assemble runs idempotently and report the import
**Goal**: `import` writes each run and its outcomes in a fixed order, so a stop at any point re-imports to the same store, and returns a report in which every exception is explained.
**Implementation Logic**:
1. `import(folder: &Path, rules: &ImportRules, readers: &ReaderFactory, store: &Store) -> Result<ImportReport, ImportError>`: `LegacyFolder::read`, `map_runs`, then each run in `RunKey` order.
2. Per run (I11): a finished run whose stored manifest equals the mapped one is skipped; a finished run that differs is refused naming the run and the first differing source; an unfinished one is replaced (`Existing::ReplaceUnfinished`). Otherwise `RunWriter::create`, the assets, `tree`, `raw` for each framed stream, `trace` when normalisation succeeded, then `done`. A valid run then gets one outcome file: `OutcomeWriter::create(run, evaluator, request_id)`, `append` each record of `map_grading`, `commit`; a final outcome file already present is left as it is.
3. `ImportReport {runs: u64, outcomes: u64, invalid_runs: Vec<InvalidRun {run, reason, explanation}>, trace_failures: Vec<TraceFailure>, usage_diffs: Vec<UsageDiff>, tree_diffs: Vec<TreeDiff>, skipped: Vec<SkippedEntry {path, reason, explanation}>}`, every list sorted, serialised to JSON with `ImportReport::to_json`. The report is the caller's to keep; the store holds nothing that depends on when or from where the import ran.
4. `tests/guard_import_mini.rs`: the four B tests of the Success Gates, with the rules from `tests/support/legacy_rules.rs` and `ClaudeCodeReader` (a dev-dependency) as the reader.
**Deliverables**: `crates/candler-core/src/import/assemble.rs` (`import`); `crates/candler-core/src/import/report.rs` (`ImportReport`, `InvalidRun`, `SkippedEntry`, `ImportReport::to_json`); `crates/candler-core/tests/guard_import_mini.rs`
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_import_mini)'` (expected: PASS)
**Commit**: `feat(importer): assemble runs idempotently and report the import`

## Step 4: Import the golden corpus against the oracle
**Goal**: The full corpus imports equal to the oracle, and the core is shown to name no domain.
**Implementation Logic**:
1. `tests/guard_golden.rs`: the two ignored B tests of the Success Gates; they read `$CANDLER_GOLDEN_CORPUS` and write the report to `$CANDLER_GOLDEN_REPORT` when set.
2. `tests/guard_layering_i1.rs`: the three B tests of the Success Gates. The deny-list is a literal in the test.
3. Run the golden test once and keep its log for the leaf report. Then delete the leaf's scaffolding and write the bite record.
**Deliverables**: `crates/candler-core/tests/guard_golden.rs`; `crates/candler-core/tests/guard_layering_i1.rs`; `crates/candler-core/tests/public_surface.rs` (the import items)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_layering_i1) | binary(public_surface)' && CANDLER_GOLDEN_CORPUS="$GOLDEN" cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_golden)' && ! git grep --untracked -n 'mod scaffold' -- crates/candler-core/src/import crates/candler-core/src/trace` (expected: PASS)
**Commit**: `test(importer): import the golden corpus against the oracle`
