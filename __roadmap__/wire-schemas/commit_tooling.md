# Commit tooling: installs, cog and the commit-msg hook

**Goal**: `cog`, `release-plz` and `cargo-nextest` are installed and documented, and every commit is checked against CONTRIBUTING's convention locally.
**Pre-conditions**:
- [ ] CONTRIBUTING declares the `rdm` and `reports` types (`feat/wire-schemas`, commit `docs(contributing): give roadmaps and reports their own commit types`)
**Success Gates**:
- ⬜ `cog --version`, `release-plz --version` and `cargo nextest --version` each exit 0
- ⬜ `cog check main..HEAD` passes on this branch (confirm the range syntax with `cog check --help`)
- ⬜ The installed `commit-msg` hook exits non-zero on a message file holding `feat: no scope` and on one holding `rdm: no scope`, and exits 0 on `rdm(schemas): plan x` and on `reports(schemas): close x`
- ⬜ The types and scopes in `cog.toml` equal the ones in CONTRIBUTING's tables (a one-line comparison, recorded in the step's report)
**References**: [R06 CONTRIBUTING](../../CONTRIBUTING.md) — types, scopes, Enforcement; [R01 §Workspace](../../__design__/architecture.md) — tooling

## Step 1: Install and document the tools
**Goal**: A contributor can install the exact tools from CONTRIBUTING.
**Implementation Logic**:
Install with `brew install cocogitto cargo-nextest` and `cargo install --locked release-plz` (Homebrew has no `release-plz` formula). Add a "Tools" section to CONTRIBUTING with these commands, the versions installed, and `cog install-hook --all` as the per-clone step.
**Deliverables**: `CONTRIBUTING.md` (§Tools)
**Consistency Checks**: `cog --version && release-plz --version && cargo nextest --version` (expected: PASS)
**Commit**: `docs(contributing): say how to install cog, release-plz and nextest`

## Step 2: Check commits with cog
**Goal**: `cog` knows the convention, and the hook enforces it on every commit.
**Implementation Logic**:
`cog.toml`: every CONTRIBUTING type, with the custom `merge`, `rdm` and `reports`; the scope list of CONTRIBUTING's table, if this cog version restricts scopes; `[git_hooks.commit-msg]` running a scope-presence regex (`^[a-z]+\([a-z0-9-]+\)!?: `) and then `cog verify --file "$1"`. Install the hook with `cog install-hook --all`. No changelog or bump settings: `release-plz` owns releases.
**Deliverables**: `cog.toml` (types, scopes, `[git_hooks.commit-msg]`)
**Consistency Checks**: `cog check main..HEAD` (expected: PASS)
**Commit**: `chore(repo): enforce the commit convention with cog`

## Step 3: Describe the enforcement that exists
**Goal**: CONTRIBUTING §Enforcement says what runs, where, and how to install it.
**Implementation Logic**:
Rewrite §Enforcement: the `commit-msg` hook (what it checks, how to install it), `cog check` in CI on the pushed range, and the fact that commits made before `cog.toml` are outside every checked range.
**Deliverables**: `CONTRIBUTING.md` (§Enforcement)
**Consistency Checks**: `cog check main..HEAD` (expected: PASS)
**Commit**: `docs(contributing): describe the commit checks that now run`
