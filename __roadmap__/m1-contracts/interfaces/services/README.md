# Services

## Context
Level 2. On the L1 contracts it builds the process supervisor and framed raw streams, the run store, the Claude Code trace reader and the legacy mapping, the services the importer joins at L3.

## Goal
Each service lands with its guards green and its required verification closed.

## Pre-conditions
- ✅ [cmd] The L1 leaves are landed: `for l in wire_execution wire_evaluation harness_traits resolve_config; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`
- ✅ [nl] api_review reports no open finding

## Success Gates
- ⬜ [cmd] The four L2 leaves are landed: `for l in supervisor run_store claude_code_reader legacy_mapping; do git log --first-parent --format=%s feat/m1-contracts | grep -qx "merge(.*): land $l" || exit 1; done`
- ⬜ [cmd] STD passes on `feat/m1-contracts`, and `cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings` passes
- ⬜ [nl] The supervisor's required verifier and the claude_code_reader verifier report no open finding, and the level's Grit review is closed

## Status
```mermaid
graph TD
    supervisor[Process supervisor]:::planned
    run_store[Run store]:::planned
    claude_code_reader[Claude Code trace reader]:::planned
    legacy_mapping[Legacy mapping]:::planned
    import[Import]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `supervisor.md` | 📄 Leaf Task | ⬜ Planned |
| `run_store.md` | 📄 Leaf Task | ⬜ Planned |
| `claude_code_reader.md` | 📄 Leaf Task | ⬜ Planned |
| `legacy_mapping.md` | 📄 Leaf Task | ⬜ Planned |
| `import/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
