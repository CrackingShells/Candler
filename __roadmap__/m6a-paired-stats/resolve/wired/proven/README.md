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
    known_truth[Known truth]:::done
    design_record[Design record]:::done
    golden_design[Golden design]:::done
    import_treatment[Import treatment]:::done
    level_of_prefix[Level-of prefix]:::done
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
| `known_truth.md` | 📄 Leaf Task | ✅ Done |
| `design_record.md` | 📄 Leaf Task | ✅ Done |
| `golden_design.md` | 📄 Leaf Task | ✅ Done |
| `import_treatment.md` | 📄 Leaf Task | ✅ Done |
| `level_of_prefix.md` | 📄 Leaf Task | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A8 | 2026-11-18 | m6a-l3-addendum.md (notes folder of session local_7f577b41), Eliott's approval of the L3 reshape | [ "golden_design", "import_treatment" ] | The 240 golden runs are the trace of a design to rerun, not numbers to refit: golden_blocked is blocked, superseded by golden_design (the design plans 12 cells and analyses against a second engine); import_treatment makes an import declare request.treatment like its native rerun; G2 becomes the golden design gate, G12 the import gate, 6 verifiers; known_truth runs the golden shape end to end; design_record documents the treatment table form and re-reads the offline citations |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| `design_record.md` | m6a/design_record | 2 | merged 56bdc6bb. §Analysis statistics as a ## heading, the treatment table form and the importer rule; every validity source cited (G3). The N3 residuals list wants confirmation. Chung and Davis 1998 caveat on the Mantel–Fleiss criterion recorded. The leaf's `:!__roadmap__` pathspec fails in this git; `:(exclude)` used |
| `golden_design.md` | m6a/golden_design | 3 | merged fffbad28. The golden design (3 models x 4 versions x 20 runs) plans and analyses through Candler; second engine (scipy 1.17.0, statsmodels 0.15.0) agrees (G2). Accepted deviations (A13): fabricated resolutions, partial weighted match, exact fallback not exercised, orientation by recorded key order. Verifier closed |
| `level_of_prefix.md` | m6a/level_of_prefix | 3 | merged e33894e2. The object-under-prefix branch of level_of was unreachable (check_factors refuses a prefix factor) and is deleted (A13); the design record states A and B are the recorded key order (`Comparison.levels`) |
| `import_treatment.md` | m6a/import_treatment | 3 | merged 854c275c. The importer declares request.treatment when typed treatments differ and drops the paths mapping one to one onto it (A11); stratum_verdict applies it to all-imported strata only (A12). 25/25 targeted, golden declared_ row green (G12), DIGEST unchanged. Verifier PASS, low findings in the handover |
| `known_truth.md` | m6a/known_truth | 6 | merged fb691ef1. H0 rates: binary 0.0505, one block 0.0447, tie-free weighted 0.059, tied weighted 0.024 (upper bound only, A14/A15). Simpson blocked 0.051 vs pooled 0.727; power at 0.2 is 0.329 at 12 runs per cell; two levels end to end on fakes (seal `invalid(infrastructure)`, a fakes artefact). Fisher-for-mid-p bite red; weighted constant-p bite not run |
