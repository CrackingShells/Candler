# Leaf report: review fixes (rows 2, 3, 4, 6, 7)

Branch `leaf/run-context/review_fixes`, from `feat/run-context`.

| Row | Result | Commit |
|---|---|---|
| 2 | Changed | `fix(evaluation): read a run's raw/ once in the store and refuse strays` |
| 3 | Changed, guarded | same commit |
| 4 | Changed, guarded | `fix(evaluator-helper): refuse a trace out of order like Candler's reader` |
| 6 | Changed (no defect found) | `refactor(evaluation): build a served copy's context entry in one place` |
| 7 | Changed | `refactor(adapter-opencode): spell the export reader's version once` |

## #2 One spelling of the stored names
`store/layout.rs` now holds `MANIFEST_FILE`, `TRACE_FILE`, `RAW_DIR`, `RAW_SUFFIX`, `raw_file_name` and `stream_of_raw_file`; `StoreLayout::{manifest, trace, raw_dir, raw}` use them. `context/assemble.rs` takes its workspace names from them, the frame writer (`frames/mod.rs`) builds `<stream>.jsonl.gz` with `raw_file_name`, and the runner's `stream_of` (`runner/exec/mod.rs`) calls `stream_of_raw_file`. The workspace names deliberately equal the store names, so an evaluator finds each file where a run directory holds it; a doc comment on `stored_file` says so. `import/` and `jobs/orchestrate/controls.rs` still spell `manifest.json` for their own purposes (an import file name and a path join); not in scope of the row.

## #3 `raw/` has one reader, and it refuses
No store reader of `raw/` names existed (`run.rs` only writes). New `Store::raw_streams` is the one reader: it lists the visible entries of `raw/` and returns `StoreError::Parse` for an entry that is not `<stream>.jsonl.gz` with a valid stream name. `assemble` calls it, so the refusal reaches the caller as `ContextError::Store`, code `candler.internal` (an existing code: "A stored file is not a valid record"; files of the store are Candler's own). No new code. A missing `raw/` still gives no streams, so `PartMissing` is unchanged.
Not covered: hidden entries (a leading `.`, as the writer's `.tmp-` siblings) and `*.partial` names are skipped by the store's existing `visible_names` rule; a stray with such a name is not refused.

Guard: `a_stray_entry_in_raw_refuses_the_context` (`tests/it/guard_run_context.rs`). Oracle: hand-written entry names (`notes.txt`, `agent_stream.jsonl`, `Bad Name.jsonl.gz`) and the code `candler.internal`, written into a mini-corpus import's `raw/`; the clean run assembles first. Bite: with `raw_streams` skipping instead of refusing (`filter_map`), it failed at `a stray entry is refused: Assembled { ... }` (`expect_err` on an `Ok`). Seen red, then green.

## #4 `read_trace` order
Rules matched from `TraceFile::read`: empty file; a session or event before the header; a second header; a session after an event; a duplicate session. Not added: the rules of `TraceFile::new` that relate records (`seq` gaps, undeclared session, child before parent, `raw_ref` stream not listed); Rust has them, but the task named five and the helper's existing fixtures hold a `seq` 1 event with no `seq` 0. The docstring, README, CHANGELOG and the page note say these still pass.
The helper requires a code on every raise (`test_d4_*`), so a new class `TraceOrderError(ValueError)` (exported) carries the new code `helper.trace_out_of_order` (side `plugin`); no existing `[helper]` code fits. Row added to `codes.toml`, page regenerated with the bless command. The page note was reworded to "`read_trace` refuses a trace out of order and `null` on an optional field." (page stays within 120 words). CHANGELOG entry in Unreleased updated; version not bumped.

Guard: `test_the_reader_refuses_a_file_out_of_order` (`tests/test_guard_context.py`). Oracle: the rules of `TraceFile::read` applied by hand to lines written here; each case's message names the line (`line 2: a second header`, `line 4: session s2 after an event`, `line 3: session s1 declared twice`, `line 1: session before the header`, `line 1: event before the header`, `an empty trace file`) and the code. Bite: with the checks disabled it failed `DID NOT RAISE TraceOrderError` (first case, the empty file). Seen red, then green.

## #6 `materialise`
The three repeated `ContextArtifact::new(.., text_of(&copy)?, digest)` arms now call one `artifact_of(path, copy, digest)`; call sites read one line each. The `Input` arm drops the digest by design, not by defect: every assembled file, input files included, goes through `copy_stored`, which hashes the copy against `file.sha256` and refuses with `CorruptBlob`; the wire for an input carries the directory of its tree and the digest of its list, no per-file digest. A comment on the arm says so. No fix needed, so no new guard (`a_corrupt_blob_is_refused_and_not_digested` and the inputs guards already own it). The `Served` tag stays mirrored; reducing it did not fall out of the change.

## #7 OpenCode reader version
A private macro `export_reader!(0, 2, 0)` in `candler-harness-opencode/src/lib.rs` defines `EXPORT_READER` (`"opencode-export@0.2.0"`) and `EXPORT_READER_VERSION` from one spelling. Values written to stores are unchanged (the OpenCode suite passes). Claude Code crate untouched. `LIVE_READER` is left as it was (not in the task).

## Checks
See the final message of the leaf; the full run is on the branch tip.
