# v1-review-code-gaps

## Context
A fix campaign off `main` (025754d, v0.4.1, schemas@0.4.1). It closes the three code gaps that the v1 conformance review found and that CrackingShells/Candler#8 left out on purpose: the agent container has no `container-record/1`, the legacy streams are not import sources, and 37 local lint suppressions were never decided. R01 already states the rule for the first two, so the code changes and R01 does not. No open milestone owns any of the three (M2c's A2, M4's and M5's A3 checked 2026-11-10).
Depth is ordering. L0, this level, holds the two behavioural fixes, which share no file. L1, `settle/`, holds the suppression pass, which edits files L0 edits and whose deny lints must also cover L0's new code, and the digest check, which needs both fixes in place.

## Reference Documents
- [R01 Architecture](../../__design__/architecture.md) — §Process supervision (container records, destroying containers), §Golden corpus (every source file kept as a blob), §Wire encodings
- [R02 Conformance review](../../__reports__/candler-v1-review/00-findings_v0.md) — the findings this campaign follows from
- [R03 Docs hand-over](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/conformance-docs-handover.md) — the two code gaps, the 37-line list, Eliott's ruling that the Grit reviewer judges each suppression
- [R04 M2b architecture report](../../__reports__/m2b-execution-parity/00-architecture_v0.md) — §10.1 job dir: "the container record is written as soon as the cid is known"; the destroy rule R10
- [R05 Grit](https://github.com/PCfVW/Amphigraphic-Strict/tree/master/Grit) — the `rust-conventions` skill (`references/enums.md` for `EXHAUSTIVE`) and its reviewer agent
- [R06 CONTRIBUTING](../../CONTRIBUTING.md) — commit types, scopes, breaking changes and the core line's bumps
- [R07 Implementer brief](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/gaps-implementer-brief.md) — read order, owned paths, commit flow, the leaf report

## Goal
An agent container is recorded from the moment its cid is known, every legacy stream is an import source kept as a blob, and every remaining lint suppression is a reasoned `#[expect]` that the workspace lints keep honest, with no cell digest moved.

## Pre-conditions
- [x] [nl] Eliott approved the plan (2026-11-10)
- [x] [cmd] The milestone branch starts at main: `git merge-base --is-ancestor origin/main fix/v1-review-code-gaps`
- [x] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the checkout's path
- [x] [cmd] The golden corpus is in place: `test -d /Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`

## Success Gates
- ⬜ [cmd] STD and LINUX pass on `fix/v1-review-code-gaps`
- ⬜ [cmd] SUBJ passes over `origin/main..fix/v1-review-code-gaps`
- ⬜ [cmd] No `#[allow]` is left in library or binary code, and every suppression in the workspace carries a reason: `! git grep -n -E '#!?\[allow\(' -- 'crates/*/src'`, each crate root denies `clippy::allow_attributes`, and `cargo clippy --workspace --all-targets -- -D warnings` passes with `allow_attributes_without_reason` denied in `[workspace.lints.clippy]`. In `tests/` an `#[allow]` stays only where an `#[expect]` cannot hold (a support module shared by several test binaries), with its reason
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_container_record) | binary(guard_job_orchestrate) | binary(guard_import_mini)'` passes, with the new guards of both L0 leaves among the tests run
- ⬜ [cmd] `CANDLER_GOLDEN_CORPUS="$GOLDEN" cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_golden)'` passes on the milestone branch
- ⬜ [nl] The digest report (`__reports__/v1-review-code-gaps/digest_invariance_v0.md`) shows the run keys (and so the cell digests) and the outcome context digests of the golden and mini-corpus imports identical between `main` and the milestone branch
- ⬜ [nl] Each leaf report records the bite of every new guard: the guard fails with the fix reverted
- ⬜ [nl] Every leaf is `done`, and the spec verifier and the Grit reviewer report no open finding
- ⬜ [cmd] The PR to `main` is titled `merge(isolation): close the v1 review's three code gaps (#N)`, at most 72 characters, and its CI is green: `gh pr checks <N>`

## Gotchas
- **Condition tags.** Every Pre-condition and Success Gate line starts with `[cmd]` (a command whose exit status decides) or `[nl]` (a judgement).
- **Standard gates**, named in every leaf:
  - **STD**: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`;
  - **LINUX**: `cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings`;
  - **DRIFT**: `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas -E 'binary(schema_drift)' && test -z "$(git status --porcelain crates/candler-schemas/schema)"`;
  - **SUBJ**: `cog check <base>..HEAD && git log --format=%s <base>..HEAD | awk 'length>72{b=1}END{exit b}'`, with `<base>` the leaf's branch point on `fix/v1-review-code-gaps`;
  - **SPAWN**, before every test run that may spawn a process: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)'`.
- **`$GOLDEN`** is `/Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`, extracted by M1. Read it; never write in it.
- **Build footprint (shared machine).** Run every cargo command with `RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0` exported. Each worktree keeps its own `target/`; never set `CARGO_TARGET_DIR`. At most two leaves build at once. The coordinator removes a leaf's worktree and `target/` as soon as the leaf is merged.
- **Safety.** The rules of M2b's Gotchas hold. Signals go through the I22 guard only; containers are destroyed only through `ContainerGuard`; bite tests of stopping or destroying use the recording fakes of `test-support`; never `kill`, `pkill`, `killall`, `docker rm -f $(…)`, `docker … prune`, or a `docker stop` or `rm` of an id the test did not create. A real-daemon test destroys only the containers it created, by exact id.
- **Digests.** Cell digests come from `comparable`; context digests project outputs, snapshots, inputs and rule metadata; run keys and request ids come from the cell digest and the replicate. None of the three fixes touches these inputs. A digest that moves is a defect to report, never a result to bless.
- **Breaking changes.** The import fix changes every imported manifest's `import.sources[]`, so a store imported by ≤0.4.1 fails re-import with `Conflict`. Its commit carries `!` and a `BREAKING CHANGE:` footer whose body names the migration (re-import into a fresh store). The two signature refactors of `settle/local_suppressions.md` also carry `!`.
- **Owned files.** Each leaf lists its OWNED paths. Implementers never edit `__design__/`, `__roadmap__/`, another leaf's files, `cog.toml`, `.github/`, or `Cargo.lock`. Only `settle/local_suppressions.md` edits the root `Cargo.toml` (its `[workspace.lints.clippy]` table) and `AGENTS.md`.

## Status
```mermaid
graph TD
    agent_container_record[Agent container record]:::planned
    import_stream_sources[Import stream sources]:::planned
    settle[Settle]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `agent_container_record.md` | 📄 Leaf Task | ⬜ Planned |
| `import_stream_sources.md` | 📄 Leaf Task | ⬜ Planned |
| `settle/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
