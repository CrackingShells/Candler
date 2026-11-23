# Shed and audit

## Context
Runs on the merged L0 tip, where S1 is fixed and the suite is cut. Three code leaves remove what the plan proves dead, duplicated, spelled twice or silently accepted; `second_sources` and `silent_acceptance` both touch `resolve/layers.rs`, and the second to merge rebases. `e2e_audit` runs beside them, changes no product code, and judges each README claim from real RiVault runs.

## Goal
Candler loses its dead and duplicated machinery, refuses what it used to ignore, and has a verdict per README claim from stored bytes.

## Pre-conditions
- [ ] `[nl]` Every L0 leaf merged into `feat/diet` and the L0 gates passed

## Success Gates
- ✅ The four leaves merged
- ✅ `__reports__/diet/01-findings_e2e_audit_v0.md` has a verdict and evidence per claim
- ✅ Workspace gates pass; DIGEST and GOLDEN unchanged
- ✅ Spend for the level within its caps (about $15)

## Status
```mermaid
graph TD
    shed_dead_dup[shed_dead_dup]:::planned
    second_sources[second_sources]:::planned
    silent_acceptance[silent_acceptance]:::planned
    e2e_audit[e2e_audit]:::planned
    fix[Audit fixes]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `shed_dead_dup.md` | 📄 Leaf Task | ⬜ Planned |
| `second_sources.md` | 📄 Leaf Task | ⬜ Planned |
| `silent_acceptance.md` | 📄 Leaf Task | ⬜ Planned |
| `e2e_audit.md` | 📄 Leaf Task | ⬜ Planned |
| `fix/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
