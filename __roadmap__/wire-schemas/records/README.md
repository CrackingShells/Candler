# Records

## Context
Level 1 of `wire-schemas`: runs after `workspace.md` (primitives, drift harness, lints), `commit_tooling.md` (`cog.toml`) and `fixtures.md` (raw examples). It produces the two wire types with their schemas and tests, and the release and CI configuration; `closeout/` follows.

## Reference Documents
- [R01 Architecture](../../../__design__/architecture.md) — §Wire encodings, §Wire schema: outcome record, §Wire schema: tool-catalog snapshot, §Golden corpus
- [R02 Campaign](../README.md) — gotchas shared by every leaf

## Goal
`OutcomeRecord` and `CatalogSnapshot` exist with generated, drift-checked schemas and three test layers, and release-plz and CI are configured.

## Pre-conditions
- [x] `../workspace.md`, `../commit_tooling.md` and `../fixtures.md` are `done` and merged on `feat/wire-schemas`

## Success Gates
- ⬜ `cargo nextest run -p candler-schemas` passes with both roots registered in `generated_schemas()`
- ⬜ `cargo clippy --workspace --all-targets -- -D warnings` passes
- ⬜ `actionlint .github/workflows/*.yml` passes

## Gotchas
- `outcome_record.md` and `catalog_snapshot.md` both add a `pub mod` line to `lib.rs` and one entry to `generated_schemas()`; the coordinator resolves that conflict at integration.
- Both record leaves get an adversarial verifier and a Grit review before they merge; `release_ci.md` merges on its implementer's report.

## Status
```mermaid
graph TD
    outcome_record[Outcome record]:::inprogress
    catalog_snapshot[Tool-catalog snapshot]:::inprogress
    release_ci[Release tooling and CI workflows]:::inprogress
    closeout[Close-out]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `outcome_record.md` | 📄 Leaf Task | 🔄 In Progress |
| `catalog_snapshot.md` | 📄 Leaf Task | 🔄 In Progress |
| `release_ci.md` | 📄 Leaf Task | 🔄 In Progress |
| `closeout/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
