# M1 Contracts

**Goal**: Every remaining wire type, the harness traits, the Claude Code trace reader, the run store, the process supervisor core and the legacy importer exist, and the golden corpus imports losslessly.
**Pre-conditions**:
- [ ] `../wire_schemas.md` is `done` and merged on `main`
- [ ] Reminder `candler-M1` read: its notes predate the architecture's final version; where they differ, this leaf wins (its "one trivial evaluator end to end" exit moves to M2a, which builds the evaluation engine)
**Success Gates**:
- ⬜ The 240 golden runs import into a Candler store: 10,320 schema-valid outcome records. A test checks each against its `expectations.jsonl` row and its grading JSON's `blocked_by` through a mapping table written in the test itself, independent of the importer's code
- ⬜ All 240 `agent_stream.jsonl.gz` normalise into traces: the expected count of `invalid` runs is 0, and any other count is explained run by run in the import report
- ⬜ The integrity mapping holds: the 56 `incomplete` golden runs are valid, and a synthetic `contaminated` fixture run, built by this milestone, imports as `invalid(input_leak)`
- ⬜ A supervisor test spawns a process tree in its own group, stops it by escalation and finds no survivor; the child record is on disk before the supervisor's spawn API returns to its caller
- ⬜ Every new wire type is generated and drift-checked like the first two
**References**: [R01 §Data Model, §Harness drivers, §Jobs, runs and supervision, §Golden corpus](../../../__design__/architecture.md) — the types, traits, store, supervisor and mapping

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m1-contracts/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`, covering:

- **Wire types** in `candler-schemas`: normalised trace (with `wrapped_calls`); plug-in manifest; suite file; run manifest with the fixed comparability and nuisance sets; experiment file; evaluator messages (`describe` result, `evaluate` params with the run context, notifications); job record and event envelope; budget.
- **`candler-harness`**: the `HarnessDriver` and `TraceReader` traits of R01 §Traits (pre-stop `Action`, `CollectSpec::Command`, framing, several sessions, observed ids, `classify_exit` seeing the sessions).
- **Claude Code trace reader** in `candler-harness-claude-code`, with a fixture corpus drawn from the golden streams (subagent side-chains, denials, hook payloads) and its reader version recorded. The driver comes in M2b.
- **Run store writer** (R01 §Run store): `runs/`, `blobs/`, `tree.json`, one outcome file per request written as `.partial` then renamed, an index that can be rebuilt; run-relative paths only (I17).
- **Process supervisor core** in `candler-core`: spawn into its own process group with the run marker, child records, deadlines, stop escalation (pre-stop action, signal and grace, terminate group, kill group) and framed output capture. It sits here, not in M2a or M2b, because both need it and run in parallel. The stray sweep and the slots stay in M2b.
- **`resolve` skeleton and project config loader** (plug-in directories, slot count, store root): built here so the parallel M2a and M2b each register their preflight steps instead of both writing `resolve`.
- **Legacy importer** (`ingest` of a results folder): `experiment.json` → run manifest; `_run_cache/*/run-N/outputs` → blobs and tree; `agent_stream.jsonl.gz` → raw; grading → outcome records under the imported evaluator version `htn-creator-eval@0.4.2`, instrument identity from `PROVENANCE.json`; integrity verdicts mapped. Replicate = `run-N`, attempt = 1.

Adversarial verification is required for the importer mapping (measurement semantics: every row, text values, `blocked` and `not_measured` causes) and for the supervisor (a leak is silent).
Decide in the plan, and record in `__design__/architecture.md`: the field mapping from `experiment.json` and `PROVENANCE.json` to the run manifest; how legacy cells get a cell digest; how legacy raw streams, which have no receive times, are framed (I3); where the run manifest records an unrecorded value such as the corpus's `null` planner version; the store index format.
Gotchas: the corpus is read from the tag (`git -C <artifact repo> worktree add <dir> candler-m2a-golden`); git dropped 4 empty `outputs/` directories, which must be recreated empty; `planner.sandbox.version` is `null` in the corpus and must be labelled, not guessed.
**Deliverables**: `__roadmap__/m1-contracts/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m1-contracts/` (expected: PASS)
**Commit**: `docs(roadmap): plan the m1-contracts milestone`

## Step 2: Build and land

**Goal**: The milestone campaign is executed and lands on `main`.
**Implementation Logic**:
Execute the campaign on `feat/m1-contracts`, then land it on `main` with one `--no-ff` merge. The Success Gates above must pass on `main` after the merge. If a design decision changed, `__design__/architecture.md` changes in the same merge.
**Deliverables**: the wire types above in `candler-schemas`; `HarnessDriver` and `TraceReader` in `candler-harness`; the Claude Code `TraceReader`; the run store, the process supervisor, the `resolve` skeleton with the project config loader, and the legacy importer in `candler-core`; a golden-import test; the synthetic `contaminated` fixture
**Consistency Checks**: `cargo test --workspace` (expected: PASS)
**Commit**: `merge(importer): land the contracts and the golden-corpus import`

## Step 3: Close out

**Goal**: The milestone is recorded and the next ones can start cold.
**Implementation Logic**:
Write a knowledge-transfer report under `__reports__/m1-contracts/`. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott: `candler-M1` complete; `candler-M2a` and `candler-M2b` startable. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/m1-contracts/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/` (expected: PASS)
**Commit**: `docs(reports): close the m1-contracts milestone`
