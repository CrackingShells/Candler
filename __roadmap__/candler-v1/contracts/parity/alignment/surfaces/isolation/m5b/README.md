# M5b Egress proxy and ground-truth guard

## Context
Depth 6. M5b, split from M5 by amendment A8 of `../README.md`: it follows M5a (`../m5_container_isolation.md`) and holds the rest of container isolation, planned with a fresh budget and step-sized leaves. `stats/` (M6a, M7, M8) sits one level below it. Its plan is a campaign `__roadmap__/m5b-egress-proxy/` that M5b's own session writes.

## Goal
An agent's container reaches only its listed hosts and the model gateway, a real container is shown never to hold ground truth, and the destroy guard re-uses its verification.

## Pre-conditions
- [x] `../m5_container_isolation.md` (M5a) is `done` (PR #36, 3e654ef)
- [x] A plan for M5b, with step-sized leaves and a budget, is approved by Eliott (2026-11-16: `__roadmap__/m5b-egress-proxy/`, cap $52; build started on his go)

## Success Gates
- ✅ The egress proxy enforces the allowlist and the `isolation.egress_unsupported` refusal is removed (D1, D2) (PR #38, merge 1e6dc91: G1; CI container-e2e run 44008540106)
- ✅ Ground truth is refused at resolve and absent from the agent container at every point of a real run (D9, I8), merged with its e2e (PR #38: G2, `i8_canary_never_reaches_the_agent` in CI's list)
- ✅ The destroy guard verifies by one multi-id inspect and re-uses the facts (A1) (PR #38: G3, `destroy_verifies_by_one_inspect`)
- ✅ The rest of the design record is in `__design__/architecture.md`, and the live smoke with `isolation.egress` is recorded (PR #38: G6; live smoke job 01a1e780-5310-7c3a-9718-216900be5694 in `__reports__/m5b-egress-proxy/live_smoke_v1.md`)
- ✅ The live smoke runs a plug-in environment through the product path, not a hand-built plan: `DockerIsolation::plan`, the image step's merge, the runner's input mounts and the `mcp.*` rendering, and the agent lists the server's tools (M5a's `plugin_environment_serves_live_mcp_tools` builds these by hand) (PR #38: G7, same job; F2 guard `a_plugins_mcp_servers_reach_claude_code`)
- ✅ A container created but not recorded is destroyed, not left to the label sweep (with A1) (PR #38: G4, `guard_unstarted_destroy`)
- ⬜ `stats/` is `done`

## Status
```mermaid
graph TD
    stats[Paired statistics]:::inprogress
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `stats/` | 📁 Directory | 🔄 In Progress |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A8 | 2026-11-16 | `__reports__/candler-v1-review/04-findings_amendment_a8_v0.md` | [ "m5b" ] | M5 is split after its $60 cap was reached at about $56 with the rest forecast at $57-67 more: M5a (this directory's `m5_container_isolation.md`) keeps the Docker client, the egress cell value (refused until the proxy lands), `docker create` and plug-in environments; M5b (new `m5b/`, one level below M5a) wraps `stats/` and takes the guard reuse (A1), the egress proxy, the I8 refusal and e2e, and the live smoke; the order is M5a, M5b, M6a, M7, M8 |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
