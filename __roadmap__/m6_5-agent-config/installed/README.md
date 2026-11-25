# Installed

## Context
L2 of M6.5. It runs after `probe_harness_checks.md` (which harness checks are reliable) and `declaration.md` (the settings, the one reader, the refusals). Its three leaves touch three separate crates: the Claude Code driver, the OpenCode driver and core. `install_claude_code` defines the census channel for loaded instruction files, as the reference harness, and `install_opencode` starts once that channel has merged, so the two drivers fill one channel. `preflight_check` runs alongside `install_claude_code`. A seam review of the whole diff follows its close.

## Goal
Each driver installs the global instruction text and hooks where its harness looks, and names the workspace instruction file, which the core mounts read-only at the workspace root. Each run records which instruction files loaded. Preflight refuses a cell that a harness check rejects, or whose workspace file collides with an input.

## Pre-conditions
- [ ] `[cmd]` `git merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds, after L1 has merged
- [ ] `[nl]` The probe report names each harness's check command, or `no` for a harness that has none

## Success Gates
- ⬜ `install_claude_code.md`, `install_opencode.md`, `preflight_check.md` and `proved/` are `done`

## Status
```mermaid
graph TD
    install_claude_code[Install on Claude Code]:::planned
    install_opencode[Install on OpenCode]:::planned
    preflight_check[Preflight check]:::planned
    proved[Proved]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `install_claude_code.md` | 📄 Leaf Task | ⬜ Planned |
| `install_opencode.md` | 📄 Leaf Task | ⬜ Planned |
| `preflight_check.md` | 📄 Leaf Task | ⬜ Planned |
| `proved/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
