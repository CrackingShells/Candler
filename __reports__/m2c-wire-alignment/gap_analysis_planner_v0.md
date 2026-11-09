# Gap Analysis: candler-v1 — the core and the binary hold the HTN planner as a built-in concept

Source of amendment A2 in `__roadmap__/candler-v1/contracts/parity/README.md`. A2 adds no node: it adds a Success Gate and a planning item to the leaf `alignment/m2c_wire_alignment.md`, which A1 created and which has not started.

## Problem Statement

Candler is domain-agnostic: a platform for testing LLM-based agent systems, where everything a domain needs reaches it through a plug-in. Main at `v0.3.0` (b2bb782) breaks that rule. The core has a "planner" concept: it probes the env image for a planner library and writes `provenance.planner.sandbox.*` into every run manifest. The `candler` binary also hardcodes the HTN planner's import package, `gtpyhop`. The I1 guard did not catch either of them.

## Evidence

On `origin/main` at b2bb782 (`git grep -n -i planner -- 'crates/*/src'`):

| Where | What |
|:--|:--|
| `crates/candler/src/commands/supervise.rs:40-42, 115` | `const PLANNER_PACKAGE: &str = "gtpyhop"`, passed as `planner_package: Some(...)` |
| `crates/candler-core/src/runner/mod.rs:69`, `jobs/runtime/mod.rs:57`, `jobs/runtime/session.rs:305` | `planner_package: Option<String>` threaded through the runtime |
| `crates/candler-core/src/isolation/image.rs:513-630` | `PlannerAxis`, `PlannerProbe`, `planner_script`, `parse_planner` |
| `crates/candler-core/src/jobs/orchestrate/session.rs:185-260, 383-426` and `orchestrate/mod.rs:88-106` | `JobHost::planner()`, `DockerHost`'s probe, a per-image planner map, `CellRun::with_planner` |
| `crates/candler-core/src/runner/exec/cell.rs:36-67`, `runner/exec/mod.rs:534` | `CellRun.planner` |
| `crates/candler-core/src/runner/evidence/provenance.rs:2, 42-85` | writes `provenance.planner.sandbox.distribution` and `.version` |
| `crates/candler-core/src/manifest/compare.rs:331-334` | the stratum verdict documents the planner library as exempt |
| `crates/candler-core/src/isolation/container/run.rs:3` | module doc: "the planner's entries" |

The guard `crates/candler-core/tests/guard_layering_i1.rs` has an 11-name `DENY_LIST` (`htn`, `gtpyhop`, `tier_d`, …). It scans the core, the harness crate and the schemas, but not the binary crate. It has no generic role names such as `planner`. So the core passes the guard even though it holds a planner axis, and the one literal `gtpyhop` lives in a crate the guard never reads. M2a's rebase onto b2bb782 found the literal when M2a's NEUT change extended the scan to the binary.

## Root Cause

M2b's leaf asked for runs equivalent to today's `htn-creator-eval run-config`, with "same sandbox composition, same provenance fields". The reference's provenance has ten dotted fields, four of them `planner.runtime.*` and `planner.sandbox.*` (`__roadmap__/m2b-execution-parity/feature_map/provenance_fields.txt`). M2b reproduced the fields in Candler's own vocabulary instead of a mechanism a plug-in declares. Nothing in the leaf or in R01 said that the fields belong to the domain:

- R01 names the planner only in prose about the legacy corpus (lines 586, 974).
- R01 does not say how a plug-in asks the core to record a fact about its environment.
- The I1 deny-list holds the legacy names, not the roles those names fill.

The M2b review and the coordinator's review of PR #4 did not catch it either.

## Impact Assessment

- **Scope.**
  - Included:
    - a plug-in declaration of the env-image packages whose distribution and version a run records;
    - recording them under a neutral provenance path;
    - removing the core's planner concept and the binary's constant;
    - the stratum verdict's treatment of a declared package, decided and written in R01;
    - the I1 guard extended to the binary and to role names.
  - Excluded:
    - imported golden runs: they keep their legacy `planner.*` values, which the import rules map and the core never names;
    - M2a's grading, which never reads the field.
- **Dependencies.**
  - The declaration sits in the plug-in or experiment surface M2c already reworks, and the re-validation smoke M2c already runs reads the provenance.
  - M2b's `planner_pin` and `provenance_fields` checks read `.planner.sandbox.*` from a `jq` projection of `manifest.json`. The projection maps the neutral path back to the reference's name, and the checks stay as they are.
  - M2a lands first, with one exact NEUT allowance for `supervise.rs`'s constant; this gate removes that allowance.
  - M3 and M4 already wait on M2c.
- **Risk.**
  - A run's cell digest must not move. The planner axis lives in `provenance`, outside `comparable`, so it should not move, but the change must prove it.
  - A broadened deny-list can flag legitimate words (`plan` in `run_plan`, `ContainerPlan`). Match the role name as a whole word.
  - Deferring the fix leaves v0.3.0's run manifests with a domain-named field. That is acceptable, because no Candler-run corpus exists yet that a reader depends on.

## Proposed Solution

Add to M2c, with no new node:

1. **A Success Gate.** No planner concept in the core or the binary. The env packages a run records are declared by the plug-in and written under a neutral path. I1 scans the binary and denies `planner` as a whole word. M2a's interim allowance is removed. Cell digests are unchanged.
2. **A planning item** in Step 1, for the declaration's shape and home, the provenance path, and whether a declared package's version gates a stratum verdict, all recorded in R01.

## Recommendations

1. M2a merges with its narrow allowance and a comment naming this gate. Main's behaviour does not change before M2c.
2. R01 §Plug-ins and §Run manifest say how a plug-in names facts the core records about its environment. That stops the next parity milestone, M6, from adding a second domain's fields to the core.
3. The reviewers dispatched on 2026-11-09 look for other places where a parity gate let a domain field into Candler's vocabulary. Their findings may add to this gate before M2c's plan is approved.
