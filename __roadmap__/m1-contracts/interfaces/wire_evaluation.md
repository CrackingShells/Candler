# Wire evaluation: plug-ins, suites, evaluator protocol

**Goal**: `candler_schemas::evaluation` defines the plug-in manifest, the suite file with its graph rules and `suite_entry_digest`, the `describe` result with evaluator needs, and the `evaluate` params, run context with `context_digest`, notifications and protocol constants, all published and drift-checked.
**Pre-conditions**:
- [ ] [cmd] DEP(wire_foundations), DEP(wire_trace) and DEP(golden_oracle) hold: `for l in wire_foundations wire_trace golden_oracle; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`
- [ ] [cmd] `test -s crates/candler-core/tests/oracle/vectors/context_digest.toml && test -s crates/candler-core/tests/oracle/vectors/suite_entry_digest.toml`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
**Success Gates**:
- ✅ [cmd] `cargo nextest run -p candler-schemas -E 'binary(guard_evaluator_protocol)'` passes, holding three B tests: `tests/vectors/suite/cont_train.toml`, translated by the committed script from `eval.json` at `htn-creator-eval@0.4.2`, validates as a suite of 43 entries whose id set the test lists by hand, with the 6 entries of evals 3 and 4 (B1, B2, B3, B7, B8, B9) carrying `applies_to` `plan.config ∈ ["with_skill"]`; a suite-graph table (a duplicate id, a dependency on an unknown id, a two-entry cycle, each refused with the ids named, and the valid graph accepted); an `outcome` notification whose record carries a `{legacy: …}` cause is refused while the same record with a `{reason: …}` cause is accepted, and `OutcomeRecord` itself still accepts the legacy cause
- ✅ [cmd] `cargo nextest run -p candler-schemas -E 'binary(guard_context_digest)'` passes, holding two B tests: `context_digest` of each case equals the oracle's `context_digest.toml`, and a hand-written table shows that changing `root`, `trace`, `raw`, `scratch` or an `abs` path keeps the digest while changing an artifact's `sha256` or a metadata value changes it; `suite_entry_digest` equals `suite_entry_digest.toml` and each twin differing only in `id`, `group`, `evaluator` or `deadline_ms` has the same digest
- ✅ [cmd] `! git grep -n 'execution::' -- crates/candler-schemas/src/evaluation.rs`
- ✅ [cmd] STD, DRIFT and SUBJ pass
- ✅ [nl] The leaf report has a bite mutation per B test; api_review (end of this level) reports no open finding on this leaf
**References**: [R01 §Plug-ins, §Evaluator protocol, §Wire encodings, §Data Model (Outcome record)](../../../__design__/architecture.md) — every message and field; `.claude/skills/htn-creator-workspace/infrastructure/evals/eval.json` in mcp-python-ingestion at tag `htn-creator-eval@0.4.2` — 13 evals, 43 expectations; [R07](../../candler-v1/contracts/parity/README.md) — M2a speaks this protocol

## Step 1: Define the plug-in manifest and the suite file
**Goal**: A plug-in manifest and a suite file are read from TOML into validated values, and a suite's dependency graph is checked.
**Implementation Logic**:
1. `PluginManifest {schema: "plugin-manifest/1", name: NameToken, version: semver::Version, protocol: ProtocolVersion {major: u32, minor: u32}, command: Vec<String> (non-empty, relative to the plug-in directory), environment: PluginEnvironment}`; `PluginEnvironment`, tagged on `kind`: `uv {project: RelPath, lock: RelPath}` or `none`. `PluginManifest::from_toml_str`.
2. `SuiteFile {schema: "suite/1", expectations: Vec<SuiteEntry>}`; `SuiteEntry {id: ExpectationId, group?: NameToken, role: Role, evaluator: EvaluatorRef {name: NameToken, range: semver::VersionReq}, kind: NameToken, params: serde_json::Map, depends_on: Vec<ExpectationId>, applies_to?: Vec<Condition>, deadline_ms?: u64}`; `Condition {path: FieldPath, any_of: Vec<JsonValue>}` (non-empty). An entry applies to a run when every condition's path holds one of its values. `Role` is the outcome module's closed enum.
3. `SuiteFile::from_toml_str` refuses a duplicate id, a `depends_on` naming no entry, and a cycle; each error names the ids involved.
4. `SuiteEntry::digest(&self) -> Digest`: `Digest::of_canonical` over `{kind, params, role, depends_on (sorted), applies_to?}` (R01 §Digests of a request).
5. `tests/vectors/suite/{translate_eval_json.py,cont_train.toml}`: the script reads `eval.json` with `git show` and writes one entry per expectation: `group` the eval id, `kind` the `grading_spec.type`, `params` the `grading_spec` without `type`, plus `text`, `question`, `tier` and `scale`, `evaluator` `htn-creator-eval@^0.4`, and `applies_to` from the eval's `configurations` on `plan.config`. The test never reads `eval.json`.
**Deliverables**: `crates/candler-schemas/src/evaluation.rs` (`PluginManifest`, `ProtocolVersion`, `PluginEnvironment`, `SuiteFile`, `SuiteEntry`, `SuiteEntryBuilder`, `EvaluatorRef`, `Condition`, `EvaluationError`, `SuiteFile::from_toml_str`, `SuiteEntry::digest`, `PLUGIN_MANIFEST_SCHEMA`, `SUITE_SCHEMA`); `crates/candler-schemas/tests/guard_evaluator_protocol.rs`; `crates/candler-schemas/tests/vectors/suite/`
**Consistency Checks**: `cargo nextest run -p candler-schemas -E 'binary(guard_evaluator_protocol)'` (expected: PASS)
**Commit**: `feat(schemas): define the plug-in manifest and the suite file`

## Step 2: Define the describe result and evaluator needs
**Goal**: What an evaluator says about itself, and what each kind needs from a run, are typed.
**Implementation Logic**:
1. `DescribeResult {protocol: ProtocolVersion, name: NameToken, version: semver::Version, instruments: Vec<Instrument>, environment: EvaluatorEnvironment {lock_digest?: Digest, packages: BTreeMap<String, String>}, kind: EvaluatorKind (deterministic | probabilistic), kinds: Vec<KindSpec {kind: NameToken, params_schema: serde_json::Value, needs: Needs, default_deadline_ms: u64}>, reuse?: serde_json::Value}`. `Instrument` is the outcome module's type.
2. `Needs {trace: bool, artifacts: Vec<String> (run-relative globs), snapshots: Vec<NameToken>, inputs: Vec<NameToken>, metadata: Vec<FieldPath>}`, every list present.
3. `ProtocolVersion::accepts(&self, offered: &ProtocolVersion) -> Result<(), VersionMismatch>`: same major and an offered minor no newer than ours; the error names the side to upgrade.
**Deliverables**: `crates/candler-schemas/src/evaluation.rs` (`DescribeResult`, `EvaluatorEnvironment`, `EvaluatorKind`, `KindSpec`, `Needs`, `VersionMismatch`, `ProtocolVersion::accepts`)
**Consistency Checks**: `cargo clippy -p candler-schemas --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(schemas): define the describe result and evaluator needs`

## Step 3: Define evaluate params, run context and notifications
**Goal**: The `evaluate` request, its run context and digest, the notifications and the protocol constants are typed.
**Implementation Logic**:
1. `RunContext {run: RunKey, root: String, trace?: String, raw: Vec<ContextRaw {stream: StreamName, path: String}>, artifacts: Vec<ContextArtifact {path: RelPath, abs: String, sha256: Digest}>, snapshots: Vec<ContextRef {kind: NameToken, path: String, digest: Digest}>, inputs: Vec<ContextRef>, metadata: BTreeMap<FieldPath, JsonValue>}`. Absolute paths are plain strings valid where the evaluator runs.
2. `RunContext::context_digest(&self) -> Digest` over `{artifacts: [{path, sha256}], snapshots: [{kind, digest}], inputs: [{kind, digest}], metadata}`, lists sorted by `path` or `(kind, digest)`, every key present.
3. `EvaluateParams {context: RunContext, items: Vec<Item {expectation_id, kind, params, deadline_ms: u64}>, prior: Vec<OutcomeRecord>, scratch: String}`; `EvaluateResult {reported: u64}`.
4. Notifications: `OutcomeParams {record: OutcomeRecord}`, whose deserialisation and builder refuse a record with a `legacy` cause; `ProgressParams` and `LogParams` (a free `serde_json::Map`); `CancelParams {request_id: RequestId}`.
5. Constants: `METHOD_DESCRIBE`, `METHOD_EVALUATE`, `METHOD_OUTCOME`, `METHOD_PROGRESS`, `METHOD_LOG`, `METHOD_CANCEL`, `ERROR_REQUEST_CANCELLED = -32800`, published as `const` in the schema.
6. `RunContext`, `Condition` and `Needs` use `FieldPath`, `RunKey` and digests only, never `execution` types (a sibling's module).
7. `tests/guard_context_digest.rs` reads `../candler-core/tests/oracle/vectors/{context_digest,suite_entry_digest}.toml`.
**Deliverables**: `crates/candler-schemas/src/evaluation.rs` (`RunContext`, `ContextRaw`, `ContextArtifact`, `ContextRef`, `RunContext::context_digest`, `EvaluateParams`, `Item`, `EvaluateResult`, `OutcomeParams`, `ProgressParams`, `LogParams`, `CancelParams`, the method and error constants); `crates/candler-schemas/tests/guard_context_digest.rs`
**Consistency Checks**: `cargo nextest run -p candler-schemas -E 'binary(guard_context_digest) | binary(guard_evaluator_protocol)'` (expected: PASS)
**Commit**: `feat(schemas): define evaluate params, run context and notifications`

## Step 4: Publish the evaluator protocol schemas
**Goal**: The plug-in manifest, suite and evaluator message roots are generated and drift-checked, and the scaffolding is gone.
**Implementation Logic**:
`evaluation::roots()` returns `plugin-manifest`, `suite`, `evaluator-describe-result`, `evaluator-evaluate-params` and `evaluator-notification` (a union of the four notification params, keyed by method). Bless and read the diff. Delete every `mod scaffold` and write the bite record.
**Deliverables**: `crates/candler-schemas/src/evaluation.rs` (`roots`, `Notification`); `crates/candler-schemas/schema/{plugin-manifest,suite,evaluator-describe-result,evaluator-evaluate-params,evaluator-notification}.schema.json`
**Consistency Checks**: `cargo nextest run -p candler-schemas -E 'binary(schema_drift) | test(no_published_pattern_depends_on_the_regex_engine)' && ! git grep --untracked -n 'mod scaffold' -- crates/candler-schemas` (expected: PASS)
**Commit**: `feat(schemas): publish the evaluator protocol schemas`
