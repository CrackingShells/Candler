# Adversarial review of the run context campaign

Read: `git diff origin/main...feat/run-context` at tip `297929773330b9ed5f77043c1cadf4f8187f5188`, through `git diff` and file reads, and `colgrep` for the call sites of the raw-stream naming rule and of trace validation. I read the diff first, then `02-seam_review_v0.md` and the leaf reports. Nothing was built, run or tested. Rulings 1 to 14 of the seam review are not re-raised. Where a finding touches a carried follow-up, it says so.

## Answers

**Is any stored part served by a second path, or copied by more than one routine?**
No second copy routine. `copy_stored` (`materialise.rs:389`) is the only copy, called once (`materialise.rs:495`) for artifacts, inputs, trace, raw streams and manifest. Before, `copy_artifact` was called from two sites.
There is a second *reader* of the raw naming rule: `stored_streams` (`assemble.rs:276-305`) parses `<stream>.jsonl.gz` with its own `RAW_SUFFIX`. `stream_of` (`runner/exec/mod.rs:415`) parses the same name, and `StoreLayout::raw` (`store/layout.rs:91`) builds it. The names `trace.jsonl` and `manifest.json` are also spelled in `assemble.rs:208,225`, beside `layout.trace` and `layout.manifest`. See findings 2 and 3.

**Does any reader accept what the schema refuses?**
- Rust trace readers: `TraceFile::read` refuses `exit: null`, a bare `{code}` and a string (`guard_trace_wire.rs`, new test). It accepts an unknown event kind as `EventKind::Unknown` (`trace.rs:905`), which the six-kind schema does not list. That is older than this diff and is not raised.
- Wire readers: `Needs.raw` and `Needs.manifest` take a boolean or are absent; `null` is refused (`evaluation.rs:1342-1347`; vector `needs_raw_not_boolean`). `ContextArtifact`, `ContextRaw` and `RunContext` refuse `null` through `present`.
- Python `read_trace` (`trace.py`) accepts what the Rust reader refuses: an empty file, a missing or second header, a `seq` gap, an event for an undeclared session, a `raw_ref` to an unlisted stream. Its own docstring lists them. AGENTS.md calls a more lenient second reading a defect. Finding 4.
- Python `ExitStatus.code` is an unbounded `int` where Rust has `i32`. This was ruled in seam review 2; not re-raised.
- `stored_streams` ignores a file in `raw/` that is not `<stream>.jsonl.gz` instead of refusing the run (follow-up carried in the seam review; finding 3 shows the second parser is what makes it unguarded).

**Does the context digest change for a context that serves none of the new parts?**
No. `context_digest` (`evaluation.rs:1805-1835`) builds the same four keys and adds `trace`, `raw` and `manifest` only when served. RFC 8785 sorts keys, so the switch from `json!` to a `Map` changes nothing. `context_digest.toml` is unchanged in the diff and `guard_context_digest.rs` still runs its rows against it; the new `context_digest_served.toml` rows are separate. The digest of a context that serves a part depends on which sibling kinds were sent (ruling 3); not re-raised.

**Does any doc, doc comment, contract page, README or CHANGELOG carry history or plan references, or say something the code does not do?**
- No plan, milestone, leaf or roadmap reference in source, README, contract pages or `__design__`. The `CHANGELOG.md` line "`trace` was a bare path and is now a `ServedFile`" is history, which is what a changelog is for.
- Claims the code does not back:
  - `READER` and `LIVE_READER`/`EXPORT_READER` docs say "the version changes whenever the reader's output does" (`claude-code/src/lib.rs:57`, `opencode/src/lib.rs:33,36`). The ratchet that enforced it (`digests.toml`) was deleted in this diff, so nothing enforces it now. Finding 7.
  - `PROVENANCE.toml` (claude-code fixtures, new block) says the recorded stream came from `fakemodel.py` "with one extra script, model id sh1, kept outside the repository". The fixture cannot be regenerated from the repository. Finding 8.
- Not checked: the `Needs.manifest` doc listing what the manifest holds (`evaluation.rs:1342`), which I did not compare with `RunManifest`.

**Did the refactor leave `materialise` simpler?**
Yes on paths: one copy routine, one call site, one `text_of` for the UTF-8 refusal that appeared three times. The cost moved. `Served` is a five-variant tag that `materialise` matches (`materialise.rs:496-518`); three arms repeat `ContextArtifact::new(.., text_of(&copy)?, digest)` and the `Input` arm discards the digest `copy_stored` returns. A fourth part would add an arm in `assemble`, `Served`, `materialise` and `union`. Finding 6.

**Exit-status readers (Claude Code `Bash`, OpenCode `shell`)**
- Non-shell call getting an `exit`:
  - OpenCode: gated at the source, `name == SHELL_TOOL && status == "completed"` (`export.rs:221`). The `read` row with `exit: 7` guards it.
  - Claude Code: `exit_of` is computed for every result, then cleared in `Registry::emit` unless the call is a `Bash` call of the same session (`sessions.rs:158-167`). Correct today; the gate lives in a second file and the first producer gives any non-error result `Code 0` (finding 5). The shared-call-id stream guards the join.
- Shell call getting a wrong one:
  - Claude Code returns `Code 0` for every non-error result with no `backgroundTaskId`, `timedOutAfterMs` or `interrupted` flag (`parse.rs:64-66`). The probe (`probe_shell_shapes_v0.md`) ran `exit 3`, `exit 2` with stderr, and a timeout. It did not run a command whose non-zero exit Claude Code's Bash tool reports as success, such as `grep` with no match or `diff` on differing files. From my knowledge of that tool, such a result is `is_error: false` with an interpretation string in `tool_use_result`, and the real exit is 1. If so, the reader writes exit 0 for a command that exited 1. I did not verify this; finding 1.
  - OpenCode: a `completed` call whose `metadata.exit` is absent or not an `i32` gets none. An `error` state gets none even if the harness recorded a code. Both are documented in the comment at `export.rs:217-220`.

**Tests: tautological, or expected values from the code under test?**
- No tautological guard found. Expected values come from outside the code under test: `jq` over the raw lines (`expected.toml`, `exits` rows), the fake model's script (`graded_trace_e2e.rs`), the store's own bytes (`guard_run_context.rs`), `rfc8785` vectors (`context_digest_served.toml`), hand-written wire lines.
- Weak points:
  - `stored_parts` in `guard_run_context.rs:123-145` lists `raw/` by directory listing, as `assemble` does. A stray file in `raw/` is served by the oracle and refused by nothing. No test pins that case.
  - `guard_wiring.rs` and `guard_registry.rs` hard-code `opencode-export@0.2.0`; this is a literal, not a read of the code, so the pair with `EXPORT_READER` is checked. Fine.
  - The Claude Code `exits` row for the timeout comes from a backgrounded call (`backgroundTaskId`), and the `timedOutAfterMs` branch of `exit_of` has no recorded stream behind it; only the synthetic `exit_join_stream` reaches `interrupted`.

## Findings

| # | file:line | Finding | Failure it would cause | Class | Severity |
|---|---|---|---|---|---|
| 1 | `crates/candler-harness-claude-code/src/parse.rs:64-66` | A non-error `Bash` result is read as `Code 0`. The probe did not run a command that Claude Code's tool reports as success on a non-zero exit (`grep` no match, `diff`). | A trace that says exit 0 for a command that exited 1; a kind that grades on exit passes a failed command. Unverified: probe one such command before merge. | defect | blocking, pending that probe |
| 2 | `crates/candler-core/src/evaluation/context/assemble.rs:208,220,225,272` | The workspace names `trace.jsonl`, `manifest.json`, `raw/<stream>.jsonl.gz` and `RAW_SUFFIX` repeat what `store/layout.rs` and `runner/exec/mod.rs:415` already spell. | A rename in the layout leaves the context serving a different name from the store, or a missing file read as `PartMissing`. | waste | minor |
| 3 | `assemble.rs:276-305` | `stored_streams` is a second reader of `<stream>.jsonl.gz` and skips a file, or a name `StreamName` refuses, instead of refusing the run. | A damaged `raw/` serves fewer streams than the run stored, with a context digest that looks valid. | defect (second, lenient reader) | minor |
| 4 | `python/candler-evaluator/src/candler_evaluator/trace.py:539-558` | `read_trace` checks lines, not order. The Rust `TraceFile::read` (`trace.rs:1299-1336`) refuses an empty file, a second header, a session after an event and a duplicate session. | A hand-supplied path (the helper takes `Path | str`) is graded as a trace Candler would have refused. The served copy is sha256-pinned, so the stored trace is safe. | defect | minor |
| 5 | `parse.rs:54-66` with `sessions.rs:158-167` | Claude Code decides `exit` in two places: `exit_of` gives every non-error result of any tool `Code 0`, and `Registry::emit` clears it for non-`Bash`. OpenCode gates at the source (`export.rs:221`). | A new caller of `block_draft` that skips the registry writes `exit 0` on a `Read` result. None today. | idiom | minor |
| 6 | `materialise.rs:496-518` | Three arms repeat `ContextArtifact::new(.., text_of(&copy)?, digest)`; the `Input` arm drops the digest. The `Served` tag is mirrored in `assemble`, `materialise` and `union`. | A fourth served part needs edits in four places. | idiom | minor |
| 7 | `crates/candler-harness-claude-code/src/lib.rs:57`, `crates/candler-harness-opencode/src/lib.rs:33-41` | The "version changes whenever the reader's output does" claim is unenforced after `digests.toml` was deleted. `EXPORT_READER` and `EXPORT_READER_VERSION` both hold `0.2.0`; `LIVE_READER` is a bare literal. | A reader change that forgets the bump goes unseen; the two `0.2.0` copies drift. | waste | minor |
| 8 | `crates/candler-harness-claude-code/tests/fixtures/PROVENANCE.toml` (new block, `shell_exit_stderr_timeout`) | The stream was recorded with a fake-model script kept outside the repository; `tests/fixtures/README.md` gives no way to record it again. | A reader change that needs a new recording cannot reproduce this stream. | waste | minor |
| 9 | `assemble.rs:192-208,261` with `materialise.rs:423` | Each served file is stat-ed (trace), hashed at assembly, copied and hashed again: three reads of every raw stream for every request of every run. | Cost grows with raw size times requests; the assembly hash only guards a store change between two steps of one request. | waste | minor |

## Files in the diff stat

Every file below was read in the diff unless marked. "Skim" means the diff was read for shape and for expected values, not line by line.

| File | Status |
|---|---|
| `.github/workflows/container-e2e.yml` | read |
| `__design__/architecture.md` | read |
| `__reports__/run-context/00-plan_run_context_v0.md` | skipped: the plan, read only for the ruling on "what a kind sees" via `02` |
| `__reports__/run-context/01-ruling_trace_served_v0.md` | skipped: ruling record |
| `__reports__/run-context/02-seam_review_v0.md` | read |
| `__reports__/run-context/leaves/probe_shell_shapes_v0.md` | read (grep for the shell shapes it covers) |
| `__reports__/run-context/leaves/cc_reader_exit_v0.md`, `graded_run_sees_trace_v0.md`, `helper_context_v0.md`, `oc_reader_exit_v0.md`, `schema_exit_needs_v0.md`, `serve_context_v0.md` | skipped: leaf reports; the code and tests were judged directly |
| `__roadmap__/m3-lean-cli/refusal_scope.tsv` | read |
| `__roadmap__/run_context/README.md`, `probe_shell_shapes.md`, `readers/README.md`, `readers/cc_reader_exit.md`, `readers/e2e/README.md`, `readers/e2e/graded_run_sees_trace.md`, `readers/e2e/review/README.md`, `readers/e2e/review/adversarial_review.md`, `readers/helper_context.md`, `readers/oc_reader_exit.md`, `readers/serve_context.md`, `schema_exit_needs.md` | skipped: roadmap task files, no code or contract claim |
| `assets/docker/fakemodel/fakemodel.py` | read |
| `crates/candler-core/src/evaluation/context/assemble.rs`, `materialise.rs`, `mod.rs`, `union.rs` | read |
| `crates/candler-core/src/evaluation/engine/run.rs`, `refusal.rs`, `registry/needs.rs` | read |
| `crates/candler-core/tests/fixtures/harness_baseline/census.json`, `run_totals.json` | skipped: generated data, no logic |
| `crates/candler-core/tests/it/guard_run_context.rs` | read |
| `crates/candler-core/tests/it/guard_wiring.rs` | read |
| `crates/candler-core/tests/oracle/vectors/context_digest_served.toml` | read |
| `crates/candler-core/tests/oracle/vectors/gen_live_context.py`, `gen_vectors.py`, `live_context_runs.toml` | skipped: generators and their output; the digest rule they project was checked against `evaluation.rs` |
| `crates/candler-harness-claude-code/README.md`, `src/lib.rs`, `src/parse.rs`, `src/sessions.rs` | read |
| `crates/candler-harness-claude-code/tests/fixtures/PROVENANCE.toml`, `README.md` | read |
| `crates/candler-harness-claude-code/tests/fixtures/digests.toml` | read (deletion) |
| `crates/candler-harness-claude-code/tests/fixtures/streams/shell_exit_stderr_timeout/expected.toml` | read |
| `.../streams/api_error`, `mini_clean`, `mini_incomplete`, `mini_zero_mappings`, `multi_init`, `subagent_denials`, `synthetic_mcp` `expected.toml` | skim: new `exits` rows, same rule |
| `.../streams/shell_exit_stderr_timeout/agent_stream.jsonl.gz`, `usage_totals.toml` | skipped: recorded data |
| `crates/candler-harness-claude-code/tests/it/guard_reader.rs` | read |
| `crates/candler-harness-opencode/src/export.rs`, `lib.rs`, `prepare.rs` | read |
| `crates/candler-harness-opencode/tests/fixtures/PROVENANCE.toml`, `streams/shell_exit_stderr_timeout/export.json`, `stdout.jsonl` | skipped: recorded data, judged through the test that reads them |
| `crates/candler-harness-opencode/tests/it/guard_opencode_driver.rs` | read |
| `crates/candler-schemas/schema/*.schema.json` (five files) | skipped: generated from `evaluation.rs` and `trace.rs`; the Rust source was read |
| `crates/candler-schemas/src/evaluation.rs`, `trace.rs` | read |
| `crates/candler-schemas/tests/fixtures/rpc/PROVENANCE.toml`, `README.md`, `invalid/needs_raw_not_boolean.json`, `valid/response_describe_needs_raw_manifest.json` | read (README), skim (rest) |
| `crates/candler-schemas/tests/it/guard_context_digest.rs`, `guard_trace_wire.rs` | read |
| `crates/candler-schemas/tests/it/guard_evaluator_protocol.rs`, `guard_rpc_wire.rs` | skipped: one-line version edits |
| `crates/candler-schemas/tests/vectors/field_classes.toml` | read |
| `crates/candler/tests/fixtures/plugins/graded-trace/kinds/graded_trace.py`, `plugin.toml` | read (py), skim (toml) |
| `crates/candler/tests/it/guard_cli_e2e.rs`, `guard_cli_e2e/graded_trace_e2e.rs` | read |
| `crates/candler/tests/it/guard_cli_e2e/i8_e2e.rs`, `parity_e2e.rs` | skipped: refactor of the parity rig into shared helpers; used through `graded_trace_e2e.rs` |
| `crates/candler/tests/it/guard_input_seam.rs`, `guard_registry.rs`, `guard_run_evaluate.rs`, `support/seam.rs` | read (first two), skipped (last two: helper plumbing) |
| `docs/contract/codes.toml`, `evaluator-helper.md`, `evaluator-protocol.md`, `index.md` | read |
| `python/candler-evaluator/CHANGELOG.md`, `README.md`, `pyproject.toml`, `scripts/gen_models.py` | read |
| `python/candler-evaluator/src/candler_evaluator/__init__.py`, `plugin.py`, `trace.py` | read |
| `python/candler-evaluator/src/candler_evaluator/_models/*.py` (six files) | skipped: generated; `EventKind4` and the `TraceRecord` union spot-read |
| `python/candler-evaluator/tests/test_guard_context.py` | read |
| `python/candler-evaluator/tests/test_guard_isolation.py` | skipped: two-line edit |
| `python/candler-evaluator/uv.lock` | skipped: lock file |

Not read: the Rust workspace beyond the diff, apart from `store/layout.rs`, `store/run.rs`, `runner/exec/mod.rs`, `import/assemble.rs` and `import/raw.rs` for the trace and raw-stream rules.
