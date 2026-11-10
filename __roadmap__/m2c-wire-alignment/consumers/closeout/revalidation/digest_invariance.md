# Digest invariance

**Goal**: A report shows, by measurement, that M2c moves no run key, cell digest, outcome context digest or manifest of the golden and mini-corpus imports between `main` (b017d31) and the milestone branch. An adversarial verifier then attacks that claim, and the source, `ENTRY_PARAMS` and provenance moves behind it.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree branches off the milestone after L2: `git log --first-parent --format=%s feat/m2c-wire-alignment | grep -qE '^merge\(.*\): land layering_guard$'`
- [ ] [cmd] The golden corpus is in place: `test -d /Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`
- [ ] [nl] OWNED: `__reports__/m2c-wire-alignment/digest_invariance_v1.md` only, plus scratch under `$TMPDIR`. No code
**Success Gates**:
- ⬜ [cmd] GOLDEN passes on the leaf's branch
- ⬜ [cmd] Each corpus is imported with `candler import` built from b017d31 and from the branch, into fresh stores under `$TMPDIR`, with each side's rules file. The branch's rules file adds only `entry_params`. Then, per corpus:
  - the sorted run keys are equal;
  - the sorted `(run key, context_digest)` pairs over every outcome record are equal;
  - the manifests, as `jq -S -c`, one line per run, are equal.
  Each is a `diff` of two sorted files that prints nothing
- ⬜ [cmd] The parity fixtures are untouched: G5a and G5b hold
- ⬜ [nl] The report gives, for each corpus: the counts (240 runs and 10,320 records; 6 runs and 136 records), each `diff` command and its empty output, the sha of both builds, and the rules files used (with their sha256). It is written with `writing-reports` (type findings), with no code dump
- ⬜ [cmd] The scratch stores and the b017d31 worktree are removed: `test ! -e "$TMPDIR/m2c-digest-main" && test ! -e "$TMPDIR/m2c-digest-branch" && ! git worktree list | grep -q m2c-digest-main-src`
- ⬜ [nl] The `digest_invariance` verifier closes with no open finding. It assumes the measurement is wrong and tries to show it:
  - a comparison that could not fail, such as an empty side or a filter that drops a field;
  - a manifest field outside the comparison;
  - the `entry_params` move changing a `suite_entry_digest`;
  - a source-only change moving a live cell digest (it re-runs T1);
  - a live run's provenance or `evaluation_inputs` leaking into an imported manifest.
**References**: [R08 digest_invariance_v0](../../../../../__reports__/v1-review-code-gaps/digest_invariance_v0.md) — the method and the baseline counts; [the v1-review digest leaf](../../../../v1-review-code-gaps/settle/digest_invariance.md) — the same steps; [R10 P5](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md)

## Step 1: Measure and record
**Goal**: The digest claim is measured on both corpora and written down.
**Implementation Logic**:
1. Build `candler` (release) from b017d31 in its own temporary worktree (`git worktree add "$TMPDIR/m2c-digest-main-src" b017d31`) and from the branch, with the campaign's build settings.
2. Import each corpus (`$GOLDEN`, `crates/candler-core/tests/fixtures/mini-corpus`) with each build into fresh stores `$TMPDIR/m2c-digest-<side>/<corpus>`. Use b017d31's rules file on its side and the branch's on the other.
3. Extract and `diff` as in the gates. A difference is a finding, reported with its first lines, never smoothed over.
4. Run GOLDEN on the branch. Write the report, then remove the scratch stores, the worktree and its build.
**Deliverables**: `__reports__/m2c-wire-alignment/digest_invariance_v1.md`
**Consistency Checks**: `test -s __reports__/m2c-wire-alignment/digest_invariance_v1.md && ! git worktree list | grep -q m2c-digest-main-src` (expected: PASS)
**Commit**: `reports(experiment): record that m2c moves no imported digest`
