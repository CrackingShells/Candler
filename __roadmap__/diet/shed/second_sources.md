# second_sources

**Goal**: Each fact the CLI prints or checks has one source: the M6b follow-ups that remove a second source.
**Estimate**: about 40 Sonnet calls ($1.3 with the verifier), cap 60. At 60 calls, stop, commit what is green, and list what is left in the leaf report.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/diet HEAD` succeeds, with every L0 piece merged into `feat/diet`
- [ ] `[nl]` OWNED = `^crates/candler/src/(commands/(resolve|watch|evaluate)\.rs|main\.rs)$|^crates/candler/build\.rs$|^crates/candler-core/src/(resolve/(layers|paths)\.rs|isolation/image\.rs|trace/transcript\.rs|resolve/record[a-z_]*\.rs)$|^crates/candler-schemas/|^crates/candler/tests/it/guard_cli[a-z_]*\.rs$|^__design__/architecture\.md$|^docs/|^__reports__/diet/leaves/second_sources`
**Success Gates**:
- ⬜ `resolve` failures print a versioned record from `candler-core`, not a hand-built `json!`; its schema is generated and the drift guard passes
- ⬜ `watch --live` is retired; `watch --format text` is the one live view
- ⬜ Key names, the five path texts in `resolve/paths.rs`, `HOME` in `probe_env` and the 0/1/2 exit statuses each come from one named constant or type
- ⬜ "stratum" no longer means a comparability class anywhere outside R01 §Analysis statistics; `transcript.rs` no longer folds a serialisation failure into an empty line
- ⬜ `cargo nextest run -p candler -p candler-core -p candler-schemas` passes
- ⬜ `cargo fmt --check` passes
- ⬜ `cargo clippy --workspace --all-targets -- -D warnings` passes
- ⬜ `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes
- ⬜ DIGEST unchanged: `cargo nextest run --workspace -E 'test(/cell_digest|context_digest|build_context/)'` passes with no vector edited
- ⬜ GOLDEN unchanged: `cargo nextest run --workspace -E 'test(/golden/)'` passes with no vector edited
- ⬜ Ownership: `git diff --name-only feat/diet...HEAD | grep -vE '^crates/candler/src/(commands/(resolve|watch|evaluate)\.rs|main\.rs)$|^crates/candler/build\.rs$|^crates/candler-core/src/(resolve/(layers|paths)\.rs|isolation/image\.rs|trace/transcript\.rs|resolve/record[a-z_]*\.rs)$|^crates/candler-schemas/|^crates/candler/tests/it/guard_cli[a-z_]*\.rs$|^__design__/architecture\.md$|^docs/|^__reports__/diet/leaves/second_sources'` prints nothing
- ⬜ A fresh Sonnet verifier, given only this leaf and the diff, reports no promise left without a test and no gate unmet
**References**: [Diet plan](../../../__reports__/diet/00-architecture_diet_plan_v0.md) — §What the codebase sheds and §M6b's open question, answered; [AGENTS.md](../../../AGENTS.md) — checks and conventions

## Step 1: The resolve failure record
**Goal**: `resolve` renders a core record, as every other verb does.
**Implementation Logic**:
Replace `commands/resolve.rs:200-219 print_failures` with a versioned failure record built in `candler-core` and rendered by the CLI, like `evaluate`'s. Generate its schema and run the drift guard with `CANDLER_BLESS_SCHEMAS=1` once. One CLI guard row in `guard_cli_refusal` or `guard_cli_surface` reads the new record; see it red against the old output.
**Deliverables**: `commands/resolve.rs`, the core record type, its schema under `candler-schemas`, one guard row.
**Consistency Checks**: `cargo nextest run -p candler-schemas drift` and `-p candler` (expected: PASS)
**Commit**: `feat(cli)!: print resolve failures as a versioned record`

## Step 2: Retire watch --live and literal second sources
**Goal**: One spelling per fact.
**Implementation Logic**:
Remove `--live` from `commands/watch.rs` (24-35, 81-87); `--format text` is the live view. Name the key literals (`layers.rs:371 is_location`, the value readers, the `slots` check, `paths.rs` texts including `REPLICATES`) from the constants that define them. Type the exit statuses in `main.rs:122` (`usage_failure`) as one enum. Take `HOME` in `isolation/image.rs:936-943` from the constant the image layer already defines. Reword `build.rs:66`. Make `trace/transcript.rs:20-52` return the error instead of a degraded line.
**Deliverables**: the owned files listed above.
**Consistency Checks**: `cargo nextest run -p candler -p candler-core` (expected: PASS)
**Commit**: `refactor(cli)!: retire watch --live and give each key and status one source`

## Step 3: Verify
**Goal**: An agent that did not write the change attacks it.
**Implementation Logic**:
Hand a fresh Sonnet verifier this leaf, the leaf report and `git diff feat/diet...HEAD`, with the brief: find any fact still spelled in two places among the ones this leaf names, and any CLI output that changed without a guard row. Fix what it finds with a fresh agent, once; list the rest as follow-ups.
**Deliverables**: `__reports__/diet/leaves/second_sources_v0.md` with First reads, Bites, Verifier and Follow-ups.
**Consistency Checks**: the Success Gates above (expected: PASS)
**Commit**: `reports(api): close second_sources`
