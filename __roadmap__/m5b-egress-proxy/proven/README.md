# Proven

## Context
L1 of `m5b-egress-proxy`. It follows the L0 leaves `guard_reuse`, `egress_proxy` and `ground_truth_refusal` and shows them on the product path and on a real daemon: a created-but-unstarted container is destroyed (`unstarted_destroy`), the allowlist holds under attack and its refusal is removed (`egress_attack`), ground truth never reaches a real agent (`ground_truth_e2e`), and R01 records the result (`design_record`). The four leaves own disjoint files. `egress_attack` also owns CI's expected list for the other leaves' e2e names. The coordinator reviews R01's text against this level's tip before merging.

## Goal
The refusal `isolation.egress_unsupported` is gone because enforcement is proven; no container leaks from a run that fails before it runs; I8 holds in a real `candler run`; R01 says what is true.

## Pre-conditions
- [ ] [cmd] L0 is merged: `for l in guard_reuse egress_proxy ground_truth_refusal; do git log --format=%s | grep -q "^merge(isolation): land $l" || exit 1; done`

## Success Gates
- ⬜ [cmd] The egress rig and the restored rows pass, and the refusal is gone: `cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*egress_/)' && ! git grep -n egress_unsupported -- ':(glob)crates/*/src/**'`
- ⬜ [cmd] I8 holds in a real run: `cargo nextest run -p candler --no-tests=fail --run-ignored only -E 'test(/^guard_cli_e2e::.*i8_/)'`
- ⬜ [cmd] No container leaks, and the stop race holds: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_unstarted_destroy::/) | test(/^guard_container_stop::/)'`
- ⬜ [cmd] CI's list names the new e2e guards: `for n in egress_allowlist_holds_on_a_real_daemon destroy_verifies_by_one_inspect i8_canary_never_reaches_the_agent; do grep -q "$n" .github/workflows/container-e2e.yml || exit 1; done`
- ⬜ [cmd] R01 and the code table agree: `grep -q isolation.ground_truth_exposed docs/contract/codes.toml __design__/architecture.md && ! grep -q egress_unsupported docs/contract/codes.toml __design__/architecture.md`
- ⬜ [nl] Both adversarial verifiers closed with no open finding; DIGEST and GOLDEN are unchanged at the level tip

## Status
```mermaid
graph TD
    unstarted_destroy[Unstarted destroy]:::planned
    egress_attack[Egress attack]:::planned
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
| `unstarted_destroy.md` | 📄 Leaf Task | ⬜ Planned |
| `egress_attack.md` | 📄 Leaf Task | ⬜ Planned |
| `ground_truth_e2e.md` | 📄 Leaf Task | ⬜ Planned |
| `design_record.md` | 📄 Leaf Task | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
