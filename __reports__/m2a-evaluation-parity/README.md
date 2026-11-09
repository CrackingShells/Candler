# m2a-evaluation-parity reports

## Round 00
- [00-architecture_v0.md](00-architecture_v0.md) — latest. Decisions 1–13 for the A–C scope move: the quirk table, the kind signature, the generic diff with its equivalence and rates formats, the file rule, the skeleton seams and the Phase 0 environment.
- [htnstd_baseline.txt](htnstd_baseline.txt) — the 83 HTN-repo tests already failing or erroring at `28be2c15`; HTNSTD fails only on an id outside this list.

## Round 01
- [01-test_definition_v0.md](01-test_definition_v0.md) — latest. Risks R1–R10, the 31 cornerstone guards with their oracle and bite, the dropped list, the promotion rule, the fixture budget and the verification policy.

## Round 02
- [02-knowledge_transfer_v0.md](02-knowledge_transfer_v0.md) — latest. The parity and throughput results, the pain points and their root causes, the follow-ups (import throughput, KP4 test 2, the sync gaps, Tier D and others), what M2c needs from M2a, and the per-cell gate-rate table beside the published tier means.

Per-leaf reports are in `<leaf>/` subfolders, the rulings in [`l1/00-rulings_v0.md`](l1/00-rulings_v0.md), and the throughput record in [`throughput/`](throughput/00-regrade_profile_v0.md).

## Status
Every node of the campaign [`__roadmap__/m2a-evaluation-parity/`](../../__roadmap__/m2a-evaluation-parity/README.md) is done. The live regrade with `htn-creator-eval@0.5.1` matches the 0.4.2 import on all 8,880 A–C rows (209 excused, 0 changed). What remains is the coordinator checklist: the rebase onto main, the HTN re-pin, Linux clippy, the design-doc check and the PR.
