# Leaf report: eval_infra_origins

## P1

**Question.** What does `.cargo_vcs_info.json` look like in a `.crate` that `cargo package` produced, on a clean tree and on a dirty one?

**Tool.** cargo 1.99.0 (5f94df478 2026-08-27), run in a throwaway `git clone --local` of this branch under `$TMPDIR`. Nothing was written into the worktree. The scratch copy is removed.

**What could not run as the leaf words it.** `cargo package -p candler` refuses: the four workspace path dependencies have no `version`, and `candler` has `publish = false`. After the scratch copy gave the path dependencies `version = "0.5.0"`, `candler` still cannot be packaged alone, because its siblings are not on crates.io (`no matching package named candler-core`), and `cargo package --workspace` failed the same way on the second crate. Only `candler-schemas`, which has no workspace dependency, packages. The file below is the real output for `candler-schemas`. The file's shape does not depend on the crate. Only `path_in_vcs` does, and for `candler` it would read `crates/candler`. This is a finding: nothing here packages `candler`, so a crates.io build of the binary is not possible today.

| Tree | `.cargo_vcs_info.json` |
|:--|:--|
| clean (committed) | `{"git": {"sha1": "<40 hex>"}, "path_in_vcs": "crates/candler-schemas"}` |
| dirty (tracked file modified, `--allow-dirty`) | `{"git": {"sha1": "<40 hex>", "dirty": true}, "path_in_vcs": "crates/candler-schemas"}` |
| untracked file outside the package, `--allow-dirty` | same as clean |

The files are pretty-printed with two-space indentation and no trailing newline. The sha1 is the full 40-hex `HEAD`.

**Answer.** Cargo writes `git.dirty` only when the tree is dirty, so on a clean tree the field is absent, not `false`. An absent `dirty` is therefore a clean tree, provided `git.sha1` is present. The file sits beside `Cargo.toml` in the package root. Dirtiness is judged over the package's own directory (an untracked file elsewhere in the repository did not set it).

**Decision.** The packaged origin reads `git.sha1` as the commit, and `git.dirty` as `true` when present and `false` when absent. A vcs file with no `git.sha1`, or one that does not parse, gives neither field.

## Commits

| Sha | Subject |
|:--|:--|
| e1b088b | `reports(manifest): record the shape of a packaged crate's vcs info` |
| 703d2a5 | `fix(manifest): record a build's identity when it is not a git checkout` |
| (this report) | `reports(manifest): record the eval_infra_origins leaf` |

## Resolver

`build_infra::resolve(dir, version, env) -> Identity {origin, tag, commits_ahead, commit, dirty}`, with `Origin` one of `Git`, `Packaged`, `Packager`, `None`. `build.rs` and `tests/guard_eval_infra.rs` both include `build_infra.rs` with `#[path]`. `supervise.rs` and `Cargo.lock` are untouched. The vcs file is parsed by hand (no build-dependency).

## Gates

| Gate | Result | Last line |
|:--|:--|:--|
| E1, `binary(guard_eval_infra)` | PASS | 10 tests run: 10 passed |
| `Cargo.lock` unchanged | PASS | `git diff --quiet feat/m2c-wire-alignment -- Cargo.lock` exits 0 |
| build records its commit in full | PASS | `cargo build -p candler`, then `test(/this_checkout/)`: 1 passed |
| SPAWN | PASS | `guard_signal` 9 passed |
| STD, `-p candler` form | PASS | fmt clean, clippy `-D warnings` clean, nextest 24 passed 5 skipped, doc `-D warnings` clean |
| LINUX | PASS | workspace clippy on x86_64-unknown-linux-gnu, finished |
| SUBJ | PASS | see the checks run before this report's commit |
| OWNED | PASS | the diff lists this report, `build.rs`, `build_infra.rs`, `guard_eval_infra.rs` |
| DRIFT, G5a | not applicable | no schemas crate and no test vector touched |

## Guards

Every guard was red first, against a stub resolver that returned nothing (9 of 9 failed), then green. Each carries `Guards:` and `Oracle:` lines in its doc.

| Guard | Oracle | Bite and the failing assertion seen |
|:--|:--|:--|
| `git_checkout_clean_records_the_full_commit` | `git rev-parse HEAD`; an edit to an unwatched file stays clean | short commit: `left: None, right: Git` (the 40-hex filter rejects it) |
| `git_checkout_dirty_records_dirty` | the test's own edit, `git rev-parse HEAD` | same bite, red |
| `git_checkout_records_the_nearest_tag_and_distance` | hand-made history | same bite, red (`right: Some("v0.1.0")`) |
| `git_checkout_that_cannot_tell_leaves_dirty_unrecorded` | git's own failure, asserted first | a failed status counted as clean: `left: Some(false), right: None` |
| `packaged_clean_records_sha_and_clean` | P1's clean file bytes | no direct bite beyond the shared ones |
| `packaged_dirty_records_dirty` | P1's dirty file bytes | the vcs `dirty` ignored (recorded `false`): `left: Some(false), right: Some(true)` |
| `packaged_inside_a_checkout_records_the_vcs_file` | the surrounding repository's HEAD differs from the sha | git consulted first for a packaged crate: `left: Git, right: Packaged` |
| `packager_variables_are_recorded_and_unset_dirty_stays_unrecorded` | hand-written variables | an unset `dirty` recorded as `false`: `left: Some(false), right: None` |
| `no_origin_records_the_tag_only` | hand-written; a vcs file with no sha or not JSON is no origin | none beyond the shared ones |
| `this_checkout_records_the_full_commit` | `git rev-parse HEAD`, compared with the value `build.rs` exported into this test binary | short commit: `left: Some("e1b088b...")` vs the 40-hex head, red |

The leaf names two bites: both were injected, seen red and restored (the two rows above that name them). Every bite was restored; `build_infra.rs` was diffed against the good copy afterwards.

## Findings

1. **Spec defect, P1 as worded cannot run.** `cargo package -p candler` fails (path dependencies without a version) and `candler` has `publish = false`. Even with versions added in the scratch copy, the siblings are not on crates.io, so neither `-p candler` nor `--workspace` packages it. P1 used `candler-schemas`. This also means the packaged origin cannot occur for the binary today. It is prepared, not exercised end to end.
2. **Ordering of origins.** The leaf's order is git, packaged, packager. The E1 line says "the packager variables set: they win". I followed the step's order: the packager's variables apply only when neither a checkout nor a vcs file gives an identity. If the packager must override a packaged crate, it is a one-line change in `resolve`, and the guard row for it does not exist yet.
3. **Deviation, vcs file beats a surrounding checkout.** A directory that holds `.cargo_vcs_info.json` is treated as packaged and git is not consulted, so a crate unpacked under someone else's repository does not record that repository's commit. This goes beyond the leaf and is guarded.
4. **Behaviour change in the git origin.** The commit is now always the full `git rev-parse HEAD`, also when a tag is found (it was `git describe`'s abbreviated hash). A checkout with no commit yet records nothing from git. A failed `git status` now leaves `dirty` unrecorded; before, it was recorded `false` because the old fallback looked at `rev-parse --git-dir`. With a checkout whose status fails, the verdict will now refuse instead of passing.
5. **Packager tag fallback.** When the packager sets any of the three names, a missing tag falls back to `v<version>`. A missing commit or flag stays unrecorded. A flag that is not `true` or `false` is unrecorded.
6. **WATCHED gains `build_infra.rs`**, so a change to the resolver reruns the build and counts as dirty. The build now also watches `.cargo_vcs_info.json` and the three variables (`rerun-if-env-changed`).
7. **No request** for `supervise.rs`: it reads the same names and omits `COMMITS_AHEAD` for non-git origins as before. The build warning for the none origin is emitted by `build.rs` and is not asserted by a guard.

## Fix round 2

Branch `m2c/eval_infra_hardening`, cut from the milestone tip dab0335. The commit is 970df17, `fix(manifest): record only the build identity the build can prove`.

### Changes
- **Output names.** `build.rs` now exports under `CANDLER_BUILT_{TAG,COMMITS_AHEAD,COMMIT,DIRTY}`, always all four, empty when unrecorded. The packager's `CANDLER_EVAL_INFRA_*` stay inputs only. I chose a distinct prefix because a shared name lets a value set as input be read as output: with the old names, `option_env!` saw the builder's shell whenever the build skipped a field. The distinct prefix removes that class, and the empty export covers a stale `CANDLER_BUILT_*` in the shell, which `rustc-env` now overrides.
- **Read side.** `build_infra::read_recorded` (shared by `#[path]` with `build.rs`, the guard and `supervise.rs`) treats empty as unrecorded: `dirty` is exactly `true` or `false`, `commits_ahead` only digits, `commit` 40 lowercase hex. `supervise.rs::eval_infra()` only reads the four literals and calls it.
- **Packager commit** must be 40 lowercase hex, else unrecorded.
- **Doc** of `EvalInfra::commit` now says the full 40-hex commit.

### Gates
| Gate | Result | Last line |
|:--|:--|:--|
| E1 | PASS | 15 tests run: 15 passed |
| `this_checkout` after `cargo build -p candler` | PASS | 1 test run: 1 passed |
| SPAWN | PASS | `guard_signal` 9 passed |
| STD `-p candler -p candler-core` | PASS | fmt, clippy `-D warnings`, doc `-D warnings` clean; nextest 180 passed, 17 skipped |
| LINUX | PASS | workspace clippy on x86_64-unknown-linux-gnu finished |
| SUBJ (base dab0335) | PASS | `cog check` and the 72-character check, run before this commit |
| `Cargo.lock` | PASS | unchanged against dab0335 |
| OWNED | PASS | provenance.rs, `build.rs`, `build_infra.rs`, `supervise.rs`, `guard_eval_infra.rs` and this report |

### Guards and bites
The new guards were written together with the code, so their red was seen through the bites below, each injected and restored.

| Guard | Oracle | Bite and the assertion seen |
|:--|:--|:--|
| `ambient_variables_never_stand_in_for_an_unknown_field` | hand-written: the shell holds `false`, a commit, a tag and a distance under the input and output names; the build proves none | emit skipped for unknown fields: `left: Recorded { tag: Some("v9.9.9"), ..., dirty: Some(false) }`, right all `None` |
| `packager_commit_that_is_not_a_full_commit_is_unrecorded` | hand-written spellings: `abc123`, blank, empty, upper case, 39 hex | any non-empty commit accepted: `left: Some("abc123"), right: None` |
| `flag_text_other_than_true_or_false_is_unrecorded` | hand-written: `yes`, empty, `TRUE`, `1`, ` true`; distances `-1`, `+3`, `3.5` | anything but `true` read as `false`: `left: Some(false), right: None` (three guards red) |
| `a_proven_identity_is_read_back_as_exported` | hand-written values | round trip, no separate bite |
| `the_binary_reads_the_names_the_build_exports` | the literals of `supervise.rs`, read as text | catches a name drift between `EXPORTED_VARS` and the binary |

The ambient guard goes through the resolver, `exported()` and `read_recorded` with no cargo build: a map stands for the compiler's environment, the shell's values first, then the emitted ones overlaid. The real `build.rs` loops over the same `exported()`, and `supervise.rs` calls the same reader. The one thing the guard does not run is rustc's own precedence of `rustc-env` over the shell, which is cargo's documented behaviour. `this_checkout` shows the real build exporting a commit and a flag.

### Findings
1. `supervise.rs` includes `build_infra.rs` with `#[path]` and one `#[expect(dead_code)]`, because the resolver half is unused in the binary. `build.rs` carries the same attribute for the reader half.
2. `EvalInfra::tag` is read as any non-empty text. The leaf asked no stricter rule.
3. `Identity::exported` returns a `Vec`, not an array, only so that a bite can filter it.
