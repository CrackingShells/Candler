# Wire schemas: outcome record and catalog snapshot

## Context
The campaign of milestone `wire_schemas` in `candler-v1`. It produces the cargo workspace, the commit and release tooling, and `candler-schemas` with the outcome record and the tool-catalog snapshot. M1 adds the remaining wire types to the same crate and drift check, and HTN Phase 4.5 reads the snapshot schema.

## Reference Documents
- [R01 Architecture](../../__design__/architecture.md) — §Workspace, §Data Model (§Wire encodings, §Wire schema: outcome record, §Wire schema: tool-catalog snapshot), §Golden corpus
- [R02 Milestone leaf](../candler-v1/wire_schemas.md) — scope, success gates, adversarial-verification brief
- [R03 Golden corpus](/Users/hacker/Documents/src/External/PCfVW/agenticai4hpc26-artifact) — tag `candler-m2a-golden`, `campaign/20260806-cont-train/` (`expectations.jsonl`, `PROVENANCE.json`, per-run `eval-N-grading.json`), read with `git show candler-m2a-golden:<path>`
- [R04 HTN eval](/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion) — tag `htn-creator-eval@0.4.2` (`28be2c15`): `.claude/skills/htn-creator-workspace/infrastructure/htn_creator_eval/catalogs/rikyu_mcp.json`, `infrastructure/scripts/capture_catalog_snapshot.py`, `servers/bio-opentrons/`, `htn_creator_eval/_outcome.py`
- [R05 Grit](https://github.com/PCfVW/Amphigraphic-Strict/tree/master/Grit) — `Cargo.toml` lints, `lib.rs`, `prompt.md`; the `rust-conventions` skill and its reviewer agent
- [R06 CONTRIBUTING](../../CONTRIBUTING.md) — commit types and scopes, versioning lines, integration

## Goal
The workspace, its commit and release tooling, and `candler-schemas` with the outcome record and the tool-catalog snapshot exist on `feat/wire-schemas`, validated against domain-free properties, the MCP specification and real examples.

## Pre-conditions
- [x] The pre-conditions of `../candler-v1/wire_schemas.md` hold
- [x] Eliott approved the plan (2026-10-28)
- [x] The wire encodings are recorded in R01 §Data Model on `feat/wire-schemas` (5f49685)

## Success Gates
- ⬜ `cargo build --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` pass
- ⬜ `cargo test -p candler-schemas` passes, including tests that validate against the generated JSON Schema: the rikyu snapshot, a bio-opentrons snapshot, golden outcomes of the four statuses the corpus has (pass, fail, blocked with its artifact cause, not_measured), and synthetic `not_applicable` and text-valued outcomes
- ⬜ The committed JSON Schema files match the structs: the drift test fails on a deliberate drift and passes after `CANDLER_BLESS_SCHEMAS=1`
- ⬜ `cog check main..HEAD` passes, and the `commit-msg` hook rejects a message without a scope
- ⬜ The catalog snapshot schema is ready to land on `main` for HTN Phase 4.5

## Gotchas
- Half the `*-grading.json` blobs in the golden tag are AppleDouble `._*` files; skip them.
- JSON Schema treats `3.0` as an integer while serde's `u64` rejects it; writers emit integer literals, and a test pins the difference.
- `release-plz` has no Homebrew formula: `cargo install --locked release-plz`. `cog` may not require a scope by itself; the hook checks the scope with a regex before `cog verify`.
- MCP tool objects keep their camelCase keys (`inputSchema`, `outputSchema`); every other Candler key is snake_case.
- schemars turns `///` docs into schema `description`s: editing a doc comment on a wire type drifts the committed schema, and that commit is `schemas`-scoped.
- Source files say what is: no history, origin or meta-discourse in comments, docs, `#[deprecated(note)]` or `$comment`. Reasons go to `__design__/` or `__reports__/`.
- `wildcard_enum_match_arm = deny` meets downstream matches on `#[non_exhaustive]` enums in M1; `workspace.md` records how they interact.
- Identity fields of the golden fixtures (cell digest, `suite_entry_digest`, `context_digest`, `request_id`) are synthetic; M1's importer owns their real derivation.
- Judge every gate by exit status. In a worktree, first check that `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path.
- Nothing is pushed in any repository; nothing is published.

## Status
```mermaid
graph TD
    workspace[Workspace and schema foundation]:::inprogress
    commit_tooling[Commit tooling: installs, cog and the commit-msg hook]:::inprogress
    fixtures[Fixtures: golden shape classes, real catalogs, MCP spec]:::inprogress
    records[Records]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `workspace.md` | 📄 Leaf Task | 🔄 In Progress |
| `commit_tooling.md` | 📄 Leaf Task | 🔄 In Progress |
| `fixtures.md` | 📄 Leaf Task | 🔄 In Progress |
| `records/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
