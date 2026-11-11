# Design record

**Goal**: R01 §Experiment file, §Run manifest and §Plug-ins record decisions D1–D9. Two lines outside those sections are updated so they do not go stale: the configuration-surface table and the run context's `inputs[]` line. The M2b KT's hand-off of `planner.runtime.*` to M4 is withdrawn.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `test "$(git rev-parse --show-toplevel)" = "$PWD"`
- [ ] [cmd] The leaf branches off the milestone after L0: `git merge-base --is-ancestor feat/m2c-wire-alignment HEAD && git log --first-parent --format=%s feat/m2c-wire-alignment | grep -qx 'merge(isolation): land agent_env'`
- [ ] [nl] OWNED:
  - `__design__/architecture.md`: §Experiment file, §Run manifest, §Plug-ins, the configuration-surface table of §Agent environment and secrets, and the run-context `inputs[]` line of §Evaluator protocol;
  - `__reports__/m2b-execution-parity/04-knowledge_transfer_v0.md`, "For M4" item 3 only.

  No code.
**Success Gates**:
- ⬜ [cmd] G6d: every `provenance.*` path the code writes is described in §Run manifest: `for p in $(git grep -ohE '"provenance\.[a-z_.]+' -- ':(glob)crates/*/src/**' | tr -d '"' | sort -u); do sed -n '/^### Run manifest/,/^### Normalised/p' __design__/architecture.md | grep -qF "$p" || echo "$p"; done` prints nothing
- ⬜ [cmd] §Experiment file names each new field: `s="$(sed -n '/^### Experiment file/,/^### CLI surface/p' __design__/architecture.md)"; for w in inputs audience '`at`' TABLE_VALUED_PATHS prompt.values source isolation.build.prune isolation.record_packages; do printf '%s' "$s" | grep -qF "$w" || echo "$w"; done` prints nothing
- ⬜ [cmd] §Run manifest names the new fields and records: `s="$(sed -n '/^### Run manifest/,/^### Normalised/p' __design__/architecture.md)"; for w in evaluation_inputs '`from`' 'bare sha256' commit; do printf '%s' "$s" | grep -qF "$w" || echo "$w"; done` prints nothing
- ⬜ [cmd] `request.treatment_source` is gone from R01: `! grep -nF 'request.treatment_source' __design__/architecture.md`
- ⬜ [cmd] Only the owned sections moved: `git diff feat/m2c-wire-alignment -- __design__/architecture.md | grep '^@@'` shows hunks inside §Experiment file, §Agent environment and secrets, §Plug-ins, §Evaluator protocol and §Run manifest only (checked by reading the hunk headers against `grep -n '^##' __design__/architecture.md`)
- ⬜ [cmd] SUBJ passes
- ⬜ [nl] The milestone's final `spec-verifier` run finds R01 faithful to the code on the merged branch
**References**: [R10 D1–D9 and "R01 exceptions"](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md); [R02 Meta leaf, Step 1's list of decisions](../../candler-v1/contracts/parity/alignment/m2c_wire_alignment.md); the merged L0 leaves' reports under `__reports__/m2c-wire-alignment/leaves/`, for the exact names the code chose; [AGENTS.md "Source files say what is"](../../../AGENTS.md) — the same register holds for R01's own sections

## Step 1: R01 records the decisions
**Goal**: A reader of R01 learns the new experiment fields, every provenance path with its meaning and whether it gates a stratum, and where an environment's facts are declared, with no history or meta-discourse.
**Implementation Logic**:
1. **§Experiment file:**
   - `[[inputs]] {kind, path, at?, audience}`, with path resolution (I14), the reserved kind and unique kinds;
   - the table-valued paths (`TABLE_VALUED_PATHS`, today `prompt.values`) and the folding rule (D1);
   - `[treatment] {name, tag?, commit?, source?}`, where `source` is a local checkout path relative to the file (D4);
   - the settings `isolation.build.prune` and `isolation.record_packages` (D6).
2. **§Run manifest:**
   - the treatment reference: a live run always carries the commit; an imported one carries none;
   - the input digest (the list digest) and `evaluation_inputs[]` outside the hash (D2, D3);
   - `ValueSource::Derived.from`;
   - the imported snapshot digest is the bare legacy sha256 (D5);
   - a provenance table with every path the code writes (`eval_infra.{commit,dirty,tag,commits_ahead}`, `sandbox_contract`, `packages.<import>.{distribution,version}`): its meaning, its origin (D7) and whether it gates a stratum (D8).
3. **§Plug-ins:** an evaluator's packages are reported through `describe`'s `environment.packages{}`; the agent environment's facts are declared by the experiment; the core records a domain's facts only under names the experiment gives (D6, D9).
4. **The configuration-surface table:** drop `request.treatment_source`; add the two settings and `request.inputs`.
5. **The run-context `inputs[]` line:** it is served from `comparable.inputs` and `evaluation_inputs`.
6. Take every name from the merged code, not from this leaf. Where they differ, report it.
**Deliverables**: `__design__/architecture.md`: the sections listed
**Consistency Checks**: `! grep -nF 'request.treatment_source' __design__/architecture.md && sed -n '/^### Run manifest/,/^### Normalised/p' __design__/architecture.md | grep -qF evaluation_inputs` (expected: PASS)
**Commit**: `docs(design): record the experiment's inputs, sources and provenance`

## Step 2: Withdraw the planner hand-off to M4
**Goal**: The M2b KT no longer hands `planner.runtime.*` to M4.
**Implementation Logic**:
1. In `__reports__/m2b-execution-parity/04-knowledge_transfer_v0.md`, "For M4" item 3, mark the item withdrawn by M2c (amendment A2). A grader's packages are reported through `describe`, and no milestone adds a planner field to the core.
2. Change no other line of the report.
**Deliverables**: `__reports__/m2b-execution-parity/04-knowledge_transfer_v0.md`: item 3 of "For M4"
**Consistency Checks**: `git diff feat/m2c-wire-alignment --stat -- __reports__/m2b-execution-parity/04-knowledge_transfer_v0.md | grep -q '1 file changed'` (expected: PASS)
**Commit**: `reports(experiment): withdraw the planner hand-off to m4`
