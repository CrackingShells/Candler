# run_context: leaf report (v0)

The implementer's report follows as it came back, after the adversarial verifier's first round and the fixes it led to. The rulings on its findings are in [l1/00-rulings_v0.md](../l1/00-rulings_v0.md) §run_context.

## Implementer's report

Branch `m2a/run_context`, worktree `/Users/hacker/Documents/tmp/claude-worktrees/Candler/m2a-run_context`. Clean, nothing pushed.

### 1. Commits
- 1253174 feat(evaluation): assemble run contexts from suite needs
- c8323df feat(evaluation): materialise contexts into private workspaces
- 8a46a1f test(evaluation): guard the context digest, file rule and isolation
- 4a1c49a fix(evaluation): refuse a workspace that contains the store
- 1b4480b test(evaluation): guard corrupt blobs, copy modes and store-containing workspaces

### 2. Gates
- SPAWN (clean src, `cargo clean -p candler-core`, guard_signal): PASS, "4 tests run: 4 passed".
- fmt: PASS. clippy workspace all-targets `-D warnings`: PASS. Linux-target clippy `--tests`: PASS. cargo doc `-D warnings`: PASS.
- workspace nextest: PASS, "275 tests run: 275 passed, 3 skipped". guard_run_context 6/6.
- RC1 vectors: PASS, `uv run --with rfc8785 python gen_live_context.py --check` prints "vectors OK: live_context_runs.toml". ruff clean.
- cog check merge-base..HEAD: PASS, no errored commits. OWNED: only context/*.rs, guard_run_context.rs, the two oracle files and public_surface.rs changed.
- SCAFFOLD: 6 `#[test]`, 6 `Guards:` lines, no scaffold marker.

### 3. Guards (oracle, bite seen failing)
- `the_live_context_digest_is_the_independent_vector` (RC1). Oracle: python rfc8785 vectors from the raw mini-corpus, plus a one-byte twin that differs. Bite: remove `builder.artifact(..)` in materialise.rs, 3 guards fail.
- `the_file_rule_decides_the_paths_and_the_digest` (RC2). Oracle: hand-listed path sets and a digest rebuilt from literal bytes. Bite: drop the dot-component clause in rule.rs, fails.
- `a_plug_in_cannot_write_through_to_the_store` (RC3, I11). Oracle: store-recorded sha256, blob bytes and mode. Bite: `fs::hard_link` instead of the copy, fails.
- `a_corrupt_blob_is_refused_and_not_digested` (new, verifier finding 1). Oracle: a blob overwritten here. Bite: take the digest from the store's recorded sha and drop the check, fails.
- `a_materialised_file_is_a_private_read_only_copy` (new, finding 2). Oracle: stat of copy and blob, executable bit set in a synthetic tree. Bite: modes 0600/0700, fails.
- `a_workspace_that_contains_the_store_is_refused` (new, finding 5). Oracle: parent of the store root and the root itself. Bite: disable the ancestor check, fails. Source fix is 4a1c49a.
Bites were restored with `git checkout -- <file>`, except bite C's first run (restored from a copy while the fix was uncommitted). It had left an `outputs/` in `$TMPDIR`; I removed it and made the guard use a private holder directory.

### 4. One-off records
- Adversarial verifier ran (agent a6f9ff3302fa1c9b8). It re-ran the three leaf bites, all caught, and two bites of its own, which survived until the guards above.
- STD log: `$TMPDIR/m2a-run_context-std.log`; final gate log: `$TMPDIR/m2a-run_context-final.log`.
- Earlier flaw, fixed: the first commit had included materialise.rs; amended before any later work.

### 5. Findings, spec defects, requests, deviations
- SPEC DEFECT: the leaf names `CheckedNeeds`, defined nowhere. `assemble` takes `candler_schemas::evaluation::Needs` and compiles the globs itself. The engine and suite-check leaves need to decide who owns a checked form.
- `needs.trace` is silently ignored. Non-empty snapshots or inputs return `ContextError::Unsupported` (spec says they are empty).
- A metadata path missing or without a recorded value gives `MissingMetadata`.
- `materialise(&Assembled, &Path)` plus `Workspace::prepare`. It refuses a relative workspace, one inside or above the store (symlinks resolved), and a pre-existing `outputs/`. A failed `materialise` leaves a partial `outputs/`; the engine must remove the workspace before a retry (verifier NIT).
- `AssembledArtifact` has public fields; `FileRule` and `file_rule_keeps(globs, path)` are exposed.
- The importer already drops `._*` and `.DS_Store` but not `.claude/`, so RC2 uses `RunWriter`-sealed synthetic runs.
- File rule (verifier MINOR 3, 4, left as is): a dot component is kept if any literal segment of the matching glob has the same text, wherever it sits (`outputs/.a/**` keeps `.a/.a/f`); and only plain literal segments count, so `outputs/\.env`, `outputs/[.]env` and braces across `/` drop the file. Both fail safe (drop, never leak). R03 should say which it means.
- The mini-corpus has no dot files under `outputs/`, so the Python vectors never exercise the dot rule; RC2's hand table does.
- Case or Unicode-normalisation collisions on APFS give a hard `Io` error, not an overwrite.
- `public_surface.rs` has many `#[test]`s without `Guards:` lines (pre-existing); my surface test has none either.
- Verifier verdict: no open BLOCKER or MAJOR after the guards above.

## Adversarial verifier, second round
The verifier checked `4a1c49a` and `1b4480b` and reported no open BLOCKER or MAJOR. Its MAJOR (finding 1, corrupt blobs) and MINORs 2 (copy modes) and 5 (a workspace above the store) are closed.

| Bite | Guard that failed |
|---|---|
| Digest taken from the store's recorded sha, with the byte check off | `a_corrupt_blob_is_refused_and_not_digested` |
| Copies left `0o600` | `a_materialised_file_is_a_private_read_only_copy` |
| The new `store.starts_with(&real)` refusal disabled | `a_workspace_that_contains_the_store_is_refused` |
| The `0o555` branch never applied | `a_materialised_file_is_a_private_read_only_copy` |

Each run failed exactly one guard, and the other five passed.

Still open, as the rulings decided:
- MINORs 3 and 4 on the dot rule are kept and written into R03.
- MINOR 6 (`Needs` for `CheckedNeeds`) is fixed in the leaf text.

Two new NITs on the test, both left as they are:
- The store-refusal test does not tell which refusal fired.
- A symlinked ancestor is covered by reading the code, not by a test.

## Landing (coordinator, rebased onto 12f76fc)
Rebased as `ea5c89c`, `db3daf9`, `5af51fc`, `a194a97`, `57d9cd5`. The implementer then amended the tip subject, which was 81 characters, to `test(evaluation): guard corrupt blobs, modes and store workspaces` (`a34795d`, 65), keeping its body and trailer.

| Gate | Result |
|---|---|
| OWNED | PASS against `12f76fc`. `public_surface.rs` changes only the `run_context_surface` block |
| SUBJ | `No errored commits`, longest subject 67 characters |
| SCAFFOLD | PASS: 6 `#[test]`, 6 `Guards:` lines, no scaffold marker |
| SPAWN | `guard_signal` 4/4, after `cargo clean -p candler-core`, uid 501 |
| STD | `285 tests run: 285 passed, 4 skipped`, with fmt, clippy and doc clean |
| Linux clippy | PASS |
| `guard_run_context` | 6/6 |
| RC1 vectors (`gen_live_context.py --check crates/candler-core/tests/fixtures/mini-corpus`) | `vectors OK: live_context_runs.toml`, and ruff clean |

The leaf landed as `7bc318f` `merge(evaluation): land run_context`.
