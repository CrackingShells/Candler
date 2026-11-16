# Seams

## Context
Level L2, after `client/`. It holds three leaves:
- `guard_reuse` (A1): the destroy guard verifies by one multi-id inspect;
- `container_create` (A8): `docker create`, the record, then `start -a`;
- `plugin_environment` (D4): a plug-in's agent environment as a fragment of layered values,
  proven by a live MCP server inside a real container.
`egress/` needs `container_create`'s create with two networks and `plugin_environment`'s
appended egress.
**Amendment A8:** `guard_reuse` moved to M5b and `egress/` with it; M5a's seams are `container_create` and `plugin_environment`. They stay listed here with their statuses.

## Reference Documents
- [R01 Architecture](../../../../__design__/architecture.md) — §Process supervision (Destroying containers), §Plug-in manifest
- [R06 Approved plan](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md) — D4, D7, D8, the race rules
- [R07 HTN agent-runtime needs](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/htn-agent-runtime-needs-for-m5.md) — N1–N3, N6

## Goal
A stopped run costs a handful of docker spawns, not about 37. The agent's record exists before its container starts, with no cidfile. A plug-in environment gives the agent live MCP servers, their dependencies and inputs, through layered values the core already reads.

## Pre-conditions
- [ ] [cmd] `client/` merged: `git log --format=%s 18142ec..feat/m5-container-isolation | grep -q '^merge(isolation): land docker_client' && git log --format=%s 18142ec..feat/m5-container-isolation | grep -q '^merge(isolation): land egress_setting'`
- [ ] [cmd] DIGEST and GOLDEN pass on the milestone tip

## Success Gates
- ⬜ [cmd] The three leaves' gates pass on the milestone tip after their merges; STD, LINUX, DIGEST and E2E pass
- ⬜ [nl] The one L2 verifier on the merged diff of `guard_reuse` and `container_create` closed with no open finding, and `plugin_environment`'s verifier closed with no open finding
- ⬜ [nl] The coordination session's ruling on `harness_baseline/argv.json` is recorded in the Progress table
- ⬜ [cmd] The milestone branch is pushed and CI's `container-e2e` job is green on it

## Gotchas
- **Race rules:**
  - `guard_reuse` owns `container/{guard,sweeper,stop}.rs` and keeps the `pub(crate)` signatures of
    `ContainerGuard`;
  - `container_create` owns `container/{run,mod,exec}.rs`, `runner/exec/mod.rs`, the cidfile code
    in `gateway.rs`, `harness_baseline/argv.json`, `guard_harness_facts.rs`, `guard_session.rs`,
    and the existing files and Dockerfile of `assets/docker/testagent/`;
  - `plugin_environment` owns `resolve/steps/plugin_environment.rs` (new), `resolve/steps/image.rs`,
    the manifest read in the evaluation registry, and its fixture plug-in. It does not edit
    `assets/docker/`.
- **Step order.** `plugin_environment`'s step registers before `egress_setting`'s parse step. The
  coordinator checks the order in `resolve/steps/mod.rs` at the merge.
- **The spawn budget** counts destroy-path verbs only (ls, inspect, stop, rm), so A8's extra spawn
  does not move it.
- **Builds.** At most two of the three leaves build at once.

## Status
```mermaid
graph TD
    guard_reuse[Guard reuse]:::planned
    container_create[Container create]:::planned
    plugin_environment[Plug-in environment]:::planned
    egress[Egress]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `guard_reuse.md` | 📄 Leaf Task | ⬜ Planned |
| `container_create.md` | 📄 Leaf Task | ⬜ Planned |
| `plugin_environment.md` | 📄 Leaf Task | ⬜ Planned |
| `egress/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
