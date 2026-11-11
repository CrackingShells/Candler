# verbs

## Context
L1 of `m3-lean-cli`. It builds on L0: the `Refusal` trait and `codes.toml` sections from `contract_wire`, the statistics from `stats`, the sweep fix from `stray_sweep_tick`, and the platform inventory from `platform_seam`. It holds seven leaves with disjoint OWNED paths: two are doc-only, and five build. Dispatch order under the cap of 3 builders is `refusal_core`, `evaluate_body`, `analyze`, then `observe`, `ingest_inspect` and `docs_frame` as slots free; `design_record` needs no build slot. It produces the verbs `resolve`, `ls`, `analyze`, `ingest` and `inspect`, the evaluate job body, the core refusal codes, the contract-page frame and R01's record of D1–D10.

## Reference Documents
- [R01 Campaign README](../README.md) — gotchas, shared constraints, standard gates
- [R02 Approved plan](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m3-approved-plan.md) — §2 decisions, §4 races, §5 guards

## Goal
Every new verb exists and works, `evaluate` has a supervised job body, the core's author-facing refusals carry codes, and the contract pages have their frame.

## Pre-conditions
- [ ] [cmd] L0 is merged: `git grep -q 'fn started_no_earlier' -- crates/candler-core/src/supervise/signal.rs && test -f crates/candler-schemas/src/refusal.rs && test -d crates/candler-core/src/analysis && test -f crates/candler-core/tests/vectors/platform_inventory.toml`
- [ ] [cmd] `feat/m3-lean-cli` holds both early PRs' content, merged locally while #17 and #18 wait for review (D1): `git merge-base --is-ancestor refs/heads/m3/scaffold feat/m3-lean-cli && git merge-base --is-ancestor refs/heads/m3/stray_sweep_tick feat/m3-lean-cli`. Once their merge commits are on `main`, `main` is merged into `feat/m3-lean-cli` (a `merge(repo)` commit, no rebase), and the coordination session is told before it happens
- [ ] [cmd] STD and LINUX pass on the L0 tip

## Success Gates
- ⬜ [cmd] Each leaf's own gates pass on the merged tip; STD, LINUX and G-digest pass after the last L1 merge
- ⬜ [nl] The coordinator reviewed the seams: the enums the L1 leaves add against R1 and `refusal_late`'s scope, `analyze` against `stats`' types, `docs_frame`'s Fields against `contract_wire`'s classes

## Gotchas
- `refusal_core` lands first among the builders: its `CliError::refused_by` calls in `commands/common.rs` are the only shared hunk at this level.
- No L1 leaf codes the enums of `jobs`, `evaluation` or `import`; those are `refusal_late`'s (L2), so the exhaustive matches never meet a sibling's new variant mid-level.
- `design_record` writes R01 from the approved decisions, not from code; its claims about names are re-checked by `cli_surface`'s G-verbs and by the final spec-verifier.

## Status
```mermaid
graph TD
    refusal_core[Refusal core]:::done
    evaluate_body[Evaluate body]:::done
    analyze[Analyze]:::done
    observe[Observe]:::done
    ingest_inspect[Ingest and inspect]:::done
    docs_frame[Docs frame]:::done
    design_record[Design record]:::done
    surfaces[Surfaces]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `refusal_core.md` | 📄 Leaf Task | ✅ Done |
| `evaluate_body.md` | 📄 Leaf Task | ✅ Done |
| `analyze.md` | 📄 Leaf Task | ✅ Done |
| `observe.md` | 📄 Leaf Task | ✅ Done |
| `ingest_inspect.md` | 📄 Leaf Task | ✅ Done |
| `docs_frame.md` | 📄 Leaf Task | ✅ Done |
| `design_record.md` | 📄 Leaf Task | ✅ Done |
| `surfaces/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
