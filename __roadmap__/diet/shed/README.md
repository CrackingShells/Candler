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
    shed_dead_dup[shed_dead_dup]:::done
    second_sources[second_sources]:::planned
    silent_acceptance[silent_acceptance]:::inprogress
    e2e_audit[e2e_audit]:::done
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
| `shed_dead_dup.md` | 📄 Leaf Task | ✅ Done |
| `second_sources.md` | 📄 Leaf Task | ⬜ Planned |
| `silent_acceptance.md` | 📄 Leaf Task | 🔄 In Progress |
| `e2e_audit.md` | 📄 Leaf Task | ✅ Done |
| `fix/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| `e2e_audit.md` | `diet/e2e_audit` | 4 | merged `ae87ec67`; B1 fresh evaluation and B2 catalog snapshot broken, D1-D5 defects, see `__reports__/diet/01-findings_e2e_audit_v0.md` |
| `shed_dead_dup.md` | `diet/shed_dead_dup` | 3 | merged `6a1aa220`; plan summary removed and digest path folded; walker fold, one docker mapper and the retirement kept with reasons in its leaf report |
| `silent_acceptance.md` | `diet/silent_acceptance` | 4 | merged `32aefe65`; 3 of 6 cases refused; replicates conflict and non-string server command/args need `runner/refusal.rs` and `jobs/refusal.rs`, outside OWNED |
