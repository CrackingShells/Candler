# surfaces

## Context
L2 of `m3-lean-cli`. It needs L1 merged: `evaluate_verb` wires the job body of `evaluate_body` into the command, and `refusal_late` codes every enum of `jobs`, `evaluation` and `import`, including the ones `evaluate_body` and `ingest_inspect` added. Their OWNED paths are disjoint.

## Reference Documents
- [R01 Campaign README](../../README.md) — gotchas, shared constraints, standard gates
- [R02 Approved plan](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m3-approved-plan.md) — D6, §5 E- and R-guards, the verifier table

## Goal
`candler evaluate` is a supervised job that waits for its report, and every author-facing refusal of the workspace carries a code.

## Pre-conditions
- [ ] [cmd] L1 is merged: `test -f crates/candler-core/src/jobs/runtime/evaluate.rs && test -f crates/candler/src/commands/analyze.rs && test -f crates/candler/src/commands/inspect.rs && ! git grep -q 'm3 scaffold' -- crates/candler/src/commands/analyze.rs`
- [ ] [cmd] STD and LINUX pass on the L1 tip

## Success Gates
- ⬜ [cmd] Each leaf's own gates pass on the merged tip; STD, LINUX, LINUXRUN (E3) and G-digest pass after the last L2 merge
- ⬜ [nl] The `evaluate_verb` and `refusal_late` verifiers closed with no open finding

## Gotchas
- `evaluate_verb`'s verifier covers `evaluate_body` too; `refusal_late`'s covers `refusal_core` too.
- If M4 has merged to `main` before this level closes, `refusal_late` also codes M4's new refusal variants (R09's rule: the second to merge codes the other's).

## Status
```mermaid
graph TD
    evaluate_verb[Evaluate verb]:::planned
    refusal_late[Refusal late]:::done
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
| `evaluate_verb.md` | 📄 Leaf Task | ⬜ Planned |
| `refusal_late.md` | 📄 Leaf Task | ✅ Done |
| `closeout/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
