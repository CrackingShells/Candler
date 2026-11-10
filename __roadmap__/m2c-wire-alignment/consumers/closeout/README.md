# Closeout

## Context
Level L2, after `consumers/` is merged.
- `input_seam.md` proves end to end that what `declared_inputs` writes is what `evaluator_inputs` reads, and that I8 holds through the binary.
- `layering_guard.md` rebuilds the I1 guard now that every domain name is gone.
- `env_image_e2e.md` builds the real env image that `agent_env` changed.
- The three share no file. `revalidation/` measures the final code.

## Goal
The input seam, the domain neutrality and the real env image each have a guard that was seen red.

## Pre-conditions
- [ ] [cmd] The four L1 leaves are merged: `s="$(git log --first-parent --format=%s feat/m2c-wire-alignment)"; for l in treatment_source declared_inputs evaluator_inputs design_record; do printf '%s\n' "$s" | grep -qE "^merge\(.*\): land $l\$" || exit 1; done`
- [ ] [cmd] STD passes on `feat/m2c-wire-alignment`

## Success Gates
- ⬜ [nl] The three leaves and `revalidation/` are `done`, with no open verifier finding

## Status
```mermaid
graph TD
    input_seam[Input seam]:::planned
    layering_guard[Layering guard]:::planned
    env_image_e2e[Env image e2e]:::planned
    revalidation[Revalidation]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `input_seam.md` | 📄 Leaf Task | ⬜ Planned |
| `layering_guard.md` | 📄 Leaf Task | ⬜ Planned |
| `env_image_e2e.md` | 📄 Leaf Task | ⬜ Planned |
| `revalidation/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
