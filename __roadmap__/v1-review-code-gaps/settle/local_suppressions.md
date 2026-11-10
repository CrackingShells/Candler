# Local suppressions

**Goal**: Every local lint suppression in the workspace is removed or justified in line. Library and binary code suppresses only through a reasoned `#[expect]` that fails the build once it no longer fires, and the lints keep it that way.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout and branches off the milestone after L0: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path, and `git log --first-parent --format=%s HEAD | grep -qx 'merge(importer): land the legacy streams as import sources'`
- [ ] [nl] OWNED:
  - every `crates/*/src/**/*.rs` file that holds a suppression, a crate root (`lib.rs`, `main.rs`) or a call site of the two refactors;
  - every `crates/*/tests/**/*.rs` file that holds a suppression or a call site of the two refactors;
  - the `[workspace.lints.clippy]` table of the root `Cargo.toml`;
  - `AGENTS.md` §Rust: Grit, for one paragraph.
  
  Not `crates/candler-schemas/schema/`.
**Success Gates**:
- ⬜ [cmd] No `#[allow]` in library or binary code: `! git grep -n -E '#!?\[allow\(' -- 'crates/*/src'`
- ⬜ [cmd] Every suppression left in `tests/` carries a reason: `cargo clippy --workspace --all-targets -- -D warnings` passes with `allow_attributes_without_reason = "deny"` in `[workspace.lints.clippy]`
- ⬜ [cmd] `#[allow]` is denied where it can be: each crate root (`crates/*/src/lib.rs`, `crates/candler/src/main.rs`) carries `#![deny(clippy::allow_attributes)]`, and `git grep -c 'deny(clippy::allow_attributes)' -- 'crates/*/src/lib.rs' crates/candler/src/main.rs` counts 5
- ⬜ [cmd] Grit rule 11 is enforced: `exhaustive_enums = "warn"` is in `[workspace.lints.clippy]`, and clippy passes
- ⬜ [cmd] Failing first: the lint lines alone, applied to the L0-merged tree before any other change of this leaf, make `cargo clippy --workspace --all-targets -- -D warnings` fail. The report records the count of errors per lint
- ⬜ [cmd] Stale detection: removing the cast from `runner/exec/mod.rs::micro_usd` while its `#[expect]` stays fails clippy with `unfulfilled_lint_expectations`. The bite, restored after
- ⬜ [cmd] LINUX passes: each `cfg(unix)`, `cfg(linux)` and `cfg(not(unix))` expectation holds on the Linux target
- ⬜ [cmd] DRIFT passes: no schema file moves
- ⬜ [cmd] STD and SUBJ pass
- ⬜ [nl] The leaf report has one row per suppression it found: file:line, lint, decision (removed by refactor, removed as suppressing nothing, kept), and the reason as written. Its totals reconcile with the 37 in `src/` from the hand-over list, plus what L0 added, plus every one in `tests/`
- ⬜ [nl] The Grit reviewer (`rust-conventions:rust-conventions-reviewer`) reviews the leaf's diff with no open finding; a kept suppression it rejects is refactored here or reported as a finding
**References**: [R03](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/conformance-docs-handover.md) — the 37-line list and Eliott's ruling; [R05 Grit](https://github.com/PCfVW/Amphigraphic-Strict/tree/master/Grit) — rule 3 (no `.expect` in library code), rule 11 and `references/enums.md` (`EXHAUSTIVE`); [AGENTS.md](../../../AGENTS.md) — the workspace lints are the enforced subset of Grit; [R06](../../../CONTRIBUTING.md) — `!` and the scopes `resolve`, `adapter-claude-code`, `schemas`, `repo`

## Step 1: The registration steps borrow what they clone
**Goal**: The two registration functions take by reference the values they only `Arc::clone`, so `needless_pass_by_value` no longer fires and its two suppressions go.
**Implementation Logic**:
1. `evaluation/registry/environment.rs::register_evaluation_steps` takes `supervisor: &Arc<Supervisor>`. `resolve/steps/mod.rs::register_m2b_steps` takes `services: &M2bServices`. Both keep their behaviour.
2. Remove the two `#[allow(clippy::needless_pass_by_value)]`.
3. Update every caller: the binary's `commands/common.rs` and the tests that call them. The commit carries `!` and a `BREAKING CHANGE:` footer naming both signatures.
**Deliverables**: `crates/candler-core/src/evaluation/registry/environment.rs` (`register_evaluation_steps(…, supervisor: &Arc<Supervisor>)`); `crates/candler-core/src/resolve/steps/mod.rs` (`register_m2b_steps(resolver, services: &M2bServices)`); their callers
**Consistency Checks**: `cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run -p candler-core -E 'binary(guard_resolve) | binary(guard_registry)'` (expected: PASS)
**Commit**: `refactor(resolve)!: borrow the services the registration steps clone`

## Step 2: The Claude Code driver builds its id and probe once
**Goal**: `ClaudeCodeDriver` holds no `.expect`: its constant id and version probe are built once, by a constructor that can report their error.
**Implementation Logic**:
1. `ClaudeCodeDriver` gains private fields `id: HarnessId` and `probe: CommandSpec`.
   - `pub fn new() -> Result<Self, DriverError>` builds both from `HARNESS_NAME` and `PROGRAM`, with `# Errors` documented.
   - The `const fn new` and the `Default` derive go: `Default` cannot fail.
   - `id()` and `version_probe()` return clones, annotated `// BORROW:`.
2. Remove the two `#[allow(clippy::expect_used)]`.
3. Update every construction site (`ClaudeCodeDriver::new()` / `::default()` in `crates/candler/src/commands/{run,supervise}.rs`, `candler-core/tests/support/job_runtime.rs`, `candler/tests/guard_cli_kill_live.rs`, `candler-harness-claude-code/tests/guard_claude_code_driver.rs` and any other the compiler names) to propagate or, in tests, `expect` the result.
4. The commit carries `!` and a `BREAKING CHANGE:` footer.
**Deliverables**: `crates/candler-harness-claude-code/src/driver.rs` (`ClaudeCodeDriver { id, probe }`, `ClaudeCodeDriver::new() -> Result<Self, DriverError>`); its call sites
**Consistency Checks**: `cargo nextest run -p candler-harness-claude-code && cargo clippy --workspace --all-targets -- -D warnings` (expected: PASS)
**Commit**: `refactor(adapter-claude-code)!: build the driver's id and probe once`

## Step 3: The wire types expect what they suppress
**Goal**: Every suppression in `candler-schemas/src` is a reasoned `#[expect]`, in a commit that touches nothing outside `crates/candler-schemas/src`.
**Implementation Logic**:
Convert each `#[allow]` to `#[expect(<lint>, reason = "…")]`. Where today's reason is a trailing comment, move it into `reason`, and keep a Grit marker as its prefix (`reason = "EXHAUSTIVE: …"`, `"WIRE: …"`).
- `outcome.rs:190,228` and `process.rs:280`: `exhaustive_enums`.
- `outcome.rs:652,662,1378`: `deprecated`, on `Cause::Legacy`, which the type's own `kind` and `is_legacy` and its tests must name.
- `trace.rs:464`: `struct_field_names` (WIRE: `parent_session`).
- `execution.rs:279`: `ref_option`, because serde's `with` passes `&Option`.
- `evaluation.rs:597,1342`: `too_many_arguments`, on private `checked` constructors that validate every field of the struct.

Doc comments are untouched, so no schema file moves. Run DRIFT.
**Deliverables**: the 8 suppressions of `crates/candler-schemas/src/{outcome,process,trace,execution,evaluation}.rs` as `#[expect(…, reason = …)]`
**Consistency Checks**: `cargo clippy -p candler-schemas --all-targets -- -D warnings && CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas -E 'binary(schema_drift)' && test -z "$(git status --porcelain crates/candler-schemas/schema)" && ! git grep -n -E '#!?\[allow\(' -- crates/candler-schemas/src` (expected: PASS)
**Commit**: `style(schemas): justify each lint the wire types expect`

## Step 4: Deny the bare allow and settle every other suppression
**Goal**: The lints that keep the decision are on. Every other suppression in `src/` is removed or a reasoned `#[expect]`, and every one in `tests/` is removed, a reasoned `#[expect]`, or, where `expect` cannot hold, a reasoned `#[allow]`.
**Implementation Logic**:
1. `[workspace.lints.clippy]`: add `allow_attributes_without_reason = "deny"` and `exhaustive_enums = "warn"`.
   - Each crate root gains `#![deny(clippy::allow_attributes)]`: `candler-schemas`, `candler-core`, `candler-harness`, `candler-harness-claude-code` (`lib.rs`) and `candler` (`main.rs`).
   - That reaches library, binary and unit-test code, but not the integration-test crates under `tests/`. There, a support module compiled into several binaries that each use a subset needs `#![allow(dead_code, reason = …)]`, because an `expect` would go unfulfilled in some binaries.
   - Before anything else, record the failing-first count (Success Gates).
2. `src/` decisions:
   - **Remove, suppressing nothing:** `candler/src/main.rs:92` (with its extra block), `candler/src/commands/mod.rs:35`. Measured: neither fires on Linux or macOS.
   - **Remove by refactor:** `evaluation/selection/diff.rs:145`. The unread `generator` field becomes `#[serde(rename = "generator")] _generator`, which keeps `deny_unknown_fields`.
   - **Keep as `#[expect]` + reason:**
     - `EXHAUSTIVE`: `harness/spec.rs:476`, `frames/mod.rs:33`, `selection/diff.rs:21`, `supervise/platform.rs:23`, and the new `#[expect(clippy::exhaustive_enums, reason = "EXHAUSTIVE: …")]` on `evaluation/client/accounting.rs` `ItemOutcome`, which replaces its bare `// EXHAUSTIVE:` comment;
     - a wildcard over a foreign `#[non_exhaustive]` enum, or listed arms before it: `client/session.rs:197`, `supervise/stop.rs:331`, `harness-claude-code/driver.rs:333`, `container/run.rs:131`, `exec/ends.rs:215`;
     - casts with no lossless std conversion, each keeping its `// CAST:`: `import/manifest.rs:385`, `exec/mod.rs:279`, `orchestrate/caps.rs:121`;
     - `print_stderr` as the supervisor's log: `orchestrate/mod.rs:52`;
     - `deprecated`: `import/grading.rs:199,215`;
     - `too_many_arguments`: `container/run.rs:312`;
     - `large_enum_variant`: `jobs/view.rs:12`;
     - `struct_excessive_bools`: `exec/ends.rs:38`, `harness/spec.rs:340`;
     - `unused_async`: `commands/evaluate.rs:120`, under `cfg(not(unix))`;
     - and any suppression L0 added.

     `exhaustive_enums = "warn"` may name another public enum. Decide it as Grit says: `#[non_exhaustive]` if it may gain variants (a public-API change: report it), or an `EXHAUSTIVE` expect.
3. `tests/` decisions:
   - each suppression first tries removal (does the lint fire?), then `#[expect]`, then `#[allow]` with the reason that `expect` cannot hold;
   - the `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` of `crates/candler/tests/*` gains a reason, or goes if `allow-…-in-tests` in `clippy.toml` already covers it.
4. `AGENTS.md` §Rust: Grit gains one paragraph, written as a rule with no history:

   > A local lint suppression is `#[expect(<lint>, reason = "…")]`, so it fails once the lint stops firing. `#[allow]` is denied in `src/`. In `tests/` it is allowed, with a reason, only where an `expect` cannot hold, such as a support module shared by several test binaries. Grit's markers (`EXHAUSTIVE:`, `WIRE:`, `CAST:`) open the reason or stay beside it.
5. Write the leaf report's per-suppression table and send the diff to the Grit reviewer.
**Deliverables**: root `Cargo.toml` (`allow_attributes_without_reason`, `exhaustive_enums`); `#![deny(clippy::allow_attributes)]` in the 5 crate roots; every suppression of `crates/*/src` and `crates/*/tests` decided; `AGENTS.md` (the suppression paragraph)
**Consistency Checks**: `cargo clippy --workspace --all-targets -- -D warnings && cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings && ! git grep -n -E '#!?\[allow\(' -- 'crates/*/src'` (expected: PASS)
**Commit**: `style(repo): deny bare allow attributes and settle every suppression`
