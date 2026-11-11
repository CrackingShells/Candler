# Type merges

**Goal**: The core frames streams with `candler-harness`'s `Framing`, read from the reader rather than hardcoded, and `Session`'s parent field is named without the stutter while its wire name stays `parent_session`.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] OWNED: `crates/candler-schemas/src/trace.rs`; `crates/candler-core/src/frames/**`, `crates/candler-core/src/supervise/{output,spawn}.rs`, `crates/candler-core/src/import/streams.rs`, the framing lines of `crates/candler-core/src/runner/exec/mod.rs`; `crates/candler-harness/src/spec.rs` (the `Framing` lint reason only); `crates/candler-core/tests/{public_surface,guard_frames,guard_seams,guard_supervise}.rs`
**Success Gates**:
- ⬜ [cmd] `FrameMode` is gone: `! git grep -n 'FrameMode' -- ':(glob)crates/**'`
- ⬜ [cmd] The runner reads the reader's framing: `git grep -q -E '\.framing\(\)' -- crates/candler-core/src/runner`
- ⬜ [cmd] The frame and supervision guards pass: `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_frames)|binary(guard_seams)|binary(guard_supervise)|binary(public_surface)'`
- ⬜ [cmd] The schemas commit leaves every schema file and the accessor as they were: `git diff --quiet a26fe53 -- crates/candler-schemas/schema && git grep -q 'pub fn parent_session' -- crates/candler-schemas/src/trace.rs`, and DRIFT holds
- ⬜ [cmd] The one schemas commit touches only the schemas crate: `git log --no-merges --format=%H a26fe53..HEAD -- crates/candler-schemas | while read c; do git show --name-only --format= $c | grep -v '^crates/candler-schemas/' | grep -q . && echo $c; done` prints nothing
- ⬜ [cmd] SPAWN, STD (`-p candler-schemas -p candler-core -p candler-harness`), LINUX, DIGEST, GOLDEN and SUBJ pass
**References**: [R01 §Raw traces, §Normalised trace model](../../__design__/architecture.md) — framing and `parent_session`; [R09 D12](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-approved-plan.md); [R08 §Schema line](../../CONTRIBUTING.md) — one crate per release-triggering schemas commit

## Step 1: Rename the session's parent field
**Goal**: `Session`'s private field is `parent`, so its `#[expect(clippy::struct_field_names)]` goes, and nothing a reader or a schema sees changes.
**Implementation Logic**:
In `trace.rs`, rename the private field of `Session` and of `SessionBuilder` to `parent`, with `#[serde(rename = "parent_session")]` where the field is serialised or deserialised, and remove the `#[expect]` in the same commit (an unfulfilled expectation fails). Keep the public accessor `parent_session()` and the builder setter's name. Run DRIFT: no schema file may change.
**Deliverables**: `crates/candler-schemas/src/trace.rs`: `Session::parent`, `SessionBuilder::parent`
**Consistency Checks**: `cargo nextest run -p candler-schemas && git diff --quiet a26fe53 -- crates/candler-schemas/schema` (expected: PASS)
**Commit**: `refactor(schemas): name the session's parent field without the stutter`

## Step 2: Frame with the reader's framing
**Goal**: One framing type in the workspace, and a Document-shaped reader gets its stream framed whole.
**Implementation Logic**:
1. Replace every use of `candler_core::frames::FrameMode` with `candler_harness::spec::Framing` (`Whole` becomes `Document`), in `frames`, `supervise/{output,spawn}.rs`, `import/streams.rs` and the tests. Remove `FrameMode` and its `From<Framing>`.
2. In `runner/exec/mod.rs`, frame stdout with the live reader's `framing()` when the driver returns a reader, and with `Lines` otherwise; stderr stays `Lines`. The Claude Code reader is `Lines`, so its runs frame exactly as before.
3. Reword the two lint reasons that named each other.
4. The commit is breaking for the core's Rust API; its body names the migration: use `candler_harness::spec::Framing`.
**Deliverables**: `crates/candler-core/src/frames/mod.rs` (no `FrameMode`); `crates/candler-core/src/supervise/{output,spawn}.rs`; `crates/candler-core/src/import/streams.rs`; `crates/candler-core/src/runner/exec/mod.rs`; the four test files
**Consistency Checks**: `! git grep -n 'FrameMode' -- ':(glob)crates/**' && cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_frames)|binary(guard_seams)|binary(guard_supervise)|binary(public_surface)'` (expected: PASS)
**Commit**: `refactor(runner)!: frame streams with the reader's own framing type`
