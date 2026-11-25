# Schema: exit and needs

**Goal**: The wire can say what a plug-in may be served and what a shell call returned: a trace line's `tool_result` carries an optional `exit` (`candler_schemas::process::ExitStatus`), `Needs` has `raw` and `manifest` (default `false`), `RunContext` has `manifest`, `trace` and `manifest` are `{path, abs, sha256}` objects, `ContextRaw` carries `sha256`, and `context_digest` adds a key for each served part only when it is served. The one reader of each refuses what it does not understand (critical path)
**Estimate**: Sonnet, $8, cap $16. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use the target dir the campaign README names; the leaf removes it at its merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/run-context HEAD` succeeds
**Success Gates**:
- ⬜ A trace line `{"kind":"tool_result","is_error":true,"exit":{"kind":"code","code":3}}` reads, and the result keeps `exit` when written back. `exit` given as `{"code":3}`, as a string, or on a `tool_call` line is refused. A `tool_result` with no `exit` still reads. On the base, the line with `exit` is refused as an unknown field
- ⬜ In the shared wire vectors (`crates/candler-schemas/tests/fixtures/rpc/`), which the Rust reader and the Python helper's `test_guard_wire_vectors.py` both read, a `describe` response whose needs omit `raw` and `manifest` is valid, and one whose needs set `raw` to a non-boolean, or carry an unknown key, is invalid. Both readers decide every vector alike
- ⬜ The context digest vectors keep every existing row unchanged and green, and new rows (contexts serving a trace, raw streams, a manifest), written by the independent Python generator over python `rfc8785`, pass. Under the bite "the trace key is always added" the old rows go red; under "the key is left out when served" the new rows go red
- ⬜ The marker stays `normalised-trace/1`: the leaf report cites `__design__/architecture.md` §Wire encodings on why an optional field leaves it unchanged, or stops and tells the lead if that section says otherwise
- ⬜ The JSON Schemas under `crates/candler-schemas/schema/` and the helper's generated models (`python/candler-evaluator/scripts/gen_models.py`) are regenerated, and the helper's `pytest` passes with the smallest change to its hand-written code that keeps it reading the new shapes; the helper's new API belongs to `readers/helper_context.md`
- ⬜ The doc comments of `Needs`, `RunContext` and `ToolResult` (which reach the schemas) state each need, what each served part reveals of the run (once, with no domain judgement), that `is_error` is not comparable across harnesses for shell calls, and that `exit` is absent for a non-shell, denied or uncoded result. `docs/contract/evaluator-protocol.md`'s generated sections are regenerated, and its Notes stay within the page's 120 words, each with its stamp (CONTRIBUTING.md §Contract pages)
- ⬜ Both trace readers compile against the new constructor and write no `exit` yet (`readers/` fills it)
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass with the target dir the campaign README names; helper `uv run pytest`, `uv run ruff check` pass; `container-e2e` is green on the pull request (it touches `crates/**`)
**References**: [plan §The exit-status field, §Each need, §Wire shape of a served file, §Test plan](../../__reports__/run-context/00-plan_run_context_v0.md). Code:
- `crates/candler-schemas/src/trace.rs`: `KnownKind::ToolResult`, `EventKind::tool_result`;
- `crates/candler-schemas/src/process.rs`: `ExitStatus` (reused as is);
- `crates/candler-schemas/src/evaluation.rs`: `Needs`, `RawRunContext`, `RunContext`, `ContextRaw`, `ContextArtifact`, `context_digest`;
- `crates/candler-schemas/tests/it/guard_trace_wire.rs`, `guard_evaluator_protocol.rs`, `guard_context_digest.rs`; `crates/candler-schemas/tests/fixtures/rpc/`; the context digest vectors and their generator under `crates/candler-core/tests/oracle/vectors/`;
- `python/candler-evaluator/tests/test_guard_wire_vectors.py`, `scripts/gen_models.py`.

## Step 1: Guards on the base
**Goal**: Every guard this leaf owns is written first and seen red on the base.
**Implementation Logic**:
Add the `exit` rows to the trace reader's table in `guard_trace_wire.rs` (hand-written lines, as above). Add the `Needs` vectors to `fixtures/rpc/valid` and `fixtures/rpc/invalid`, with their `PROVENANCE.toml` entries. Extend the context digest generator with contexts that serve a trace, raw streams and a manifest, and write their rows; existing rows are not regenerated. Run each guard on the base and record the failing message in the leaf report.
**Deliverables**: the guard rows and vectors named above; the generator's new cases
**Consistency Checks**: on the base, every new row fails and every existing digest row passes (expected: PASS)
**Commit**: `test(schemas): state the exit field, the new needs and their digest keys`

## Step 2: The wire
**Goal**: The guards pass on the tip.
**Implementation Logic**:
Add `exit: Option<ExitStatus>` to `ToolResult` with the same `default`, `skip_serializing_if` and `present` attributes its siblings use, and an `exit` parameter to `EventKind::tool_result`; both readers pass `None`. Add `raw` and `manifest` to `Needs` as booleans that default to `false` when absent, refusing any other type. Make `trace` and `manifest` on `RunContext` optional `{path, abs, sha256}` objects in `ContextArtifact`'s idiom (reuse the type if its doc reads true for them, or give the shape one name used by all three), and add `sha256` to `ContextRaw`. Add to `context_digest` a `trace`, `raw` (`[{stream, sha256}]`, sorted by stream) and `manifest` key, each only when that part is present, and update its doc. Bless the schemas, regenerate the helper's models, keep `pytest` green, and rewrite the contract page's sections on needs and the context.

Commit one release line per commit, as CONTRIBUTING.md §Versioning requires (a release-triggering commit that touches `crates/candler-schemas/` touches no other crate):
- the commit named below, with a `BREAKING CHANGE:` footer and its migration path: `context.trace` becomes an object and `ContextRaw` gains a required `sha256`, so a context valid before is invalid now (none was ever written with a trace or raw streams);
- `refactor(adapter-claude-code): …` and `refactor(adapter-opencode): …` for the call sites, if the build needs them in their own commits;
- `feat(evaluator-helper): …` for the regenerated models and the smallest hand-written change;
- `docs(evaluator-protocol): …` for the contract page.
**Deliverables**: `crates/candler-schemas/src/{trace,evaluation}.rs`; `crates/candler-schemas/schema/*.json`; both readers' call sites; `python/candler-evaluator/src/candler_evaluator/_models/`; `docs/contract/evaluator-protocol.md`
**Consistency Checks**: the guards of Step 1 on the tip (expected: PASS)
**Commit**: `feat(schemas)!: carry a shell call's exit and the stored run's needs`

## Step 3: Bite and verify
**Goal**: Each guard goes red under its named bite.
**Implementation Logic**:
On a scratch branch: type `exit` as a free JSON value and see the trace rows go red; give `raw` a default that accepts any value and see the invalid vectors go red; apply each digest bite in turn. Record each failing message. Delete every `confirm_` test. A fresh Sonnet verifier gets this leaf, the report and the diff, and is asked to find a line or context the wire now reads in two ways, or a digest that changed for a context serving none of the new parts.
**Deliverables**: `__reports__/run-context/leaves/schema_exit_needs_v0.md`
**Consistency Checks**: `git grep -n confirm_ -- crates python` prints nothing (expected: PASS)
**Commit**: `reports(schemas): close the exit field and the new needs`
