# Eval infra origins

**Goal**: The binary records what its build's origin proves about its identity. A git checkout gives the full commit and `dirty`. A packaged crate (crates.io or `cargo package`) gives `.cargo_vcs_info.json`'s commit and `dirty`. Packager-set `CANDLER_EVAL_INFRA_*` values come next. Otherwise only `tag = v<package version>` is recorded, and `commit` and `dirty` are left unrecorded, so the verdict refuses and names why (review E4).
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The leaf branches off the milestone: `git merge-base --is-ancestor feat/m2c-wire-alignment HEAD`
- [ ] [nl] OWNED: `crates/candler/build.rs`, `crates/candler/build_infra.rs` (new), `crates/candler/tests/guard_eval_infra.rs` (new), and `crates/candler/Cargo.toml` only if `build.rs` needs a build-dependency the workspace already locks. `Cargo.lock` must not change. `supervise.rs::eval_infra()` reads the same `CANDLER_EVAL_INFRA_*` names as today, so it stays untouched
**Success Gates**:
- ⬜ [cmd] E1: `cargo nextest run -p candler --no-tests=fail -E 'binary(guard_eval_infra)'`. It is parameterized over:
  - a git checkout, clean and dirty: the full 40-hex commit and `dirty`;
  - a directory without `.git` that holds `.cargo_vcs_info.json`, clean and with `"dirty": true`: its `git.sha1` and `dirty`, with `tag = v<version>` and `commits_ahead` unrecorded;
  - the packager variables set: they win;
  - neither: `tag` only.
  Oracle: temp directories built by the test, `git rev-parse` for the checkout, and P1's real `cargo package` output for the vcs file's shape
- ⬜ [cmd] `Cargo.lock` is unchanged: `git diff --quiet feat/m2c-wire-alignment -- Cargo.lock`
- ⬜ [cmd] A build in this checkout still records its commit, now in full: `cargo build -p candler && cargo nextest run -p candler --no-tests=fail -E 'binary(guard_eval_infra) & test(/this_checkout/)'`
- ⬜ [cmd] STD (`-p candler`), LINUX and SUBJ pass
- ⬜ [nl] The leaf report records P1's answer (the file's exact shape, clean and dirty) and E1's bites (an unknown `dirty` recorded as `false`; the vcs file's `dirty` ignored)
**References**: [R05 E4](../../__reports__/candler-v1-review/00-findings_v0.md); [R10 D7, guard E1, probe P1](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md); `crates/candler/build.rs` as it is; [CONTRIBUTING §Versioning](../../CONTRIBUTING.md) — nothing goes to crates.io today, which this leaf prepares for

## Step 1: Probe P1, the shape of a packaged crate's vcs file
**Goal**: Know `.cargo_vcs_info.json`'s exact shape from Cargo itself, clean and dirty, before writing its parser.
**Implementation Logic**:
1. In a scratch copy outside the repo, run `cargo package -p candler --no-verify --allow-dirty` against a clean tree and against a dirty one. Read `.cargo_vcs_info.json` from each `.crate` (`tar -xzf`).
2. Record both files' content and the Cargo version in the leaf report. Note whether `dirty` is absent or `false` on a clean tree.
3. Commit nothing from the scratch copy, and remove it.
**Deliverables**: `__reports__/m2c-wire-alignment/leaves/eval_infra_origins_v0.md`: its `## P1` section
**Consistency Checks**: `grep -q '^## P1' __reports__/m2c-wire-alignment/leaves/eval_infra_origins_v0.md` (expected: PASS)
**Commit**: `reports(manifest): record the shape of a packaged crate's vcs info`

## Step 2: The build identity by origin
**Goal**: `build.rs` resolves the identity through one function that a test can call. It tries the four origins in order and exports the same `CANDLER_EVAL_INFRA_*` names.
**Implementation Logic**:
1. Write E1 first and see it red.
2. Move the logic of `build.rs` into `build_infra.rs`: a function from a crate directory, the package version and an environment lookup to the four optional fields. `build.rs` includes it with `#[path]`, and so does the test. No unwrap: a field the origin cannot give is `None`.
3. Origins, in order:
   - **git**: as today, except that the commit is `git rev-parse HEAD`, in full;
   - **packaged**: `.cargo_vcs_info.json` beside the manifest gives `git.sha1` and `git.dirty`. Treat an absent `dirty` as `false` only if P1 shows that Cargo writes the field only when dirty; otherwise treat it as unrecorded. `tag = v<CARGO_PKG_VERSION>`;
   - **packager**: `CANDLER_EVAL_INFRA_{TAG,COMMIT,DIRTY}` set in the build's environment, each read with `cargo:rerun-if-env-changed`;
   - **none**: `tag = v<CARGO_PKG_VERSION>` only, with a `cargo:warning` naming the cause.
4. The module doc of `build.rs` says what each origin records.
**Deliverables**: `crates/candler/build_infra.rs`: the identity resolver (its name in the leaf report); `crates/candler/build.rs` using it; `crates/candler/tests/guard_eval_infra.rs`: E1
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'binary(guard_eval_infra)' && git diff --quiet feat/m2c-wire-alignment -- Cargo.lock` (expected: PASS)
**Commit**: `fix(manifest): record a build's identity when it is not a git checkout`
