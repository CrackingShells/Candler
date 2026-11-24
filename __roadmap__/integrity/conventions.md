# Conventions

**Goal**: `AGENTS.md` says what sane work means in Candler, and the leaf template carries D3's conventions, so the next milestone is cut by concept and gated by observations (P0).
**Estimate**: Sonnet, $1, cap $2. At the cap, stop, commit what is green, and list what is left in the leaf report.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/integrity HEAD` succeeds
- [x] `[nl]` The `AGENTS.md` paragraph is approved by Eliott as written in v2 §D4
**Success Gates**:
- ⬜ The section "Sustainable, scalable, efficient" of `AGENTS.md` is replaced by "What sane work means in Candler", word for word as in v2 §D4, and nothing else in `AGENTS.md` changes except the link below
- ⬜ `__roadmap__/leaf_template.md` holds the leaf template and its conventions: a leaf owns a concept and touches the files the concept needs; each gate names a behaviour a consumer can see and is shown failing on the base before it passes on the tip, with counts only as evidence; a gate whose meaning rests on ignored tests runs them or says where they ran; a seam review on the whole diff follows each merge level; a roadmap merge waits for a green `container-e2e`; each leaf states its estimate and its cap as a stop action, the shared target dir and its cleanup
- ⬜ `AGENTS.md` §Where things are written down links the template
- ⬜ A fresh reader given the template finds no leaf of `__roadmap__/integrity/` that breaks it, or the leaf is fixed in the same pull request
**References**: [Cycle plan v2 §D3, §D4](../../__reports__/diet/04-architecture_cycle_plan_v2.md) — the conventions and the approved paragraph; [AGENTS.md](../../AGENTS.md) — the section it replaces

## Step 1: Replace the principle section
**Goal**: `AGENTS.md` states what sane work means for a tool that records what agents did.
**Implementation Logic**:
Replace the section "Sustainable, scalable, efficient" (its three bullets, three questions and list of failures) with the approved paragraph under the heading "What sane work means in Candler". The machine-wide principle stays in the global file; the new text neither restates nor paraphrases it. Copy the paragraph from v2 §D4 exactly; do not edit it.
**Deliverables**: `AGENTS.md` (the section "What sane work means in Candler")
**Consistency Checks**: `grep -c 'What sane work means in Candler' AGENTS.md` prints 1 and `grep -c 'Sustainable, scalable, efficient' AGENTS.md` prints 0 (expected: PASS)
**Commit**: `docs(contributing): say what sane work means in Candler`

## Step 2: Write the leaf template
**Goal**: The next campaign's author starts from leaves cut by concept with observation gates.
**Implementation Logic**:
Write `__roadmap__/leaf_template.md`: the dirtree leaf shape (Goal, Estimate, Pre-conditions, Success Gates, References, Steps with their five fields) annotated with the conventions of v2 §D3, each with one line on why. `__roadmap__/` keeps no `README.md`, so the template is a plain file there. Link it from `AGENTS.md` §Where things are written down. Then hand a fresh Sonnet reader the template and the leaves of `__roadmap__/integrity/`, and fix any leaf it shows breaking the template.
**Deliverables**: `__roadmap__/leaf_template.md`; `AGENTS.md` (§Where things are written down); `__reports__/diet/leaves/conventions_v0.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.3.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/integrity/` (expected: PASS)
**Commit**: `docs(contributing): add the roadmap leaf template`
