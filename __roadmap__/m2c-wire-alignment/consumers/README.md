# Consumers

## Context
Level L1, after the five L0 leaves are merged on `feat/m2c-wire-alignment`. It wires L0's wire types into the core.
- `treatment_source.md` reads the source from the file.
- `declared_inputs.md` resolves, stores, mounts and records inputs.
- `evaluator_inputs.md` serves `needs.inputs` from a manifest; its tests build the manifests themselves.
- `design_record.md` writes R01 from decisions D1–D9 and from L0's paths.
- The four share no file except adjacent hunks in `resolve/layers.rs`, `orchestrate/session.rs` and `runner/exec/cell.rs`, which the coordinator merges.

## Goal
The experiment file's treatment source and inputs reach the cell, the isolation and the evaluator's run context, and R01 says so.

## Pre-conditions
- [ ] [cmd] The five L0 leaves are merged: `s="$(git log --first-parent --format=%s feat/m2c-wire-alignment)"; for l in wire agent_env import_rules eval_infra_origins verdict_gating; do printf '%s\n' "$s" | grep -qE "^merge\(.*\): land $l\$" || exit 1; done`
- [ ] [cmd] STD and G5c pass on `feat/m2c-wire-alignment`

## Success Gates
- ⬜ [nl] The four leaves and `closeout/` are `done`, with no open verifier finding

## Status
```mermaid
graph TD
    treatment_source[Treatment source]:::planned
    declared_inputs[Declared inputs]:::planned
    evaluator_inputs[Evaluator inputs]:::planned
    design_record[Design record]:::planned
    closeout[Closeout]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `treatment_source.md` | 📄 Leaf Task | ⬜ Planned |
| `declared_inputs.md` | 📄 Leaf Task | ⬜ Planned |
| `evaluator_inputs.md` | 📄 Leaf Task | ⬜ Planned |
| `design_record.md` | 📄 Leaf Task | ⬜ Planned |
| `closeout/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
