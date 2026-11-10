# Revalidation

## Context
Level L3, on the final code.
- `digest_invariance.md` measures the golden and mini imports between `main` and the branch, and carries the milestone's single adversarial digest verifier.
- `equivalence_smoke.md` re-runs M2b's smoke with inputs declared and no `--set`. It spends tokens, so it waits for Eliott's go and his model choice.
- The two write separate reports.

## Goal
The measured digests and the re-run smoke show the milestone additive for the golden import and at parity for the live run.

## Pre-conditions
- [ ] [cmd] The three L2 leaves are merged: `s="$(git log --first-parent --format=%s feat/m2c-wire-alignment)"; for l in input_seam layering_guard env_image_e2e; do printf '%s\n' "$s" | grep -qE "^merge\(.*\): land $l\$" || exit 1; done`

## Success Gates
- ⬜ [nl] Both leaves are `done`, and both reports are committed under `__reports__/m2c-wire-alignment/`

## Status
```mermaid
graph TD
    digest_invariance[Digest invariance]:::planned
    equivalence_smoke[Equivalence smoke]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `digest_invariance.md` | 📄 Leaf Task | ⬜ Planned |
| `equivalence_smoke.md` | 📄 Leaf Task | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
