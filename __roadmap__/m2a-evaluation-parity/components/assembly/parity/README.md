# Parity

## Context
Level L3 of M2a, the milestone's proof. It runs after `assembly/` has landed. A single ignored corpus guard regrades the 240 golden runs live with `htn-creator-eval@0.5.0`. It then diffs the live rows against the imported `0.4.2` rows and against a second live regrade, and checks the rates and the instrument identity against the reference oracle.

## Goal
GP1–GP3 pass locally against `$GOLDEN`, and an adversarial verifier confirms that their constants come from the oracle files, that their bites reproduce, and that their vacuity counts cannot be gamed.

## Pre-conditions
- [ ] [cmd] `for l in engine surfaces helper_release; do git log --first-parent --format=%s feat/m2a-evaluation-parity | grep -q ": land $l\$" || exit 1; done`
- [ ] [cmd] `git -C "$RW" log --first-parent --format=%s feat/candler-htn-plugin | grep -qx 'merge(htn-eval): land htn_plugin'`
- [ ] [cmd] `test "$(find "$GOLDEN" -name 'eval-*-grading.json' | wc -l | tr -d ' ')" -gt 0`

## Success Gates
- ⬜ [cmd] `CANDLER_GOLDEN_CORPUS="$GOLDEN" cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'binary(guard_golden_parity)'` passes, and its log is in the leaf report
- ⬜ [cmd] `cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'binary(guard_golden_parity)'` fails when `CANDLER_GOLDEN_CORPUS` is unset
- ⬜ [nl] The `golden_parity` adversarial verifier holds no open BLOCKER or MAJOR. The L2+L3 Grit review is closed.

## Status
```mermaid
graph TD
    golden_parity[Golden parity: the live regrade guard]:::done
    throughput[Throughput: grade at machine speed]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `golden_parity.md` | 📄 Leaf Task | ✅ Done |
| `throughput/` | 📁 Directory | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A1 | 2026-11-09 | `__reports__/m2a-evaluation-parity/throughput/00-regrade_profile_v0.md` | `throughput/`, `throughput/regrade_throughput.md`, `throughput/htn_preload.md`, `throughput/proof/`, `throughput/proof/throughput_proof.md` | The corpus regrade ran at 13 items/s (one request per item, per-fork imports, F_FULLFSYNC); Eliott judged it unsustainable and authorised the fix |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
