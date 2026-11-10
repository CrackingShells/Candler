# Digest invariance

**Goal**: A report shows, by measurement, that the two L0 fixes move no run key, cell digest or outcome context digest of the golden and mini-corpus imports. Only `import.sources[]` changes, by the stream and stderr entries, and only that is expected.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree branches off the milestone after L0: `git log --first-parent --format=%s HEAD | grep -qx 'merge(importer): land the legacy streams as import sources'`
- [ ] [cmd] `$GOLDEN` is in place: `test -d /Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`
- [ ] [nl] OWNED: `__reports__/v1-review-code-gaps/digest_invariance_v0.md` only, plus scratch under `$TMPDIR`. No code changes
**Success Gates**:
- ⬜ [cmd] `CANDLER_GOLDEN_CORPUS="$GOLDEN" cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_golden)'` passes on the leaf's branch (240 runs, 10,320 records)
- ⬜ [cmd] The same corpus imported with `candler import` built from `origin/main` and from the leaf's branch, into two fresh stores under `$TMPDIR`, gives:
  - equal sorted lists of run keys;
  - equal sorted lists of `(run key, context digest)` over every outcome record;
  - per run, manifests that are equal once `import.sources[]` entries ending in `agent_stream.jsonl.gz` or `agent_stderr.txt` are dropped from the branch side.

  The same three hold for the mini-corpus. Each comparison is a `diff` of two `jq`-sorted files, and each prints nothing.
- ⬜ [cmd] Every stdout raw header's `source` digest in the branch store has a blob file in that store, and in the `main` store it does not (the gap, measured)
- ⬜ [nl] The report gives, for each corpus: the counts compared, each `diff` command and its empty output, the headline numbers (runs, records, cells), and the sha of both builds. It is written with `writing-reports` (type: findings), with no code dump
- ⬜ [cmd] The scratch stores and the `main` build's worktree are removed when the report is written: `test ! -e "$TMPDIR/digest-main" && test ! -e "$TMPDIR/digest-branch"`
**References**: [Campaign §Gotchas](../README.md) — Digests, `$GOLDEN`; [M1 campaign](../../m1-contracts/README.md) — the golden gates and their numbers; [R01 §Golden corpus](../../../__design__/architecture.md)

## Step 1: Measure and record
**Goal**: The digest claim is measured on both corpora and written down.
**Implementation Logic**:
1. Build `candler` from `origin/main` in a temporary worktree of its own (`git worktree add "$TMPDIR/digest-main-src" origin/main`, with the campaign's sccache settings), and from the leaf's branch.
2. For each build and each corpus (`$GOLDEN`, `crates/candler-core/tests/fixtures/mini-corpus`), `candler import` into a fresh store (`$TMPDIR/digest-<side>/<corpus>`). Read the CLI's own `--help` for its arguments.
3. Extract with `jq` from each store:
   - the run keys (the run directories, or the index of runs);
   - every outcome record's `(run key, context_digest)`;
   - each manifest with the stream and stderr entries of `import.sources[]` dropped.

   Sort and `diff` the two sides.
4. Check the raw headers' `source` digests against each store's blobs.
5. Run the golden guard on the branch.
6. Write the report, then remove the scratch stores, the `main` worktree (`git worktree remove`) and its `target/`.
**Deliverables**: `__reports__/v1-review-code-gaps/digest_invariance_v0.md`
**Consistency Checks**: `test -s __reports__/v1-review-code-gaps/digest_invariance_v0.md && ! git worktree list | grep -q digest-main-src` (expected: PASS)
**Commit**: `reports(importer): record that the code gaps move no cell digest`
