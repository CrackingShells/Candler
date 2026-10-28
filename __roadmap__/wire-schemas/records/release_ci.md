# Release tooling and CI workflows

**Goal**: release-plz versions the core and the schemas on their own tag lines without publishing anything, and CI runs the local gates from the first push.
**Pre-conditions**:
- [x] `../workspace.md` and `../commit_tooling.md` are `done` and merged on `feat/wire-schemas`
**Success Gates**:
- ⬜ `actionlint .github/workflows/*.yml` passes
- ⬜ In a throwaway clone of the branch that carries the configuration, `release-plz update` exits 0, computes `candler-schemas` 0.1.0 against tags `schemas@*` and the core group against tags `v*`, and a probe shows a `fix(schemas)` bumps only the schemas and a `feat(api)` only the core group
- ⬜ `release-plz.toml` runs git-only (`git_only = true`) with `publish = false` and `git_release_enable = false`; `release-plz release --dry-run` needs a forge and a token, so it is not a gate before the first push
- ⬜ No workflow and no config publishes to crates.io or creates a GitHub release: `grep -nE 'publish *= *true|git_release_enable *= *true|CARGO_REGISTRY_TOKEN' release-plz.toml .github/workflows/*.yml` prints nothing
**References**: [R06 CONTRIBUTING §Versioning](../../../CONTRIBUTING.md) — the two lines and their bump rules; [R01 §Workspace](../../../__design__/architecture.md) — release lines per crate

## Step 1: Configure release-plz
**Goal**: Each crate has its own version and changelog. The core crates share the `v*` line and `candler-schemas` has the `schemas@*` line.
**Implementation Logic**:
`release-plz.toml`:
- `candler`, `candler-core`, `candler-harness` and `candler-harness-claude-code` form one `version_group`, tagged `v{{ version }}` on `candler` only;
- `candler-schemas` is tagged `schemas@{{ version }}`;
- changelogs come from Conventional Commits, with `rdm`, `reports`, `merge`, `test`, `ci`, `build` and `chore` producing no release entry, per CONTRIBUTING;
- publishing is off: `publish = false`, and `git_release_enable = false`.

Also set `publish = false` in each crate's `Cargo.toml` if release-plz still versions and tags crates marked that way (check the docs with context7 and report which you chose).
**Deliverables**: `release-plz.toml`; crate `Cargo.toml` files (`publish`, if chosen)
**Consistency Checks**: `release-plz update --help >/dev/null` (expected: PASS)
**Commit**: `build(release): version the core and the schemas on their own tag lines`

## Step 2: Prepare release PRs
**Goal**: A push to `main` opens or updates a release PR and publishes nothing.
**Implementation Logic**:
`.github/workflows/release-plz.yml`: on push to `main`, run `release-plz release-pr` with the repository token only. No registry token and no `release` job until Eliott enables publishing.
**Deliverables**: `.github/workflows/release-plz.yml`
**Consistency Checks**: `actionlint .github/workflows/release-plz.yml` (expected: PASS)
**Commit**: `ci(release): prepare release PRs from Conventional Commits`

## Step 3: Run the local gates in CI
**Goal**: Every push and pull request runs the same commands that pass locally.
**Implementation Logic**:
`.github/workflows/ci.yml` runs on push and on pull requests (cog through `cocogitto/cocogitto-action`, which `taiki-e/install-action` does not carry). Its steps:
- the pinned toolchain from `rust-toolchain.toml`;
- `cargo fmt --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo build --workspace`;
- `cargo nextest run --workspace` (nextest through `taiki-e/install-action`);
- `cog check` on the pushed range (cocogitto through `cocogitto/cocogitto-action`).

The commands are the same strings as the campaign's gates.
**Deliverables**: `.github/workflows/ci.yml`
**Consistency Checks**: `actionlint .github/workflows/ci.yml` (expected: PASS)
**Commit**: `ci(repo): run the local gates on every push`
