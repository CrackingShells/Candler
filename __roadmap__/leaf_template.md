# Leaf template

A leaf is a dirtree task file. Copy this shape; each convention below carries the reason it exists.
The reasons are the evidence of the M6c Integrity campaign (cycle plan v2 §D3).

```markdown
# <Concept>

**Goal**: <the concept this leaf owns, as a behaviour a consumer can see> (<priority>)
**Estimate**: <model>, $<n>, cap $<2n>. At the cap, stop, commit what is green, and list what is left
in the leaf report. Builds use the target dir the campaign README names; the leaf removes it at its merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor <integration branch> HEAD` succeeds
**Success Gates**:
- ⬜ <an observation: what a consumer sees on the tip, and what they saw on the base>
**References**: [<doc>](<path>) — <what it settles>

## Step 1: <title>
**Goal**: <one sentence>
**Implementation Logic**: <how, in prose>
**Deliverables**: <files and artifacts>
**Consistency Checks**: `<command>` (expected: PASS)
**Commit**: `<type>(<scope>): <subject>`
```

## Conventions

1. **A leaf owns a concept and touches the files the concept needs.** A cause that spans two file
   sets has no owner when leaves are cut by file, and a gate over one leaf's files cannot see it.
   Two leaves run in parallel only when their concepts are independent; where they share a file,
   merge order settles it, not an ownership regex.
2. **Each gate names a behaviour a consumer can see, and is shown failing on the base before it
   passes on the tip.** A gate that passes on the base proves nothing. Counts (tests, lines) may
   appear as evidence and are never the gate: a suite can grow or shrink while passing the same
   count.
3. **A gate whose meaning rests on ignored tests runs them, or says where they ran.** A gate that
   skips is a gate that fails: run with `--run-ignored`, or name the pass that ran them with the
   input they need set, and require the output to list them as passed, not skipped.
4. **A seam review on the whole diff follows each merge level.** The reviewer gets the milestone's
   diff so far and the consumer interface, not one leaf, because defects live where two leaves meet.
5. **A roadmap merge that touches what the container path depends on waits for a green
   `container-e2e`; a merge that touches none of it says so.** The check runs the real container
   path in CI, and it starts only on pull requests that touch those paths (the `paths` list of
   `.github/workflows/container-e2e.yml`), so a merge that needed it and went without it has not
   met that path, and a merge that never triggers it records why it did not wait.
6. **Each leaf states its estimate, its cap as a stop action, the target dir the campaign README
   names, and their cleanup.** The cap says what to do on reaching it (stop, commit what is green,
   list what is left), so overrun ends work instead of growing it. Builds on a shared machine use a
   dir per leaf, removed at the merge, so the disk is given back.
