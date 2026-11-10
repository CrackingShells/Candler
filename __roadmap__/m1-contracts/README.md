# M1 Contracts: wire types, services and the golden import

## Context
The campaign of milestone M1 (`../candler-v1/contracts/m1_contracts.md`). It adds every remaining wire type to `candler-schemas`, the `candler-harness` traits, the Claude Code trace reader, the run store, the process supervisor core, the `resolve` skeleton with its config loader, and the legacy importer, and it imports the 240-run golden corpus with every row's status, value, weight, cause and role equal to an independent oracle's. M2a (evaluation) and M2b (execution) start in parallel on these contracts, so the interfaces are reviewed against both before services are built on them. Depth is ordering: L0 wire foundations, trace and oracle; L1 `interfaces/`; L2 `services/`; L3 `import/`; L4 `closeout/`.

## Reference Documents
- [R01 Architecture](../../__design__/architecture.md) — §Operation API (resolution order, value sources, preflight steps, configuration, formats), §Experiment file, §Jobs, runs and supervision, §Raw traces, §Harness drivers, §Plug-ins, §Evaluator protocol, §Data Model, §Wire encodings, §Golden corpus; decisions D1–D11 of the plan live there
- [R02 Milestone leaf](../candler-v1/contracts/m1_contracts.md) — scope, success gates, required verifications
- [R03 Golden corpus](/Users/hacker/Documents/src/External/PCfVW/agenticai4hpc26-artifact) — tag `candler-m2a-golden`, `campaign/20260806-cont-train/`; read with `git -C <repo> show candler-m2a-golden:<path>` or from the `git archive` extraction; never written
- [R04 Test definition](../../__reports__/m1-contracts/00-test_definition_v0.md) — test classes, oracle rules, the guard list per leaf, coordinator rulings
- [R05 Grit](https://github.com/PCfVW/Amphigraphic-Strict/tree/master/Grit) — the `rust-conventions` skill and its reviewer agent
- [R06 CONTRIBUTING](../../CONTRIBUTING.md) — commit types and scopes, versioning lines, integration through pull requests
- [R07 Parity milestones](../candler-v1/contracts/parity/README.md) — `m2a_evaluation_parity.md` and `m2b_execution_parity.md`, the consumers of these interfaces
- [R08 Wire-schemas lessons](../../__reports__/wire-schemas/00-knowledge_transfer_v0.md) — pain points this campaign's conventions answer

## Goal
Every M1 contract and shared service exists on `feat/m1-contracts`, guarded by cornerstone tests with independent oracles, and the golden corpus imports into a Candler store with every row's status, value, weight, cause and role equal to an independent oracle's.

## Pre-conditions
- [x] [nl] Eliott approved the plan (2026-11-05)
- [x] [cmd] `git merge-base --is-ancestor origin/main feat/m1-contracts`
- [x] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the checkout's path
- [x] [cmd] `rustup target list --installed | grep -qx x86_64-unknown-linux-gnu`

## Success Gates
- ✅ [cmd] STD and DRIFT pass on `feat/m1-contracts`, and `cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings` passes
- ✅ [cmd] `CANDLER_GOLDEN_CORPUS="$GOLDEN" cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_golden)'` passes: 240 runs, 10,320 schema-valid outcome records equal to the oracle
- ✅ [cmd] SUBJ passes over `origin/main..feat/m1-contracts`
- ✅ [cmd] `diff <(cd crates/candler-schemas/schema && printf '%s\n' *.schema.json | sort) __roadmap__/m1-contracts/interfaces/services/import/closeout/expected_roots.txt` prints nothing
- ✅ [nl] Every leaf is `done` and every required verification and Grit review reports no open finding

## Gotchas
- **Condition tags.** Every Pre-condition and Success Gate line starts with `[cmd]` (a command whose exit status decides) or `[nl]` (a judgement, naming a proxy check where one exists). An untagged line is a defect in the leaf.
- **Standard gates**, named in every leaf:
  - **STD**: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`.
  - **DRIFT**: `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas -E 'binary(schema_drift)' && test -z "$(git status --porcelain crates/candler-schemas/schema)" && git diff --exit-code origin/main -- crates/candler-schemas/schema/outcome-record.schema.json crates/candler-schemas/schema/catalog-snapshot.schema.json`.
  - **SUBJ**: `cog check <base>..HEAD && git log --format=%s <base>..HEAD | awk 'length>72{b=1}END{exit b}'`, with `<base>` the leaf's branch point on `feat/m1-contracts`.
  - **DEP(x)**: `git log --first-parent --format=%s feat/m1-contracts | grep -qx 'merge(<scope>): land x'`, and the consumer's `tests/public_surface.rs` compiles.
- **Schema publish rule.** A commit that changes a file under `crates/candler-schemas/schema/` uses scope `schemas` and touches nothing outside `crates/candler-schemas/`: the module's `roots()`, its schema attributes, and the `schema/*.json` files.
- **Owned files.** Implementers do not edit `__design__/`, `__roadmap__/`, READMEs, `tests/schema_drift.rs`, `generate.rs` (except `wire_foundations`, in its `mod tests` only), `primitives.rs` (except `wire_foundations`), workspace `Cargo.toml` dependency tables, or another leaf's modules. A new dependency or a design ruling is a request to the coordinator, who lands it in its own `build(repo)` or `docs(design)` commit.
- **Test classes** (R04):
  - A leaf commits only cornerstone guards (class B). Each carries a doc line `/// Guards: <I#|D#|Cond#> — <property in domain terms>. Oracle: <source>.`
  - Integration-test files are `tests/guard_<topic>.rs` with `#![cfg(test)]`; pure-function guards may sit in a `#[cfg(test)] mod guards` beside the code.
  - Development scaffolding (class A) lives only in `#[cfg(test)] mod scaffold`; the leaf's last commit deletes it unless a test is promoted to B with its `Guards:` line.
  - Tautological tests are not committed: an oracle that is the code, the generated schema or a constant the code also uses; re-checks of serde or schemars derives; goldens blessed from the output they check.
  - **Bite record.** The leaf report names, for each B test, a one-line mutation of the implementation that makes it fail, and says it was tried.
  - The existing `outcome_*`, `catalog_*` and `schema_drift` test files stay as they are.
- **Oracle rules:**
  1. Expected values come from spec text, raw source data, another engine (Node, Python `rfc8785` via `uv run --with`, the OS), or a hand-written literal; never from the code under test, the generated schema or a shared constant.
  2. An implementer never writes the oracle for their own leaf; `golden_oracle.md` is written by an agent who never sees the importer.
  3. Blessed snapshots (`schema/*.json`, `digests.toml`) are ratchets, not evidence: a bless needs a version or schema-tag change and a diff a human reads, and a gate fails on any edited existing line.
  4. A property test names its property in domain terms; "round trip" alone is not a property.
  5. A refusal test injects one fault into a valid base and includes the positive control (the base passes).
  6. Every scan guard uses `tests/support::assert_clean(dir, needle)`, which has a planted-violation self-test and asserts the scan is non-empty.
  7. Timing asserts use generous slack; a flaky test is a defect, never retried.
- **Ignored and platform tests** run with `--no-tests=fail`. The golden test panics when `CANDLER_GOLDEN_CORPUS` is unset and asserts 240 runs and 10,320 rows before comparing. Every ignored run's log goes in the leaf report.
- **Fixture budget.** New committed fixtures total ≤ 3 MB: real streams ≤ 150 KB gz each, each kept once: the mini-corpus's three in `candler-core` (≤ 1.5 MB with the rest of the mini-corpus), read by the adapter through a relative path, and three more in the adapter (≤ 1 MB with its synthetic streams). Synthetic fixtures carry the `synthetic_` prefix and `synthetic = true` in `PROVENANCE.toml`, and are authored from spec text or by a committed generator, never captured from our own writer. Vectors ship with their generator script and name the engine that made them.
- **`$GOLDEN`** is `/Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`, extracted once by the coordinator with `golden_oracle.md`'s `extract_corpus.sh` after that leaf lands; leaves read it and never write in it.
- **Shell safety** (R08): absolute paths only; commands chained with `&&` or under `set -e`; never run git in another checkout; commit messages in unique `$TMPDIR/m1-<leaf>-*` files, committed with `git commit -F` after `cog verify --file`.
- **Environment.** In a worktree, first check that `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path. Local tools: cargo 1.99 with `cargo nextest`, `uv run` (no mamba env), Node, the `x86_64-unknown-linux-gnu` target. Linux-only gates are decided by the PR's CI.
- **Grit lessons up front:** one-paragraph schema docs (schemars keeps hard line breaks); `BORROW` and `INDEX` annotations in tests; `#![cfg(test)]` in integration tests; explicit lifetimes; list every enum variant before `_`; source says what is, with no history or meta-discourse.
- **Wire rules carried over:** `null` is never written and is refused on input; structs are read from objects only; patterns end with `(?![\s\S])` and never use bare `$`, `.` or `\s`; MCP keys stay camelCase, every other key is snake_case.
- **Spec defects** found while implementing are reported to the coordinator as findings, never implemented faithfully.
- **Verification policy.** Checks a test can decide are `[cmd]` gates the implementer runs; verifiers judge. An api_review verifier closes L1 (contract fit against R07, OpenCode facts in R01, `run_config`, `eval.json`) before L2 is dispatched. `supervisor.md` and `legacy_import.md` (with the `legacy_mapping.md` diff) have a required adversarial verifier; `claude_code_reader.md` has a verifier; one Grit reviewer per level covers that level's branches, and also checks that every `tests/*.rs` file is a `guard_*`, no `mod scaffold` survives, and every B test has its `Guards:` line. Every verifier names the tautological tests, states each committed test's oracle, re-runs three bite mutations from the leaf report plus one of its own, and lists the tests that survive. Fixes go back to the implementer; confirmation goes to the verifier that raised the finding.
- **Critical conditions,** ranked: (1) cell-digest content: what is hashed, list order, JCS integers; (2) frame and trace shapes: index base, optional times, `SessionId`, `EndedBy`, `Usage.scope`; (3) supervisor API: `reap`, envelope ownership, the builder; (4) traits: `Send`, Document framing, `observed()`; (5) `PreflightStep` and `ResolveContext`; (6) store atomicity and layout; (7) `context_digest` projection and the legacy-cause refusal; (8) importer identity: rules and the M2a join key.
- **Nothing is pushed** except `feat/m1-contracts` at landing; nothing is published; nothing is written in R03.

## Status
```mermaid
graph TD
    wire_foundations[Wire foundations: values, RFC 8785, child record]:::done
    wire_trace[Wire trace: raw frames and trace records]:::done
    golden_oracle[Golden oracle: independent expected rows]:::done
    interfaces[Interfaces]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `wire_foundations.md` | 📄 Leaf Task | ✅ Done |
| `wire_trace.md` | 📄 Leaf Task | ✅ Done |
| `golden_oracle.md` | 📄 Leaf Task | ✅ Done |
| `interfaces/` | 📁 Directory | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
