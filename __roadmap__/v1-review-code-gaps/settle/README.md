# Settle

## Context
Level L1 of `v1-review-code-gaps`, after both L0 fixes are merged on `fix/v1-review-code-gaps`. `local_suppressions.md` decides every local lint suppression, in the code L0 added as well, and adds the lints that keep the decision. `digest_invariance.md` reads the merged tree and writes a report, so the two run in parallel.

## Goal
Every lint suppression in the workspace is removed or justified in line and kept honest by the lints, and a report shows that no cell or context digest moved.

## Pre-conditions
- [ ] [cmd] Both L0 leaves are merged: `s="$(git log --first-parent --format=%s fix/v1-review-code-gaps)" && printf '%s\n' "$s" | grep -qx 'merge(isolation): land the agent container record' && printf '%s\n' "$s" | grep -qx 'merge(importer): land the legacy streams as import sources'`
- [ ] [cmd] STD passes on `fix/v1-review-code-gaps`

## Success Gates
- ⬜ [nl] `local_suppressions.md` and `digest_invariance.md` are `done`, with no open finding from the spec verifier or the Grit reviewer

## Status
```mermaid
graph TD
    local_suppressions[Local suppressions]:::done
    digest_invariance[Digest invariance]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `local_suppressions.md` | 📄 Leaf Task | ✅ Done |
| `digest_invariance.md` | 📄 Leaf Task | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| `digest_invariance.md` | `gaps/digest_invariance` | 1 (b32c6aa), merge de7b096 | No run key, cell digest or context digest moved on the golden (240 runs, 10,320 records) or mini corpus; only `import.sources[]` gained 2 entries per run. Coordinator-reviewed, as it changes no code |
| `local_suppressions.md` | `gaps/local_suppressions` | 4 (ac1afb3, 7566d37, 19c3fa6, a4e7373), merge on the milestone | One fix round taking the Grit reviewer's refactors. 40 `src/` sites: 27 kept as reasoned `#[expect]`, 13 gone. Deviations: `jobs/view.rs` no longer fires (removed); `CliError::Unavailable` was dead (deleted); `allow_attributes` is denied per crate root, not workspace-wide, so `tests/` keeps 12 reasoned `#[allow]`; steps 3-4 are typed `refactor`. The leaf's own grep gate had a vacuous pathspec, corrected here. Follow-ups: merge `FrameMode` into `Framing`; rename `Session::parent_session` |
