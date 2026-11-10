# Wire

**Goal**: `experiment/1` declares inputs, keeps a table at a table-valued path whole and names the treatment's source, and `run-manifest/1` records evaluator-only inputs and a derived value's location. All of it lands in one `feat(schemas)` commit that touches no other crate and moves no existing digest.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone: `git merge-base --is-ancestor feat/m2c-wire-alignment HEAD`
- [ ] [nl] OWNED: `crates/candler-schemas/**` only. No other crate, no `Cargo.lock`
**Success Gates**:
- ⬜ [cmd] Only the schemas crate changed: `test -z "$(git diff --name-only feat/m2c-wire-alignment..HEAD | grep -v '^crates/candler-schemas/')"`
- ⬜ [cmd] The workspace builds and passes unchanged on top of the new wire: `cargo check --workspace --all-targets && cargo nextest run --workspace --no-tests=fail`
- ⬜ [cmd] W1 table values: `cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/^table_value_/)'`. Values at or below `prompt.values` reach the cell as one object, in nested or quoted spelling, from `[fixed]` and from factor levels. A table at any undeclared path still flattens. An empty table at `prompt.values` is `EmptyTable`, and an overlap is refused. Oracle: hand-written cells
- ⬜ [cmd] W2 unchanged cells: `cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/^unchanged_cells_/)'`. `cont_train.toml` and the experiment texts of `tests/vectors/experiment/legacy_cells.toml`, none of which has a new field or a table under `prompt.values`, list cells whose digests equal those recorded in Step 1 from the b017d31 code. Oracle: that pre-change baseline
- ⬜ [cmd] W3 input-list digest: `cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/^input_list_digest_/)'` and `uv run --with rfc8785 python crates/candler-schemas/tests/vectors/gen_input_list.py --check`. Oracle: the Python second engine, which runs no Candler code
- ⬜ [cmd] Existing digests hold: `cargo nextest run -p candler-schemas --no-tests=fail -E 'binary(guard_cell_digest) | binary(guard_execution_wire) | binary(guard_experiment_file)'`, with no `-` line in any existing vector file: `git diff --numstat feat/m2c-wire-alignment..HEAD -- crates/candler-schemas/tests/vectors | awk '$2!=0{b=1}END{exit b}'`
- ⬜ [cmd] DRIFT holds, and the evaluator helper's models do not move: `(cd python/candler-evaluator && uv run --frozen python scripts/gen_models.py && test -z "$(git status --porcelain -- src)")`
- ⬜ [cmd] Three commits in this order, oldest first: the `test(schemas)` baseline, the single `feat(schemas)`, the `test(schemas)` renames: `test "$(git log --reverse --format=%s feat/m2c-wire-alignment..HEAD | sed -E 's/:.*//' | paste -sd, -)" = "test(schemas),feat(schemas),test(schemas)"`
- ⬜ [cmd] No domain name in the schemas' sources: `! git grep -n -i -w -E 'htn[a-z_-]*|rikyu|tier[a-z_-]*|gtpyhop' -- ':(glob)crates/candler-schemas/src/**'`
- ⬜ [cmd] STD (the `-p candler-schemas` forms, plus `cargo check --workspace --all-targets`), LINUX and SUBJ pass
- ⬜ [nl] The leaf report records the bites:
  - W1: flattening through `prompt.values`; no flattening anywhere;
  - W2: the factor product reordered;
  - W3: the list left unsorted;
  - existing digests: `skip_serializing_if` removed from `evaluation_inputs` or from `Derived.from`, which turns `guard_cell_digest` red.
- ⬜ [nl] The `wire` verifier closes with no open finding
**References**: [R01 §Experiment file, §Run manifest, §Evaluator protocol (`inputs[] {kind, path, digest}`), §Wire encodings](../../__design__/architecture.md); [R10 D1–D4, D9, guards W1–W3](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md); [CONTRIBUTING §Schema line](../../CONTRIBUTING.md); campaign README §Gotchas, "Shared constraints"

## Step 1: Record the cells today's code lists
**Goal**: A baseline from the b017d31 code, taken before any edit, of the cells of every experiment file in the repository that has no new field.
**Implementation Logic**:
1. Copy the experiment TOML texts that tests build inline into `crates/candler-schemas/tests/vectors/experiment/legacy_cells.toml`, one entry per text with a `name` and the TOML as a string. The texts are in `crates/candler-core/tests/support/run_plan.rs`, `crates/candler-core/tests/support/job_orchestrate.rs`, `crates/candler/tests/support/cli_run.rs` and `crates/candler/tests/guard_cli_kill_live.rs`. Read them; do not edit them.
2. Before touching `src/`, compute for each entry, and for `tests/vectors/experiment/cont_train.toml`, the sha256 of the RFC 8785 form of the JSON of `ExperimentFile::from_toml_str(..)?.cells()`. Use a throwaway test, which you delete. Write the digests into the same vector file.
3. Add the W2 guard `unchanged_cells_…` to `tests/guard_experiment_file.rs`. It reads the vector file and recomputes. Its doc says `Oracle: digests recorded from the b017d31 code before this change`. Commit it before any `src/` edit, so the history shows that the baseline came from the old code.
**Deliverables**: `crates/candler-schemas/tests/vectors/experiment/legacy_cells.toml`; `crates/candler-schemas/tests/guard_experiment_file.rs`: `unchanged_cells_…`
**Consistency Checks**: `git diff --quiet feat/m2c-wire-alignment -- crates/candler-schemas/src && cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/^unchanged_cells_/)'` (expected: PASS)
**Commit**: `test(schemas): pin the cells of experiment files with no new field`

## Step 2: The wire types, guards first
**Goal**: The new optional fields and the table-valued rule exist, with their guards seen red first, and the schema files are blessed in one commit.
**Implementation Logic**:
1. Write W1 and W3 first and see them fail.
2. `src/execution.rs`:
   - `pub const TABLE_VALUED_PATHS: [&str; 1] = ["prompt.values"]`.
   - `flatten_fixed`/`flatten_into` and the level fill (`Level`, `fill_leaves`) stop at a table-valued path. Values written at or below it, nested or quoted, fold into one JSON object for that path. A factor whose path is below a table-valued path folds into that object per cell, merged with the `[fixed]` part. A key given twice is still `Duplicate`, an empty table is still `EmptyTable`, and `check_overlaps` still refuses overlaps.
   - `ExperimentInput { kind: Name, path: String, at: Option<String>, audience: InputAudience }`, with `InputAudience { Agent, Evaluators }` (`#[non_exhaustive]`, default `Agent`). `ExperimentFile` gains `inputs: Vec<ExperimentInput>` (TOML `[[inputs]]`, default empty) with an accessor. The wire refuses `at` absolute or holding `..`. Duplicate kinds and the kind `build_inputs` are refused too: the wire has both lists, so it can.
   - `TreatmentSpec { name, tag?, commit?, source? }` (map-only, `deny_unknown_fields`). It is the type of `ExperimentFile`'s `treatment` and of its accessor, and `fn reference(&self) -> TreatmentRef` drops the source. `TreatmentRef` is unchanged.
   - `RunManifest` gains `evaluation_inputs: Vec<DigestRef>`, optional, outside the hash, absent when empty, sorted with no duplicates, like `comparable.inputs`.
   - Add `InputEntry { path: RelPath, sha256: Digest }` and `#[must_use] pub fn input_list_digest(entries: &[InputEntry]) -> Result<Digest, …>`, which sorts by `path`, refuses duplicates, and hashes the RFC 8785 form with the crate's canonical writer.
3. `src/values.rs`: `ValueSource::Derived` gains `from: Option<String>`: the location the step read, as the experiment file wrote it, absent when `None`. Keep `ValueSource::derived(step)` and add `derived_from(step, from)`.
4. Keep the API additive so that `candler-core` and `candler` compile untouched. If a change cannot be additive, stop and report it.
5. `tests/vectors/gen_input_list.py` (`uv run --with rfc8785`, `--check` mode) writes `tests/vectors/input_list_digest.toml`: an empty list is refused, plus one file, several files out of order, and non-ASCII paths. The W3 guard reads that file.
6. Update `tests/guard_execution_wire.rs`'s map-only cases for the new types. Run DRIFT with `CANDLER_BLESS_SCHEMAS=1`.
7. Commit once. The body states D1–D4, that `experiment/1` and `run-manifest/1` keep their tags because every field is optional and the cell-digest function is unchanged, and that a file with a table under `prompt.values` now lists that table as one value.
**Deliverables**: `crates/candler-schemas/src/execution.rs`: `TABLE_VALUED_PATHS`, `ExperimentInput`, `InputAudience`, `TreatmentSpec`, `RunManifest::evaluation_inputs`, `InputEntry`, `input_list_digest`; `crates/candler-schemas/src/values.rs`: `ValueSource::Derived::from`, `ValueSource::derived_from`; `crates/candler-schemas/schema/*.json` (blessed); `crates/candler-schemas/tests/guard_experiment_file.rs`: `table_value_…`, `unchanged_cells_…`; `crates/candler-schemas/tests/guard_execution_wire.rs`; `crates/candler-schemas/tests/vectors/{gen_input_list.py,input_list_digest.toml,experiment/legacy_cells.toml}`
**Consistency Checks**: `cargo nextest run -p candler-schemas --no-tests=fail && cargo check --workspace --all-targets` (expected: PASS)
**Commit**: `feat(schemas): declare inputs, table values and a treatment source`

## Step 3: Domain names out of the schemas' unit tests
**Goal**: The schemas' `src/` test modules use neutral example names, so the rebuilt I1 guard can scan `crates/candler-schemas/src`.
**Implementation Logic**:
1. In `src/outcome.rs` and `src/primitives.rs` test modules, rename `htn-creator-eval` to `acme-eval`, `htn_creator_scripts` to `acme_scripts`, `tier-a` to `stage-a` and `rikyu@2026.10` to `zenith@2026.10`.
2. Do not change any assertion's meaning.
**Deliverables**: `crates/candler-schemas/src/outcome.rs`, `crates/candler-schemas/src/primitives.rs` (test modules only)
**Consistency Checks**: `! git grep -n -i -w -E 'htn[a-z_-]*|rikyu|tier[a-z_-]*|gtpyhop' -- ':(glob)crates/candler-schemas/src/**' && cargo nextest run -p candler-schemas --no-tests=fail` (expected: PASS)
**Commit**: `test(schemas): keep the schemas' unit tests free of domain names`
