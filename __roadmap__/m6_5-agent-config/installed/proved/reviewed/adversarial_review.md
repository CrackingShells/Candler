# Adversarial review

**Goal**: A Sonnet reviewer, who did not build M6.5, reads the milestone's whole diff against the consumer interface, and the closing report says what holds and what does not. (P1)
**Estimate**: Sonnet, $4, cap $8. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use one slot of `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, and the leaf removes it when it merges. The leaf deletes the containers it starts.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds
**Success Gates**:
- ⬜ `__reports__/agent_config/02-review_m6_5_v0.md` names each gate G1–G9 with the evidence that it holds on the tip and failed on `8d82f63b`, or names it as broken.
- ⬜ The report lists any debt left behind (a second reader, a duplicated helper, dead code, a leftover script), each fixed in this leaf or filed as a notice. An out-of-scope finding is filed as an observation and escalated.
- ⬜ `__reports__/agent_config/03-knowledge_transfer_m6_5_v0.md` records the spend measured per leaf against the $75 cap.
**References**: [Architecture report](../../../../../__reports__/agent_config/00-architecture_agent_config_v0.md) — the shapes, the error model, the invariants and §Execution model

## Step 1: Review and close
**Goal**: An independent verdict, and the milestone's record.
**Implementation Logic**:
The review is read-only. A finding is fixed here only if it is in M6.5's scope and small, at most two rounds. Anything else is filed and escalated. Then write the knowledge transfer report, and update the Reminders status.
**Deliverables**: the two reports; `__reports__/agent_config/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.3.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m6_5-agent-config` (expected: PASS)
**Commit**: `reports(api): close M6.5 agent configuration`
