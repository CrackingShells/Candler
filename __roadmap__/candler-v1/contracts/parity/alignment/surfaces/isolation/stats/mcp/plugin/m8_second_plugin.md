# M8 Second plug-in

**Goal**: A second, unrelated plug-in, `waiting-on-processes`, is built in its own repository against the core as it stands and delivers a gap report. M8 adds no core feature.
**Pre-conditions**:
- [ ] `../m7_mcp_server.md` is `done`, and M5 and M6a before it
- [ ] Reminder `candler-M8` read (plug-in parts, deterministic grading)
- [ ] R05's test definition read: `00-test_definition_v0.md` beside the R05 design
**Success Gates**:
- ⬜ The `waiting-on-processes` plug-in grades its fixtures as correct, false success or stray process, deterministically (I5)
- ⬜ The plug-in lives in its own repository and builds against the core's published interfaces, with no change to `candler-core`, `candler-harness` or `candler-schemas` made for it
- ⬜ A gap report lists every point where the plug-in needed something the core lacks, each entered as a "Future features" item in `__design__/architecture.md` with its trigger; a blocking gap is named as such and proposed as a core milestone of its own
**References**: [R01 §Plug-ins, §Future features, §Contracts & Invariants (I5)](../../../../../../../../../../__design__/architecture.md) — the contract the plug-in builds against and where its gaps are listed

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m8-second-plugin/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`, covering: the `waiting-on-processes` plug-in (passive fixtures, a server stub, scheduler shims, an outcome schedule, the outcome grader) in its own repository; the agent skills from M7, which the plan uses to author the plug-in and drive its runs where they fit; the gap report. Grading stays deterministic; probabilistic evaluators stay descriptive only (I5). Rule: M8 adds no core feature. A gap goes to the future-features list, and a blocking gap gets its own core milestone, decided then. If the plug-in cannot be built without a core change, the plan stops and reports the gap instead of adding the feature.
Adversarial verification is required for the gap report: attack every workaround the plug-in used, to find the gaps it hid.
**Deliverables**: `__roadmap__/m8-second-plugin/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m8-second-plugin/` (expected: PASS)
**Commit**: `rdm(repo): plan the m8-second-plugin milestone`

## Step 2: Build and land

**Goal**: The plug-in is built and its gap report lands on `main`.
**Implementation Logic**:
Build the plug-in in its own repository. Land the gap report and the "Future features" entries it produces in this repository with one `--no-ff` merge of `feat/m8-second-plugin` on `main`. The Success Gates above must pass. Publishing the plug-in is Eliott's call.
**Deliverables**: the `waiting-on-processes` plug-in repository; the gap report under `__reports__/m8-second-plugin/`; the new "Future features" items in `__design__/architecture.md`
**Consistency Checks**: `cargo nextest run --workspace` (expected: PASS)
**Commit**: `merge(design): land the second plug-in's gap report`

## Step 3: Close out

**Goal**: The campaign is complete.
**Implementation Logic**:
Write a knowledge-transfer report under `__reports__/m8-second-plugin/`. Mark this leaf `done` with `dirtree-rdm update`, and check the campaign README's Success Gates. Propose the reminder reconciliation to Eliott: `candler-M8` complete, and a new core milestone for each blocking gap. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/m8-second-plugin/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/stats/mcp/plugin/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/alignment/surfaces/isolation/stats/mcp/plugin/` (expected: PASS)
**Commit**: `reports(repo): close the m8-second-plugin milestone`
