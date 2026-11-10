# Import stream sources

**Goal**: Every imported run lists its `agent_stream.jsonl.gz` and `agent_stderr.txt` in `import.sources[]`, each kept as a blob, so the raw header's `source` digest names a blob the store holds and re-import refuses a changed stream.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone: `git merge-base --is-ancestor fix/v1-review-code-gaps HEAD`
- [ ] [nl] OWNED: `crates/candler-core/src/import/read\.rs`, `crates/candler-core/tests/guard_import_mini\.rs`, and `crates/candler-core/tests/support/legacy_import\.rs` only for a shared helper the new guards need
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_import_mini) | binary(guard_legacy_mapping) | binary(guard_oracle_mini)'` passes
- ⬜ [cmd] Sources: in a fresh import of the mini-corpus, every run's manifest `import.sources` names its run's `agent_stream.jsonl.gz` and, when the run has one, its `agent_stderr.txt`, by cell-relative path. Each digest is the file's sha256, and `Store::blob_path` (or the store's blob reader) finds a blob for it. The raw header's `source` digest of each run's stdout frame file is among them. Oracle: `sha256` of the fixture files, computed in the test from their bytes
- ⬜ [cmd] Re-import: the one-byte cases of `two_imports…`/`an_import_that_stopped…` (today at l.406) gain two cases, each refused `ImportError::Conflict` naming the run and the file, with the store byte-for-byte as it was:
  - `synthetic_cell/_run_cache/with_skill/run-2/agent_stream.jsonl.gz`, gunzipped, one byte flipped in a line after the first, and gzipped again;
  - `…/run-2/agent_stderr.txt`, one byte flipped.

  On `main` both cases are accepted.
- ⬜ [cmd] `an_imported_request_has_the_oracle_context_digest` passes unchanged: the context digests do not move
- ⬜ [cmd] STD, LINUX and SUBJ pass
- ⬜ [nl] The leaf report records the bite: with the two `continue`s restored, both new re-import cases and the sources guard fail
- ⬜ [nl] The Grit reviewer reports no open finding on the leaf's `.rs` changes
**References**: [R01 §Golden corpus](../../__design__/architecture.md) — "every source file read with its sha256, each kept as a blob"; [R03](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/conformance-docs-handover.md) — gap 2; [R06 §Core line](../../CONTRIBUTING.md) — breaking changes while `0.y.z`

## Step 1: The stream and stderr are import sources
**Goal**: The importer treats a run's stream and stderr as the sources they are, without changing what it frames or how it reads them.
**Implementation Logic**:
1. `import/read.rs::run_files`: the arms `(true, "agent_stream.jsonl.gz")` and `(true, STDERR_FILE)` still set `run.stream` and `run.stderr`. They no longer `continue`, so the `SourceFile` also reaches `run.sources.push(file)`. This needs a clone, annotated `// BORROW:`.
   - Nothing else changes: `import_sources` (`manifest.rs:548`) lists `run.sources` by cell-relative path, and `put_sources` (`assets.rs:61`) stores each as a blob.
   - `LegacyRun::stream` and `stderr` keep their meaning, and the framing (`streams.rs`, `assemble.rs`) is untouched.
2. Check that no source path collides (`add_source` refuses a duplicate) and that the walk sorts `sources` by path as before (`finish`).
3. Guards in `tests/guard_import_mini.rs`: the Sources and Re-import gates. Write the new cases into the existing one-byte loop. The gzip rewrite uses `flate2`, a dependency of candler-core that its integration tests can use; no manifest changes.
4. The commit body states the migration under `BREAKING CHANGE:`:

   > A store imported before this change lists no stream or stderr in `import.sources[]`, so re-importing its folder is refused with `Conflict`. Re-import the folder into a fresh store.

   Its subject carries `!`.
**Deliverables**: `crates/candler-core/src/import/read.rs`: `run_files` without the two `continue`s; `crates/candler-core/tests/guard_import_mini.rs`: the sources guard, and the two new one-byte cases, named in the leaf report
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_import_mini)'` (expected: PASS)
**Commit**: `fix(importer)!: list each run's stream and stderr as import sources`
