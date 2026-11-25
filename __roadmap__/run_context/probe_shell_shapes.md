# Probe: shell shapes

**Goal**: Probe P-shell. The readers' rules for a failing shell call rest on what the real harnesses write, recorded on Candler's own rig, not on what they are expected to write. Three questions, each with the decision it feeds: (1) what does OpenCode 2.2.7 write for a `shell` call that exits nonzero (the OpenCode reader rule; no fixture exists today); (2) does either harness give a code when its own timeout ends a shell call (if neither does, `exit` stays absent and no timeout guard is written); (3) does Claude Code's `Exit code N` prefix hold when the command writes only to stderr (the Claude Code reader rule) (critical path)
**Estimate**: Sonnet, $6, cap $12. At the cap, stop, commit the fixtures already recorded, and list the questions left open in the leaf report. Builds use the target dir the campaign README names; the leaf removes it at its merge, and removes every image and container it built, by id.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/run-context HEAD` succeeds
- [ ] `[cmd]` `docker info` answers (start it with `docker desktop start` if it is down; never stop it, never prune)
**Success Gates**:
- ⬜ The leaf report answers each of the three questions with the raw line that shows it, quoted from a recorded stream, and names the reader rule each answer implies
- ⬜ For each harness, the streams of three calls are committed as reader fixtures made by the real harness: a nonzero exit, a stderr-only nonzero exit, and a call ended by the harness's timeout. Each fixture has its `PROVENANCE.toml` entry (harness version, image, date, the fake-model script). If a harness cannot be made to time a call out within the cap, the report says so and the decision of question (2) is "absent"
- ⬜ The probe's test code is deleted before merge: only the fixtures, their provenance and the report land
- The leaf touches `crates/**` (fixtures only); `container-e2e` is green on the pull request
**References**: [plan §What the trace already holds, §Probe](../../__reports__/run-context/00-plan_run_context_v0.md). Code:
- `crates/candler/tests/it/guard_cli_e2e/parity_e2e.rs`: the rig (both real harnesses, the production gateway, a scripted fake model);
- `crates/candler-harness-claude-code/tests/fixtures/streams/` and `crates/candler-harness-opencode/tests/fixtures/streams/` with their `PROVENANCE.toml`: where reader fixtures live and how their provenance is written.

## Step 1: Record the shapes
**Goal**: Each harness's stream for the three calls is on disk.
**Implementation Logic**:
On the `parity_e2e.rs` rig, script the fake model to make three `shell`/`Bash` calls in one turn: `exit 3`; `echo oops >&2; exit 2`; and a call that outlives the harness's own shell timeout (set the shortest timeout the harness allows through its configuration, as the rig already configures each harness). Run it once on each harness. Copy the raw stream (Claude Code `stream-json`; OpenCode `stdout.jsonl` and `export.json`) into a fixture directory per harness, named for what it shows, and write each `PROVENANCE.toml` entry. Answer the three questions in the report from those lines only.
**Deliverables**: two fixture directories and their provenance entries; the probe's scratch test (deleted in Step 2)
**Consistency Checks**: `ls` of both fixture directories shows the stream files (expected: PASS)
**Commit**: `test(harness): record each harness's failing and timed-out shell call`

## Step 2: Delete the probe, report
**Goal**: Only the answer and the fixtures stay.
**Implementation Logic**:
Remove the scratch test and anything it added to the rig. Remove the images and containers the runs built, by id. Write the report: each question, the line that answers it, the rule it implies for `readers/cc_reader_exit.md` and `readers/oc_reader_exit.md`. A fresh Sonnet verifier gets the leaf, the report and the fixtures and checks each quoted line against the fixture file.
**Deliverables**: `__reports__/run-context/leaves/probe_shell_shapes_v0.md`
**Consistency Checks**: `git diff feat/run-context --stat -- crates/candler/tests` prints nothing (expected: PASS)
**Commit**: `reports(harness): answer how each harness reports a failing shell call`
