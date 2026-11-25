# Readers and serving

## Context
Level 2 of M6d, after `../schema_exit_needs.md` (the wire: `ToolResult.exit`, `Needs.raw`, `Needs.manifest`, `RunContext.manifest`, the digest keys) and `../probe_shell_shapes.md` (real harness fixtures for nonzero exits and timeouts). Four leaves run in parallel: each trace reader fills `exit` from what its harness wrote, `serve_context.md` serves the declared parts through one copy routine, and `helper_context.md` gives Python plug-ins the new fields and a typed trace. `e2e/` follows.

## Goal
Both trace readers record the harness's exit status, `assemble`/`materialise` serve the trace, raw streams and manifest or refuse with a code, and the Python helper exposes them.

## Pre-conditions
- [ ] `[nl]` `../schema_exit_needs.md` and `../probe_shell_shapes.md` are `done` and merged into `feat/run-context`

## Success Gates
- ⬜ `cc_reader_exit.md`, `oc_reader_exit.md`, `serve_context.md`, `helper_context.md` and `e2e/` are `done`
- ⬜ A seam review of the whole `feat/run-context` diff after this level's merges is recorded under `__reports__/run-context/`, and its findings are fixed or ruled on

## Status
```mermaid
graph TD
    cc_reader_exit[Claude Code exit]:::done
    oc_reader_exit[OpenCode exit]:::done
    serve_context[Serve the run]:::done
    helper_context[Helper context]:::inprogress
    e2e[Consumer path]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `cc_reader_exit.md` | 📄 Leaf Task | ✅ Done |
| `oc_reader_exit.md` | 📄 Leaf Task | ✅ Done |
| `serve_context.md` | 📄 Leaf Task | ✅ Done |
| `helper_context.md` | 📄 Leaf Task | 🔄 In Progress |
| `e2e/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
