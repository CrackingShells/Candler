# Gap Analysis: candler-v1 — the experiment file cannot carry a cell's inputs, table values or treatment source

Source of amendment A1 in `__roadmap__/candler-v1/contracts/parity/README.md` (node `alignment/`, leaf `m2c_wire_alignment.md`).

## Problem Statement

M2b's equivalence smoke found three gaps in the shared `ExperimentFile` wire type (`experiment/1`, `crates/candler-schemas/src/execution.rs`). M1 owns that type, and both M2a and M2b parse it.

- The type cannot declare inputs.
- It flattens nested `[fixed]` tables.
- `TreatmentRef` cannot name a source.

So every input feature of the v5 sandbox contract is built in core but inert, and the smoke experiment reaches parity only through `--set`. No milestone leaf owns the declaration surface. Because the type is shared, M2b cannot fix it alone without a schema change that M2a has not agreed to.

## Evidence

M2b's report `__reports__/m2b-execution-parity/equivalence_smoke_report_v0.md` (on `feat/m2b-execution-parity` at `ae6857c`) recorded five RiVault runs of one cell.

| Finding | Where | What fails |
|:--|:--|:--|
| F5: no input declaration | `ExperimentFile` has no inputs table, so `JobSpec.inputs` is always empty | feature_map S2 and X19 fail (`layout`: no `pyproject.toml`, `scripts/` or server file in the work dir); X32 (input leak) cannot be driven |
| F7: nested `[fixed]` flattened | `flatten_fixed` and `flatten_into` turn `[fixed.prompt.values]` into the dotted keys `prompt.values.domain_name` | `prompt.values` can only arrive as a whole object through `--set` JSON |
| Treatment source | `TreatmentRef {name, tag, commit}` | `request.treatment_source` is passed with `--set` |

To reproduce:

1. Write an experiment file with a `[fixed.prompt.values]` table and run `candler run`. The cell holds dotted leaves, not one `prompt.values` object.
2. There is no key under which a fixture path can be declared, so `CellRun.inputs` stays empty and no read-only mount is made.

## Root Cause

R01 §Experiment file lists `fixed`, `factors`, `replicates`, `budget`, `harness`, `treatment` and `secrets[]`, with no inputs. Yet R01 §Run manifest gives `comparable.inputs[]`, and §Evaluator protocol gives `inputs[] {kind, path, digest}` in the run context. The design names inputs at both ends but has no declaration surface, so the meta-roadmap gave no milestone that surface: M1 built the type as R01 specified it, and M2b built everything downstream of it. The table-value and treatment-source gaps have the same origin. R01 calls a level "a table that fills the values under its path", but says nothing on a value that is itself a table. It names the treatment by tag and commit, and leaves out where the treatment is fetched from.

## Impact Assessment

- **Scope.** Included:
  - an inputs table, resolved to digests in `comparable.inputs[]`;
  - a rule for where flattening stops;
  - a treatment source kept out of the cell digest;
  - the DRIFT bless and the R01 sections;
  - re-validation of S2, X19, X32 and `layout`.

  Excluded: M2b-only defects. M2b is already fixing those under Eliott's go: `container.created` events, provenance fields, run-end measured values, the eval_infra verdict, I14 paths, the resume-refusal detail and the preflight path.
- **Dependencies.**
  - M3 (the lean CLI derives values from the experiment file) and M4 (sessions declare fixtures) both build on the type. M5 mounts inputs into containers. M6's second plug-in needs `needs.inputs[]` to be satisfiable.
  - M2c needs M2a and M2b merged, because both parse the type.
  - M2b's first gate is carved out for S2, X19, X32 and `layout`, which M2c owns.
- **Risk.**
  - A change to `comparable` that moves existing cell digests would break M1's golden-corpus import and M2a's parity fixtures. So the change must be additive, and that is a gate.
  - A treatment source inside the digest would split identical treatments into separate strata (I7).
  - An evaluator-only input that reached the isolation would break I8.

## Proposed Solution

Insert one milestone, M2c, after M2a and M2b and before the surfaces. On disk:

- `parity/alignment/m2c_wire_alignment.md` holds the leaf;
- `parity/surfaces/` moves under `parity/alignment/` so that depth orders it after M2c.

M2c plans its own campaign. Two design constraints are fixed now:

1. `candler-schemas` must not depend on core's `field_paths.toml`. Either the leaf-path declarations move into schemas, or flattening moves into `resolve`.
2. The treatment source is a location, so it is recorded in `sources[]` and never in `comparable.treatment`. A tag is resolved to a commit before the digest is computed.

## Recommendations

1. M2b records S2, X19, X32 and `layout` as owned by M2c in its knowledge-transfer report, and carves them out of its first gate.
2. M2a's knowledge-transfer report lists every `ExperimentFile` path M2a reads, so that M2c can check the new shape against it.
3. Later: the evaluator-side `needs.inputs[]` kinds stay in R01's open list, but M2c's input kinds must have the same shape, so that M6 needs no second schema change.
