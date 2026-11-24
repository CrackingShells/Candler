# Causes

## Context
Level 1 of M6c Integrity, after `../conventions.md`. Four independent concepts run in parallel, each in its own worktree: P1 gives sealed runs one test producer, starting with a probe whose run times set P7's caps; P2 gives collect an oracle outside the driver; P4 gives each consumer input one reader, as one chain B, A1, A2 in one worktree so that every commit builds; P6 fixes `kill` off a TTY (audit-5). Where two of them touch one file, merge order settles it. `checkpoint/` follows this level.

## Goal
The causes D1 and D5 are removed where S1, B1 and the silent acceptances came from, each shown by an observation that fails on the base.

## Pre-conditions
- [x] `[cmd]` `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.3.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh status __roadmap__/integrity/conventions.md` prints `done`

## Success Gates
- ⬜ `sealed_run_producer.md`, `collect_oracle.md`, `one_reader.md` and `kill_off_tty.md` are `done`, each with its observation seen failing on the base
- ⬜ `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass on the merged level
- ⬜ `checkpoint/` is `done`

## Status
```mermaid
graph TD
    sealed_run_producer[Sealed run producer]:::done
    collect_oracle[Collect oracle]:::done
    one_reader[One reader]:::done
    kill_off_tty[Kill off a TTY]:::done
    checkpoint[Seam review 1 and checkpoint]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `sealed_run_producer.md` | 📄 Leaf Task | ✅ Done |
| `collect_oracle.md` | 📄 Leaf Task | ✅ Done |
| `one_reader.md` | 📄 Leaf Task | ✅ Done |
| `kill_off_tty.md` | 📄 Leaf Task | ✅ Done |
| `checkpoint/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
