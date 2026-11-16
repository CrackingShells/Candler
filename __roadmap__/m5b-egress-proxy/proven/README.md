# Proven

## Context
L1 of `m5b-egress-proxy`. It follows the L0 leaves `guard_reuse`, `egress_proxy` and `ground_truth_refusal` and shows them on the product path and on a real daemon: a created-but-unstarted container is destroyed (`unstarted_destroy`), the allowlist holds under attack and its refusal is removed (`egress_attack`), ground truth never reaches a real agent (`ground_truth_e2e`), and R01 records the result (`design_record`). The four leaves own disjoint files. `egress_attack` also owns CI's expected list for the other leaves' e2e names. The coordinator reviews R01's text against this level's tip before merging.

## Goal
The refusal `isolation.egress_unsupported` is gone because enforcement is proven; no container leaks from a run that fails before it runs; I8 holds in a real `candler run`; R01 says what is true.

## Pre-conditions
- [ ] [cmd] L0 is merged: `for l in guard_reuse egress_proxy ground_truth_refusal; do git log --format=%s | grep -q "^merge(isolation): land $l" || exit 1; done`

## Success Gates
- ✅ [cmd] The egress rig and the restored rows pass, and the refusal is gone: `cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*egress_/)' && ! git grep -n egress_unsupported -- ':(glob)crates/*/src/**'`
- ✅ [cmd] I8 holds in a real run: `cargo nextest run -p candler --no-tests=fail --run-ignored only -E 'test(/^guard_cli_e2e::.*i8_/)'`
- ✅ [cmd] No container leaks, and the stop race holds: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_unstarted_destroy::/) | test(/^guard_container_stop::/)'`
- ✅ [cmd] CI's list names the new e2e guards: `for n in egress_allowlist_holds_on_a_real_daemon destroy_verifies_by_one_inspect i8_canary_never_reaches_the_agent; do grep -q "$n" .github/workflows/container-e2e.yml || exit 1; done`
- ✅ [cmd] R01 and the code table agree: `grep -q isolation.ground_truth_exposed docs/contract/codes.toml && grep -q isolation.ground_truth_exposed __design__/architecture.md && ! grep -q egress_unsupported __design__/architecture.md && grep -A4 '"isolation.egress_unsupported"' docs/contract/codes.toml | grep -q '^lifted'` (A2)
- ✅ [nl] Both adversarial verifiers closed with no open finding; DIGEST and GOLDEN are unchanged at the level tip

## Status
```mermaid
graph TD
    unstarted_destroy[Unstarted destroy]:::done
    egress_attack[Egress attack]:::done
    ground_truth_e2e[Ground-truth e2e]:::done
    design_record[Design record]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `unstarted_destroy.md` | 📄 Leaf Task | ✅ Done |
| `egress_attack.md` | 📄 Leaf Task | ✅ Done |
| `ground_truth_e2e.md` | 📄 Leaf Task | ✅ Done |
| `design_record.md` | 📄 Leaf Task | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A2 | 2026-11-17 | L1 coordinator | none; the R01 gate corrected | The level gate demanded `egress_unsupported` absent from `codes.toml`, but the plan lifts the refusal and keeps its row (`lifted = …`), as `egress_attack` and `design_record` require; the gate now checks R01's text is free of it and the row is lifted. |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| `design_record.md` | `m5b/design_record` | merge 6dc5782a; follow-up 0f82e5c4 | R01 records the proxy, the destroy rule, D9/I8 and the residuals; `gateway.upstream_ca` added at the integration review; spec-verify PASS at the L1 tip |
| `unstarted_destroy.md` | `m5b/unstarted_destroy` | merge 2524c8d2 | no container leaks when the session start fails (`unstarted_destroy_session_start`); the lost sweep lists a job's allow networks (`networks_of_job`); P6 answered; report `__reports__/m5b-egress-proxy/leaves/unstarted_destroy_v0.md` |
| `egress_attack.md` | `m5b/egress_attack` | merges 969f3830, d6585666 | rig `egress_allowlist_holds_on_a_real_daemon`; B1 fix: refuse when any `$upstream_addr` entry is refused, `proxy_next_upstream off`; Step 4 lifts `isolation.egress_unsupported`; P1-CI answered (SERVFAIL, no `--dns`); verifier closed; report `__reports__/m5b-egress-proxy/leaves/egress_attack_v0.md` |
| `ground_truth_e2e.md` | `m5b/ground_truth_e2e` | merges 0edc5e11, a416b7af, f4c4f1f1 | `i8_canary_never_reaches_the_agent` on a real agent container; `[gateway] upstream_ca` user setting (`config.upstream_ca_invalid`); the supervisor gets `XDG_CONFIG_HOME`; three Bites red; verifier closed, its Linux path proven only by container-e2e; report `__reports__/m5b-egress-proxy/leaves/ground_truth_e2e_v0.md` |
