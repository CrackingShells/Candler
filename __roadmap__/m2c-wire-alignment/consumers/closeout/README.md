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
    input_seam[Input seam]:::done
    layering_guard[Layering guard]:::done
    env_image_e2e[Env image e2e]:::done
    revalidation[Revalidation]:::done
    settings_out_of_cell[Settings out of cell]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `input_seam.md` | 📄 Leaf Task | ✅ Done |
| `layering_guard.md` | 📄 Leaf Task | ✅ Done |
| `env_image_e2e.md` | 📄 Leaf Task | ✅ Done |
| `revalidation/` | 📁 Directory | ✅ Done |
| `settings_out_of_cell.md` | 📄 Leaf Task | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A1 | 2026-11-10 | __reports__/m2b-execution-parity/equivalence_smoke_report_v2.md | ["settings_out_of_cell.md"] | P4-F1: settings written in fixed reach the cell digest; the file is now their only route |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| `env_image_e2e.md` | `m2c/env_image_e2e` → 279913b | 2 | real env image with `colorama==0.4.6`; Docker lists equal before and after; CI names the two guards (2e941f9); watch the 180 s cap on a cold runner |
| `input_seam.md` | `m2c/input_seam` → 02b5fd2 | 4 | no seam defect; the CLI test world now seals runs and its guards assert it; a tampered list blob is refused by a new guard |
| `layering_guard.md` | `m2c/layering_guard` → bc1567c | 2 | segment and sequence matching over src, build files, schemas, assets and the helper, names included; three fix rounds; 0 hits; extra renames in L3 (`planner`, `problems`, `arango`) |
| `settings_out_of_cell.md` | `m2c/settings_out_of_cell` → 0510261 | 2 | A1 (P4-F1): file settings leave the cell; a factor that varies a setting, at its path or under a parent table, is refused naming the leaf; three fix rounds after the verifier; no digest moved |
