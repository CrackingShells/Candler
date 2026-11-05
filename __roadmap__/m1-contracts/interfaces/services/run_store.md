# Run store

**Goal**: `candler_core::store` lays out the files-first store of R01 §Run store: write-once blobs, runs written step by step with `run.done` last, one outcome file per request committed atomically, and an index that can always be rebuilt from the files.
**Pre-conditions**:
- [ ] [cmd] DEP(wire_foundations), DEP(wire_trace) and DEP(wire_execution) hold: `for l in wire_foundations wire_trace wire_execution; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`
- [ ] [nl] api_review closed with no open finding on `RunManifest`, `RunTree`, `TraceFile` and `RunKey`'s path form
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
**Success Gates**:
- ✅ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_store)'` passes, holding five B tests:
  - **stop after step k**: for each k, a run written up to step k and then dropped leaves only files that an independent `serde_json` walk of the directory parses completely, no `run.done` unless every step ran, no `.partial` visible to `Store` readers, a retry with `Existing::ReplaceUnfinished` succeeds, and `seal_interrupted` instead seals it `invalid {interrupted}` with `next_attempt` returning its attempt + 1;
  - **index**: after `rebuild_index`, `index/runs.jsonl` and `index/outcomes.jsonl` equal what a test-side directory scan lists; four threads appending 25 outcome files each leave 100 parseable index lines; an index whose last line is cut by hand reads every complete line;
  - **append-only** (I11): blobs are mode 0444, a blob with a wrong expected digest is refused, re-creating a finished run is refused, and committing over a final outcome file is refused, each with its positive control;
  - **copied store** (I17): a store copied file by file (`std::fs`, no child process) to another directory reads byte-equal, manifests, trees, traces and outcomes;
  - **layout**: the path set of one fake run with two outcome files equals a literal list written from R01 §Run store
- ✅ [cmd] `cargo nextest run -p candler-core -E 'binary(public_surface)'` passes, naming `Store`, `StoreLayout`, `RunWriter`, `Existing`, `OutcomeWriter` and the readers
- ✅ [cmd] STD and SUBJ pass
- ✅ [nl] The leaf report has a bite mutation per B test
**References**: [R01 §Run store, §Run identity and resume, §Invariants (I11, I17)](../../../../__design__/architecture.md) — layout and atomicity rules; [R07](../../../candler-v1/contracts/parity/README.md) — M2a writes outcomes, M2b writes runs

## Step 1: Lay out the store and write blobs once
**Goal**: A store is created and opened by its marker, every path comes from one layout, and a blob is written once.
**Implementation Logic**:
1. `Store::init(root: &Path) -> Result<Store, StoreError>` writes `store.json` `{schema: "candler-store/1"}`; `Store::open(root)` refuses a directory without it.
2. `StoreLayout` (the only code that builds store paths): `run_dir(&RunKey)` (`runs/<cell hex>-r<n>-a<m>/`), `blob(&Digest)` (`blobs/<hex>`), `outcome(&RunKey, &NameVersion, &RequestId)`, `job_dir(&JobId)`, `experiment_lock(&Digest)`, `index_dir()`.
3. `Store::put_blob(&self, bytes: &[u8], expected: Option<&Digest>) -> Result<Digest, StoreError>`: write to a temporary file in `blobs/`, fsync, verify the digest, rename, `chmod 0444`, fsync the directory; an existing blob is not rewritten. `Store::put_blob_from_path(&self, &Path, expected: Option<&Digest>)` does the same by streaming a file. `Store::blob_path(&Digest)`.
**Deliverables**: `crates/candler-core/src/store/{mod.rs,layout.rs,blob.rs}` (`Store`, `StoreLayout`, `StoreError`, `Store::init`, `Store::open`, `Store::put_blob`, `Store::put_blob_from_path`, `Store::blob_path`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(run-store): lay out the store and write blobs once`

## Step 2: Write runs with their tree, raw and trace
**Goal**: A run is written step by step and counts as complete only once `run.done` exists.
**Implementation Logic**:
1. `RunWriter::create(&Store, &RunManifest, Existing) -> Result<RunWriter, StoreError>`, `Existing`: `Refuse` or `ReplaceUnfinished` (a run directory without `run.done` is moved aside to `runs/.replaced/<key>-<n>/`, never deleted). A finished run is always refused. The run directory appears by renaming a temporary directory that already holds the manifest, so every run directory has one. `ReplaceUnfinished` is the importer's; a live run never reuses a key (R01: the next attempt gets the next number).
2. Steps, each written to a temporary name, fsynced and renamed: `manifest` (at create, and again with `RunWriter::manifest(&RunManifest)` until `done`, the run key unchanged: a live run fills `measured`, `ended_at` and `integrity` at its end), `tree(&RunTree)`, `raw(&StreamName, gz: &Path, digest: &Digest)` (moves a finished `raw-frames/1` `.gz` into `raw/<stream>.jsonl.gz` and checks its digest; the store does no framing and does not depend on `frames`, a sibling leaf), `trace(&TraceFile)`, then `done(self)`, which writes `run.done` (an empty file) last and fsyncs the directory. `raw` refuses a stream that is already present and copies when the move crosses a device; `create` refuses a manifest whose run key names a cell other than its own cell digest, and `RunWriter::manifest` one that names another run (`ManifestMismatch`); directories are created when first written. `run.done` seals the run: nothing in its directory changes afterwards, and its manifest's `integrity` says whether it counts.
3. `Store::next_attempt(&Digest, replicate: u32) -> u32`: one past the highest attempt present for that cell and replicate, sealed or not. `Store::seal_interrupted(&RunKey) -> Result<RunManifest, StoreError>` rewrites an unsealed run's manifest with `integrity: invalid {interrupted}` and seals it (R01: an attempt without `run.done` stays in the ledger); it refuses a sealed run.
4. Readers, each returning a `Result` because a listing reads the directory: `Store::runs() -> Result<Vec<RunEntry {key: RunKey, done: bool}>, StoreError>`, `read_manifest`, `read_tree`, `read_trace`, `raw_path(&RunKey, &StreamName)`; readers skip temporary and `.partial` names.
**Deliverables**: `crates/candler-core/src/store/run.rs` (`RunWriter`, `RunWriter::manifest`, `Existing`, `RunEntry`, `Store::next_attempt`, `Store::seal_interrupted`, `Store::runs`, `Store::read_manifest`, `Store::read_tree`, `Store::read_trace`, `Store::raw_path`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(run-store): write runs with their tree, raw and trace`

## Step 3: Write one outcome file per request atomically
**Goal**: A request's outcomes become visible all at once, and never change afterwards.
**Implementation Logic**:
1. `OutcomeWriter::create(&Store, &RunKey, evaluator: &NameVersion, request: &RequestId) -> Result<OutcomeWriter, StoreError>` opens `<…>/<request id>.jsonl.partial`; it refuses when the final file exists.
2. `append(&mut self, &OutcomeRecord)` writes one line and flushes; `commit(self)` fsyncs, renames to `.jsonl` and fsyncs the directory. Dropping without commit leaves the `.partial`; a later `create` for the same request removes it and opens a new one with `create_new`, and `commit` publishes by `hard_link`, which never replaces a final file (I11).
3. Readers: `Store::outcome_files(&RunKey) -> Result<Vec<OutcomeFileRef {run: RunKey, evaluator: NameVersion, request: RequestId}>, StoreError>`, `read_outcomes(&OutcomeFileRef) -> Vec<OutcomeRecord>`.
**Deliverables**: `crates/candler-core/src/store/outcome.rs` (`OutcomeWriter`, `OutcomeFileRef`, `Store::outcome_files`, `Store::read_outcomes`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(run-store): write one outcome file per request atomically`

## Step 4: Keep a rebuildable index of runs and outcomes
**Goal**: The index is a cache: appended under a lock, rebuilt atomically from the files, and read through a torn last line.
**Implementation Logic**:
1. `RunWriter::done` and `OutcomeWriter::commit` append one line to `index/runs.jsonl` or `index/outcomes.jsonl` under an `flock` on `index/lock` (`rustix`); the line format is internal to the core.
2. `Store::rebuild_index(&self)` scans `runs/` and `outcomes/`, writes both files to temporary names and renames them.
3. `Store::indexed_runs()` and `Store::indexed_outcomes()` read the index, skipping a last line that does not parse. The index lists sealed runs only. An index append that fails after a run or outcome file is published does not undo the publish; `rebuild_index` repairs it. The store is Unix-only (`flock`, file modes).
4. `tests/guard_store.rs`: the five B tests of the Success Gates, on `tempfile` directories. Then delete the leaf's scaffolding and write the bite record.
**Deliverables**: `crates/candler-core/src/store/index.rs` (`Store::rebuild_index`, `Store::indexed_runs`, `Store::indexed_outcomes`); `crates/candler-core/tests/guard_store.rs`; `crates/candler-core/tests/public_surface.rs` (the store items)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_store) | binary(public_surface)' && ! git grep --untracked -n 'mod scaffold' -- crates/candler-core/src/store` (expected: PASS)
**Commit**: `feat(run-store): keep a rebuildable index of runs and outcomes`
