# Interlude audit: Candler against the three tests

Date: 2026-11-13, at origin/main aeb2a31 (v0.8.0). Four read-only auditors covered about 72k lines of
source and 67k lines of tests. They built nothing and ran nothing; the CI timings come from real runs.
Every finding cites file:line in that commit. This report ranks the findings and records what this
interlude fixes and what it leaves for later.

## Verdict

The code is correct in most places that were checked, and its parts are well named. Where it fails
the tests, it fails in two recurring ways:

1. **Scan where there should be a lookup.** State is kept as directories of files, and every
   question re-reads the whole directory:
   - every event log, at each job start;
   - every child record, at each guard check;
   - every run manifest, at each plan;
   - every slot file, every 50 ms.

   The costs grow with the history of the store, not with the work asked for.
2. **Each feature grew its own copy.** Shared logic was written again for each new feature:
   - five Docker clients and six container-hardening lists;
   - two hand-written tar readers;
   - eight SHA-256 → `Digest` helpers;
   - three sealing pipelines;
   - four cell-path sources;
   - 13 copies of the run-manifest fixture builder.

   The copies have already started to drift: the pax handling differs, the limits differ, and the
   rule for a failing reader differs.

The CI pipeline pays for the same habit:
- no build cache;
- four compiles in a row;
- the gates re-run when a PR is edited;
- 106 test binaries.

## Findings by test

| Theme | Sustainable | Scalable | Efficient | Ids |
|:--|:--:|:--:|:--:|:--|
| Whole-history scans (logs, records, manifests, slots) | ● | | ● | A2 A4 B1 B2 C6 |
| Docker CLI fan-out and polling where events exist | ● | | ● | A1 A3 A8 A9 C5 |
| Copies that drift (Docker clients, tar, digest, sealing, paths, constants) | | ● | | A5 A10 B4 B5 B11 C2 C3 C7 |
| Trait does not own the harness surface | | ● | | C1 C4 C8 |
| Redundant bytes: copies, re-hashes, extra fsyncs, venv copies | ● | | ● | A7 B6 B7 B8 B12 C9 |
| Correctness found on the way | | | | A6 (newest gateway lines dropped), B9 (possible double count in tallies) |
| CI: no cache, serial steps, re-runs, 106 binaries | ● | | ● | D1 D2 D3 D4 D12 |
| Tests: compile-only, duplicated fixtures, scanners, no roles | | ● | | D5 D8 D9 D10 D11 |

Estimated costs (each auditor's reasoning is in its section of the audit):

| Cost | Size |
|:--|:--|
| Event-log parsing at each job start | O(total bytes of every job log in the store) |
| Child-record JSON reads, 240-run job | ≈115k |
| /proc reads while a job runs on Linux, 500 processes | ≈14k/s |
| Slot-file syscalls during a 240-run regrade | ≈170k/s |
| Stats per plan, 10k-run store and 240 attempts | ≈2.4M |
| Docker spawns per stopped run (8 in parallel) | ≈37 |
| CI critical path per PR | 283–371 s, of which ≈150–200 s is avoidable |

## This interlude fixes

| Batch | Branch | Items |
|:--|:--|:--|
| CI | `ci/fast-gates` | D1 cache, fmt in parallel, no separate build; D2 skip on `edited`, docs-only filter; D12 timeouts; D4 delete the answered probe, build only the e2e binaries; D8 delete the compile-only tests |
| Runtime | `perf/runtime-footprint` | A4 backward read for job liveness; A2 records removed once spent; A6 tail-biased gateway capture with one limit; A7 venv volume only for the agent; B2 in-process semaphore before slot locks; A9 no blocking sleep in async; env key includes the Dockerfile and `.dockerignore`; A3 no /proc sweep for an empty group |
| Store | `perf/store-scans` | B9 probe, then fix if real; B1 one store listing per plan; B3 delete the write-only index; B5 one digest constructor; B7 `fs::copy`; B11 one constants module; B12 one trace copy and sealing off the async workers |

## Left for later, with the reason

| Ids | Why not now |
|:--|:--|
| A5 one Docker client and one control-container builder | L; it is the right home for A1 and A8, so do them together |
| A1 multi-id inspect and guard re-use, A8 `docker create` instead of cidfile polling | Depend on A5; medium risk to guard semantics |
| A3 SIGCHLD-driven reaping | M, medium risk; the cheap part is in the runtime batch |
| C1 C4 C8 the harness trait owns skills, contexts, streams and planning; const constructors | M–L design change to the driver trait; worth doing before a third harness, so it is an input to M5 |
| C2 C3 C7 driver duplication (tar readers, cell paths, versions) | M; overlaps C1, so it lands with it |
| B4 one sealing step for importers | Changes the integrity of legacy imports whose reader fails; needs a ruling first |
| B6 fsync budget, B8 streaming import | Medium risk to crash guards and memory peaks; measure first |
| B10 one Comparable diff | Changes the shape of recorded deviations |
| D3 one integration binary per crate, D5 shared test-support crate | Large mechanical diffs that conflict with every open branch; do them right after the batches merge |
| D4 parallel container e2e | Needs a per-test analysis of sweep scope |
| D6 one Python environment per test run, D7 slow and flaky timing tests | S–M; the flakiness needs root-causing, not retries |
| D9 clippy rules instead of source scanners, D10 one `scripts/check.py`, D11 test roles | Policy changes for the suite; propose at M5 planning |
| C5 static tool catalog, C9 `as_bytes`, C10–C12 schemas for CLI outputs, refusal codes, shared outcome vectors | S–M; good first tasks for the next batch |
| Known from the M4 triage: orphan env image GC, the reproducibility of `isolation.image` | Pending Eliott's decision on comparable identity |

## Steering questions

1. B4: when a legacy import's trace reader fails, should the run be sealed invalid, as raw ingest
   already does? No external users depend on the current behaviour.
2. Should the driver-trait consolidation (C1, C4, C8) become an M5 leaf? Its cost grows with each
   harness added before it.
3. Should D3 and D5, the test-layout consolidation, run as a single mechanical PR right after this
   interlude, while nothing else is open?
