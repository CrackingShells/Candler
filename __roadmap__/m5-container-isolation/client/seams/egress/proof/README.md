# Proof

## Context
Level L4, the last. It holds two leaves:
- `ground_truth_e2e`: proves I8 in a real container at every point of a run;
- `design_record`: writes the milestone's decisions into R01.
Then the coordinator opens the PR to `main`.

## Reference Documents
- [R01 Architecture](../../../../../../__design__/architecture.md) — every section `design_record` changes
- [R06 Approved plan](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md) — the R01 change table, the attack lists

## Goal
The canary of an evaluator-only input is absent from the agent's container and from every live surface at each checkpoint of a real run. R01 describes what M5 built.

## Pre-conditions
- [ ] [cmd] `egress/` merged: `for l in egress_proxy ground_truth_refusal; do git log --format=%s 18142ec..feat/m5-container-isolation | grep -q "^merge(isolation): land $l" || exit 1; done`
- [ ] [cmd] DIGEST, GOLDEN and E2E pass on the milestone tip

## Success Gates
- ⬜ [cmd] Both leaves' gates pass on the milestone tip after their merges; every gate of the campaign README passes
- ⬜ [nl] `ground_truth_e2e`'s adversarial verifier closed with no open finding

## Gotchas
- `ground_truth_e2e` adds new test and testagent files only.
- `design_record` edits `__design__/architecture.md` only. It reads the final names from the merged
  tip, not from the plan.

## Status
```mermaid
graph TD
    ground_truth_e2e[Ground-truth e2e]:::planned
    design_record[Design record]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `ground_truth_e2e.md` | 📄 Leaf Task | ⬜ Planned |
| `design_record.md` | 📄 Leaf Task | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
