# Seams

**Goal**: One fake harness run crosses every M1 seam — supervisor, framed raw, `RunWriter`, the trace reader, `OutcomeWriter`, `rebuild_index` — and the joined result is replayable (I3), leaks no secret (I19), and sits at the store paths R01 names.
**Pre-conditions**:
- [ ] [cmd] DEP(supervisor), DEP(run_store), DEP(claude_code_reader) and DEP(harness_traits) hold: `for l in supervisor run_store claude_code_reader harness_traits; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`
- [ ] [cmd] `trace::normalise` (legacy_import Step 2) is on this leaf's base: `git log --format=%s | grep -qx 'feat(importer): frame legacy streams and normalise their traces'`
- [ ] [cmd] `ls crates/candler-core/tests/fixtures/mini-corpus/*/_run_cache/with_skill/run-*/agent_stream.jsonl.gz && test -f crates/candler-core/tests/oracle/vectors/cell_digest.toml`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
**Success Gates**:
- ✅ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_seams)'` passes, holding three B tests over one pipeline run into a `tempfile` store:
  - **replay** (I3): the stored `raw/stdout.jsonl.gz`, normalised again through `trace::normalise` with a fresh `ClaudeCodeReader` whose `id()` equals the trace header's `reader`, gives a trace file byte-equal to the stored one; the frame count equals the fixture stream's line count by `lines()`;
  - **secret canary** (I19): a canary value passed in the fake harness's `.env` is found by `assert_clean` nowhere under the store, the job directory, the event log or `record_dir`, and not in the `Debug` of the request;
  - **store keys**: the run's manifest carries the `comparable` of the first entry of `cell_digest.toml`; the run directory, outcome file and index lines sit at the literal paths the test writes from R01 §Run store and that entry's digest, and `RunKey`'s `Display` equals the literal directory name
- ✅ [cmd] `! git grep -n -E 'event-envelope|jsonschema' -- crates/candler-core/tests/guard_seams.rs` (the payload-schema check lives once, in `guard_supervise`)
- ✅ [cmd] STD and SUBJ pass
- ✅ [nl] The leaf report has a bite mutation per B test, and names for each test which two leaves' seam it crosses
**References**: [R01 §Run store, §Raw traces, §Agent environment and secrets, §Invariants (I3, I19)](../../../../../__design__/architecture.md) — the rules joined here; [R04](../../../../../__reports__/m1-contracts/00-test_definition_v0.md) — the `seams` row and new assertions 2 and 3

## Step 1: Guard the seams from spawn to the store index
**Goal**: The three cross-leaf guards exist and pass on one run driven through the real supervisor and store.
**Implementation Logic**:
1. The fake harness is `tests/fixtures/seams/fake_harness.sh`, a POSIX shell that writes one mini-corpus stream (path given as argument, `gunzip -c`) to stdout, one line to stderr, and exits 0; it never prints its environment.
2. `tests/guard_seams.rs`: a test-side `EventSink` writes envelopes to `<store>/jobs/<job id>/events.jsonl`; `Supervisor::spawn` with `stdout`/`stderr` `Framed` into a temporary directory, the canary in `.env`, `record_dir` under the job directory; `wait`; `RunWriter::create` with a manifest around that `comparable`, `raw` for both streams from `Exit.raw`, `trace` from `trace::normalise` with a `ClaudeCodeReader`, `done`; one `OutcomeWriter` with two hand-written records, `commit`; `rebuild_index`.
3. The three B tests of the Success Gates read the result only through `Store` readers, `FrameReader` and a test-side directory walk. Expected paths and names are literals; no expected value comes from `StoreLayout`.
**Deliverables**: `crates/candler-core/tests/fixtures/seams/fake_harness.sh`; `crates/candler-core/tests/guard_seams.rs`
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_seams)'` (expected: PASS)
**Commit**: `test(runner): guard the seams from spawn to the store index`
