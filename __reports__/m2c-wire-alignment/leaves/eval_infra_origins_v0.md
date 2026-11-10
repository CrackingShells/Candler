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
