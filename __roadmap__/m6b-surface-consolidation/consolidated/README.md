# Consolidated verdicts

## Context
Level L1 of `m6b-surface-consolidation`, after every L0 leaf is merged. `verdicts` folds the six leaf reports into one findings report, `__reports__/m6b-surface-consolidation/01-findings_verdicts_v0.md`, with one row per input id, the seams resolved or raised, the responsibility record draft and the design suggestions the build session will plan from. Its verifier attacks the report before Eliott reads it.

## Goal
One findings report that Eliott can rule on, in which every one of the 63 ids has one verdict and every accepted item has a design suggestion.

## Pre-conditions
- [ ] [cmd] L0 is merged: `for l in execution_preflight knowledge_as_data analysis_outputs cli_contract docs_and_skills codes_review; do test -f __reports__/m6b-surface-consolidation/leaves/${l}_v0.md || exit 1; done`
- [ ] [nl] The coordinator's L0 seam review is recorded in the campaign Progress table, and its gaps are closed

## Success Gates
- ⬜ [cmd] COVER (campaign README) exits 0
- ⬜ [cmd] Campaign gate G2 passes
- ⬜ [nl] The verifier's findings are fixed or adjudicated in `__reports__/m6b-surface-consolidation/leaves/verdicts_v0.md`

## Status
```mermaid
graph TD
    verdicts[Verdicts report]:::done
    build[Build from verdicts v3]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `verdicts.md` | 📄 Leaf Task | ✅ Done |
| `build/` | 📁 Directory | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A1 | 2026-11-23 | __reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md | ["build/"] | Eliott's Go on v3; he asked this session to write the build leaves |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
