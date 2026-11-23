# silent_acceptance

**Goal**: Input that Candler accepts today and then ignores or misreads is refused with a code and a pointer.
**Estimate**: about 35 Sonnet calls ($1.2 with the verifier), cap 55. At 55 calls, stop, commit what is green, and list what is left in the leaf report.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/diet HEAD` succeeds, with every L0 piece merged into `feat/diet`
- [ ] `[nl]` OWNED = `^crates/candler-core/src/(resolve/[a-z_/]+\.rs|runner/plan\.rs|runner/preflight\.rs|jobs/orchestrate/servers\.rs|refusal[a-z_/]*\.rs)$|^crates/candler-core/tests/it/guard_silent_acceptance\.rs$|^crates/candler-core/tests/it/main\.rs$|^docs/contract/codes\.toml$|^docs/|^__reports__/diet/leaves/silent_acceptance`
**Success Gates**:
- ⬜ `ResolveInput::secret_names` is required (no empty default)
- ⬜ An `env` name equal to `model.token_secret` is refused
- ⬜ A `[fixed] replicates` that conflicts with a level, a `replicates.<x>` path and a non-integer replicate count are each refused with their own code
- ⬜ A non-string MCP server `command` or `args` is refused
- ⬜ New codes are appended to `codes.toml`; no row is edited or deleted
- ⬜ One table-driven guard, `guard_silent_acceptance`, holds one row per case and was seen red before each fix
- ⬜ `cargo nextest run -p candler-core -p candler` passes
- ⬜ `cargo fmt --check` passes
- ⬜ `cargo clippy --workspace --all-targets -- -D warnings` passes
- ⬜ `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes
- ⬜ DIGEST unchanged: `cargo nextest run --workspace -E 'test(/cell_digest|context_digest|build_context/)'` passes with no vector edited
- ⬜ GOLDEN unchanged: `cargo nextest run --workspace -E 'test(/golden/)'` passes with no vector edited
- ⬜ Ownership: `git diff --name-only feat/diet...HEAD | grep -vE '^crates/candler-core/src/(resolve/[a-z_/]+\.rs|runner/plan\.rs|runner/preflight\.rs|jobs/orchestrate/servers\.rs|refusal[a-z_/]*\.rs)$|^crates/candler-core/tests/it/guard_silent_acceptance\.rs$|^crates/candler-core/tests/it/main\.rs$|^docs/contract/codes\.toml$|^docs/|^__reports__/diet/leaves/silent_acceptance'` prints nothing
- ⬜ A fresh Sonnet verifier, given only this leaf and the diff, reports no promise left without a test and no gate unmet
**References**: [Diet plan](../../../__reports__/diet/00-architecture_diet_plan_v0.md) — §What the codebase sheds (silent acceptance) and §Compat and breaking changes; [AGENTS.md](../../../AGENTS.md) — checks and conventions

## Step 1: Guard first
**Goal**: One row per silent case, red on the L0 tip.
**Implementation Logic**:
Write `guard_silent_acceptance` at the core operation API: each row is an input file and the refusal code and pointer it must produce. Find where each value is owned with colgrep (`secret_names` in `resolve/layers.rs:116-166`; `replicates` in `runner/plan.rs:354-369` and `runner/preflight.rs:269-283`; server `command`/`args` in `jobs/orchestrate/servers.rs:36-69`). Run it and record each row red.
**Deliverables**: `crates/candler-core/tests/it/guard_silent_acceptance.rs`.
**Consistency Checks**: every row fails on the tip (expected: FAIL)
**Commit**: `test(resolve): guard the inputs accepted and ignored today`

## Step 2: Refuse at the owner
**Goal**: Each check lives where its value is read, once.
**Implementation Logic**:
Make `secret_names` a required field. Refuse an `env` name equal to `model.token_secret` in the resolve layer that reads both. Refuse the replicate conflicts in the one place that reads `replicates` (if the plan and preflight both read it, keep the check in one and remove the other read). Refuse non-string server fields instead of skipping them. Append one code per case to `docs/contract/codes.toml`; regenerate the contract pages with the bless command only if a page guard asks.
**Deliverables**: the owned source files; `docs/contract/codes.toml`.
**Consistency Checks**: `guard_silent_acceptance` passes; `cargo nextest run -p candler-core -p candler` (expected: PASS)
**Commit**: `feat(resolve)!: refuse inputs that were silently accepted`

## Step 3: Verify
**Goal**: An agent that did not write the change attacks it.
**Implementation Logic**:
Hand a fresh Sonnet verifier this leaf, the leaf report and `git diff feat/diet...HEAD`, with the brief: find any silent case listed in this leaf still accepted, any check now made in two places, and any existing experiment fixture newly refused without its test saying so. Fix what it finds with a fresh agent, once; list the rest as follow-ups.
**Deliverables**: `__reports__/diet/leaves/silent_acceptance_v0.md` with First reads, Bites, Verifier and Follow-ups.
**Consistency Checks**: the Success Gates above (expected: PASS)
**Commit**: `reports(api): close silent_acceptance`
