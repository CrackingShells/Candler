# Consumers

## Context
Level L1, after the five L0 leaves are merged on `feat/m2c-wire-alignment`. It wires L0's wire types into the core.
- `treatment_source.md` reads the source from the file.
- `declared_inputs.md` resolves, stores, mounts and records inputs.
- `evaluator_inputs.md` serves `needs.inputs` from a manifest; its tests build the manifests themselves.
- `design_record.md` writes R01 from decisions D1–D9 and from L0's paths.
- The four share no file except adjacent hunks in `resolve/layers.rs`, `orchestrate/session.rs` and `runner/exec/cell.rs`, which the coordinator merges.

## Goal
The experiment file's treatment source and inputs reach the cell, the isolation and the evaluator's run context, and R01 says so.

## Pre-conditions
- [ ] [cmd] The five L0 leaves are merged: `s="$(git log --first-parent --format=%s feat/m2c-wire-alignment)"; for l in wire agent_env import_rules eval_infra_origins verdict_gating; do printf '%s\n' "$s" | grep -qE "^merge\(.*\): land $l\$" || exit 1; done`
- [ ] [cmd] STD and G5c pass on `feat/m2c-wire-alignment`

## Success Gates
- ⬜ [nl] The four leaves and `closeout/` are `done`, with no open verifier finding

## Status
```mermaid
graph TD
    treatment_source[Treatment source]:::done
    declared_inputs[Declared inputs]:::done
    evaluator_inputs[Evaluator inputs]:::done
    design_record[Design record]:::done
    closeout[Closeout]:::inprogress
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `treatment_source.md` | 📄 Leaf Task | ✅ Done |
| `declared_inputs.md` | 📄 Leaf Task | ✅ Done |
| `evaluator_inputs.md` | 📄 Leaf Task | ✅ Done |
| `design_record.md` | 📄 Leaf Task | ✅ Done |
| `closeout/` | 📁 Directory | 🔄 In Progress |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| `evaluator_inputs.md` | `m2c/evaluator_inputs` → dab0335 | 2 | `guard_registry`'s old `unsupported` pin replaced by a `build_inputs` refusal row; materialised input dirs stay writable, files 0444 |
| `declared_inputs.md` | `m2c/declared_inputs` → 48b3bef | 4 | P2: one read-only mount per file, stub made first; two fix rounds (empty-file leak, stub exclusion exact, unmountable names); verifier PASS on I8; `job.json` may hold evaluator digests (D3 wording amended) |
| `treatment_source.md` | `m2c/treatment_source` → cc5e497 | 3 | two fix rounds; `from` is the absolute normalised path; `--set` sources resolve against the cwd; moved tag refused naming `treatment`; old setting refused by name |
| `design_record.md` | `m2c/design_record` → 83f3439 | 3 | one fix round against the L1 code; G6d prints nothing; a bare name with a source records no commit (as before M2c) |
