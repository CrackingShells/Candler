# closeout

## Context
L3 of `m3-lean-cli`, the last level. `docs_refusals` needs every code in `codes.toml` (after `refusal_late`); `cli_surface` needs every verb in its final form (after `evaluate_verb`). Their OWNED paths are disjoint. After both merge, the coordinator runs every landing gate of the campaign README and the spec-verifier against the meta leaf.

## Reference Documents
- [R01 Campaign README](../../../README.md) — the landing gates
- [R02 Approved plan](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m3-approved-plan.md) — §5 D- and U-guards, the quick-win map

## Goal
The contract pages list every refusal, and every verb and option passes the CLI-surface audit from an unrelated directory.

## Pre-conditions
- [ ] [cmd] L2 is merged: `! git grep -q -- '--plugin' -- crates/candler/src/commands/evaluate.rs && git grep -q 'jobs\.' docs/contract/codes.toml`
- [ ] [cmd] STD and LINUX pass on the L2 tip

## Success Gates
- ⬜ [cmd] Each leaf's own gates pass on the merged tip, and every landing gate of the campaign README passes
- ⬜ [nl] The `docs_refusals` verifier and the final spec-verifier closed with no open finding

## Gotchas
- `cli_surface` empties the pending lists the scaffold put in `guard_cli_surface.rs`; a verb or option still pending fails G-verbs and U1/U2.

## Status
```mermaid
graph TD
    docs_refusals[Docs refusals]:::planned
    cli_surface[CLI surface]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `docs_refusals.md` | 📄 Leaf Task | ⬜ Planned |
| `cli_surface.md` | 📄 Leaf Task | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
