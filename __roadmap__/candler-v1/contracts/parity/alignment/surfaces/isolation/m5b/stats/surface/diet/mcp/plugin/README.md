# Second plug-in

## Context
Depth 11, last. The second plug-in (M8, renamed from the old M6) against the core as it stands, in its own repository. M8 adds no core feature: a gap goes to "Future features" in `__design__/architecture.md`, and a blocking gap gets its own core milestone, decided then. Added by amendment A6 of `../../../../../../README.md`; nothing freezes here.

## Goal
A second, unrelated plug-in has exercised the contract, and the gaps it found are on record.

## Pre-conditions
- [ ] `../m7_mcp_server.md` is `done`

## Success Gates
- ⬜ `m8_second_plugin.md` is `done`

## Status
```mermaid
graph TD
    m8_second_plugin[M8 Second plug-in]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `m8_second_plugin.md` | 📄 Leaf Task | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A6 | 2026-11-16 | `__reports__/candler-v1-review/02-findings_amendment_a6_v0.md` | [ "m8_second_plugin" ] | The old `freeze/m6_second_plugin_freeze.md` becomes M8 here, with its freeze and "Deliberately open" gates removed (A5's gate went with them) |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
