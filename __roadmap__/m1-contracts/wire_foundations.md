# Wire foundations: values, RFC 8785, child record

**Goal**: `candler-schemas` gains resolved values with their sources, RFC 8785 canonical JSON and canonical digests, the child record and the exit status, and the pattern guard covers every published root automatically.
**Pre-conditions**:
- [ ] [cmd] `git log --format=%s feat/m1-contracts | grep -qx 'build(repo): scaffold the m1 crates, modules and dependencies'`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
- [ ] [cmd] `node --version && uv --version`
**Success Gates**:
- ✅ [cmd] `cargo nextest run -p candler-schemas -E 'binary(guard_jcs)'` passes: RFC 8785 Appendix B number vectors, the RFC's §3.2.3 sorting sample, the committed Node-generated double vectors, and one refusal table (integers beyond ±(2^53 − 1), a lone surrogate escape in JSON text)
- ✅ [cmd] `cargo nextest run -p candler-schemas -E 'binary(guard_pattern_vectors)'` passes, and `cargo nextest run -p candler-schemas -E 'test(no_published_pattern_depends_on_the_regex_engine)'` passes over every root of `generated_schemas()`
- ✅ [cmd] STD, DRIFT and SUBJ pass
- ✅ [nl] The leaf report records the one-off 10^5 random-double comparison against Node and the `uv run` cross-engine probe of each new pattern (Rust, Python `re`, Node), and a bite mutation per B test
- ✅ [nl] The level's Grit review reports no open finding
**References**: [R01 §Operation API (Value sources), §Process supervision, §Wire encodings](../../__design__/architecture.md) — value sources, child record, exit status, digests over RFC 8785; [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785) — §3.2.2.3 numbers, §3.2.3 sorting, Appendix B; [R04](../../__reports__/m1-contracts/00-test_definition_v0.md) — guards `guard_jcs`, `guard_pattern_vectors`, the ruling on `check::<T>()`

## Step 1: Add job ids, field paths and resolved values
**Goal**: The value vocabulary the manifest, the resolver and the importer share exists in `candler_schemas::values`.
**Implementation Logic**:
1. Text types (with the `text_type!` pattern of `primitives.rs`, which this leaf alone edits):
   - `FieldPath`: dotted, `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$` ending with the end anchor; `segments()`, `starts_with(&FieldPath)`, `join(&str)`.
   - `JobId`: a UUIDv7, lowercase hyphenated, the same format as `RequestId`.
   - `Marker`: the value of `CANDLER_RUN`, `^[0-9A-Za-z][0-9A-Za-z._-]*$`; `Marker::for_run(&RunKey)` gives the path form `<cell hex>-r<n>-a<m>`.
2. `ValueSource`, tagged on `kind` (R01 §Value sources): `explicit_argument`, `experiment_file`, `project_config`, `user_config`, `derived {step: NameToken}`, `core_default`, `imported {file: RelPath}`, `unrecorded`. `#[non_exhaustive]`.
3. `JsonValue`: a JSON value that can be hashed: not `null` at the top, integers within ±(2^53 − 1), finite numbers, objects read from objects only. Constructed with `JsonValue::new(serde_json::Value) -> Result<_, ValuesError>` (calls `canonical::check` from Step 2; in this step the check refuses `null` and out-of-range integers directly).
4. `ResolvedValue {path: FieldPath, value?: JsonValue}` (absent value = `unrecorded`), `SourceEntry {path, source: ValueSource}`, `MeasuredValue {path, value: JsonValue, source: ValueSource}`. Builders validate; `null` is refused on input.
5. Extend `generate.rs` `mod tests` only: `no_published_pattern_depends_on_the_regex_engine` also asserts every pattern ends with `END_ANCHOR`, so a type added to any root is covered without a list. Keep `primitives.rs` `check::<T>()` (ruling in R04) and add the three new text types to it.
6. `tests/guard_pattern_vectors.rs`: about 40 hand-written accept/refuse strings per new pattern (`FieldPath`, `Marker`), asserted through both the Rust type and the `jsonschema` validator of the schema fragment. A `uv run` script under `tests/vectors/patterns/probe.py` (with a Node twin) checks the same strings under Python `re` and ECMA 262; it runs once per pattern change and its output goes in the leaf report, not in CI.
**Deliverables**: `crates/candler-schemas/src/values.rs` (`ValueSource`, `JsonValue`, `ResolvedValue`, `SourceEntry`, `MeasuredValue`, `ValuesError`, builders); `crates/candler-schemas/src/primitives.rs` (`FieldPath`, `JobId`, `Marker`); `crates/candler-schemas/src/generate.rs` (`mod tests` assertion); `crates/candler-schemas/tests/guard_pattern_vectors.rs`; `crates/candler-schemas/tests/vectors/patterns/{probe.py,probe.mjs,cases.toml}`
**Consistency Checks**: `cargo clippy -p candler-schemas --all-targets -- -D warnings && cargo nextest run -p candler-schemas -E 'binary(guard_pattern_vectors) | test(no_published_pattern_depends_on_the_regex_engine)'` (expected: PASS)
**Commit**: `feat(schemas): add job ids, field paths and resolved values`

## Step 2: Canonicalise JSON values per RFC 8785
**Goal**: One function turns a JSON value into its RFC 8785 text, and digests of JSON values are taken over it.
**Implementation Logic**:
1. `candler_schemas::canonical`:
   - `to_jcs(&serde_json::Value) -> Result<String, CanonicalError>`: object keys sorted by UTF-16 code units; strings escaped per §3.2.2.2; numbers per ECMAScript `Number.prototype.toString` (`ryu-js`), integers held as `i64`/`u64` written in decimal and refused beyond ±(2^53 − 1); `-0` written as `0`.
   - `from_text(&str) -> Result<serde_json::Value, CanonicalError>`: parses JSON text and refuses a lone surrogate escape and duplicate keys, which a canonical form cannot represent.
   - `check(&serde_json::Value) -> Result<(), CanonicalError>`: the same refusals without writing.
2. `Digest::of_canonical(&serde_json::Value) -> Result<Digest, CanonicalError>` in `primitives.rs`: `sha256:` + hex of the `to_jcs` bytes (`sha2`).
3. `tests/guard_jcs.rs` (3 B tests):
   - RFC vectors: Appendix B's number table and the §3.2.3 sorting sample, as hand-copied literals with the RFC section cited.
   - Doubles: `tests/vectors/jcs/gen_doubles.mjs` writes `doubles.tsv` (bit pattern hex → `String(x)`) for about 2,000 stratified doubles: powers of 2, subnormals, values around 1e21 and 1e-7, `MAX_VALUE`, `MIN_VALUE`, `-0`, random mantissas. The test reads the TSV; the script and the Node version are named in the file header.
   - One refusal table with a positive control per row: `2^53` and `-(2^53)` refused while `2^53 − 1` passes; `"\ud800"` refused while `"😀"` passes; a duplicate key refused while distinct keys pass.
4. Once, not in CI: 10^5 random doubles through Node and `to_jcs`, compared; the count and the seed go in the leaf report.
**Deliverables**: `crates/candler-schemas/src/canonical.rs` (`to_jcs`, `from_text`, `check`, `CanonicalError`); `crates/candler-schemas/src/primitives.rs` (`Digest::of_canonical`); `crates/candler-schemas/src/values.rs` (`JsonValue::new` uses `canonical::check`); `crates/candler-schemas/tests/guard_jcs.rs`; `crates/candler-schemas/tests/vectors/jcs/{gen_doubles.mjs,doubles.tsv,README.md}`
**Consistency Checks**: `cargo nextest run -p candler-schemas -E 'binary(guard_jcs)'` (expected: PASS)
**Commit**: `feat(schemas): canonicalise JSON values per RFC 8785`

## Step 3: Define the child record and exit status
**Goal**: The record the supervisor writes for every child, and the exit status shared by drivers and events, exist in `candler_schemas::process`.
**Implementation Logic**:
1. `ProcessStart`, tagged on `kind`: `linux {boot_id, ticks: u64}`, `darwin {sec: u64, usec: u32}`, `unavailable`. `boot_id` is a lowercase hyphenated UUID text type. `#[non_exhaustive]`.
2. `ChildRecord {pid: u32, pgid: u32, start: ProcessStart, marker: Marker, spawned_at: Timestamp}` with `CHILD_RECORD_SCHEMA = "child-record/1"` carried as a `schema` field (it is a standalone file). Builder refuses `pid = 0` and `pgid = 0`.
3. `ExitStatus`, tagged on `kind`: `code {code: i32}`, `signal {signal: i32}`. Closed (a process ends one of two ways): `// EXHAUSTIVE:`.
4. Liveness is not decided here (the supervisor owns it); the record's doc says that `unavailable` never reads as alive.
**Deliverables**: `crates/candler-schemas/src/process.rs` (`ChildRecord`, `ChildRecordBuilder`, `ProcessStart`, `BootId`, `ExitStatus`, `ProcessError`, `CHILD_RECORD_SCHEMA`)
**Consistency Checks**: `cargo clippy -p candler-schemas --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(schemas): define the child record and exit status`

## Step 4: Publish the child record schema
**Goal**: `child-record.schema.json` is generated and drift-checked, and the leaf's scaffolding is gone.
**Implementation Logic**:
`process::roots()` returns the `child-record` root; `values::roots()` and `canonical` have none. Bless the file and read its diff. Delete every `mod scaffold`; promote nothing without a `Guards:` line. Write the bite record.
**Deliverables**: `crates/candler-schemas/src/process.rs` (`roots`); `crates/candler-schemas/schema/child-record.schema.json`
**Consistency Checks**: `cargo nextest run -p candler-schemas -E 'binary(schema_drift)' && ! git grep --untracked -n 'mod scaffold' -- crates/candler-schemas` (expected: PASS)
**Commit**: `feat(schemas): publish the child record schema`
