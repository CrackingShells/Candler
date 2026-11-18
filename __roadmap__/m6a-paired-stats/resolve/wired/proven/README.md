# Proven

## Context
Level L3 of `m6a-paired-stats`, the last one. It proves the battery built at L0 to L2 (reshaped by A8): `golden_design` shows the HTN golden experiment's design (htn-creator-eval 0.4.2) plans and analyses through Candler against a second engine, `import_treatment` makes an import of the golden runs declare the same factors, `known_truth` simulates through the real `analyze`, and `design_record` writes the design and its citations into `__design__/architecture.md`. The four share no file. `golden_blocked` is blocked, superseded by `golden_design`.

## Goal
The blocked battery is shown correct on real data and on known truth, and its design is recorded with every validity source cited.

## Pre-conditions
- [ ] [cmd] L2 is merged: `for l in level_wiring analyze_output; do test -f __reports__/m6a-paired-stats/leaves/${l}_v0.md || exit 1; done`
- [ ] [cmd] STD, DIGEST and GOLDEN passed at the L2 merge (Progress table)

## Success Gates
- ⬜ [cmd] G2 golden design (A8): the campaign README's G2 command
- ⬜ [cmd] G12 import (A8): the campaign README's G12 command
- ⬜ [cmd] G8: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_known_truth::/)'`
- ⬜ [cmd] G3: the citation loop of the campaign README passes
- ⬜ [cmd] STD, DIGEST, GOLDEN, LINUX and SUBJ pass at the L3 merge

## Status
```mermaid
graph TD
    golden_blocked[Golden blocked]:::blocked
    known_truth[Known truth]:::planned
    design_record[Design record]:::planned
    golden_design[Golden design]:::amendment
    import_treatment[Import treatment]:::amendment
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `golden_blocked.md` | 📄 Leaf Task | 🚫 Blocked |
| `known_truth.md` | 📄 Leaf Task | ⬜ Planned |
| `design_record.md` | 📄 Leaf Task | ⬜ Planned |
| `golden_design.md` | 📄 Leaf Task | 🔵 Amendment |
| `import_treatment.md` | 📄 Leaf Task | 🔵 Amendment |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A8 | 2026-11-18 | m6a-l3-addendum.md (notes folder of session local_7f577b41), Eliott's approval of the L3 reshape | [ "golden_design", "import_treatment" ] | The 240 golden runs are the trace of a design to rerun, not numbers to refit: golden_blocked is blocked, superseded by golden_design (the design plans 12 cells and analyses against a second engine); import_treatment makes an import declare request.treatment like its native rerun; G2 becomes the golden design gate, G12 the import gate, 6 verifiers; known_truth runs the golden shape end to end; design_record documents the treatment table form and re-reads the offline citations |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
