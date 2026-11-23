# Knowledge transfer

**Goal**: The build closes with every standard gate green on the integration tip, a knowledge-transfer report that holds the HTN lane's migration note and the spend against the cap, and nothing left behind (worktrees, branches, colgrep indexes); the supervising session gets one note it can relay at merge.
**Estimate**: about 20 coordinator calls ($1.3), cap 30; at 30 calls, stop, commit what is done, list what is left
**Pre-conditions**:
- [x] [cmd] Every L2 leaf is merged: `for l in analysis_record docs_chores experiment_env; do test -f __reports__/m6b-surface-consolidation/build/${l}_v0.md || exit 1; done`
- [x] [nl] OWNED (regex): `^__reports__/m6b-surface-consolidation/(03-knowledge_transfer_build_v0|README)\.md$|^__roadmap__/m6b-surface-consolidation/.*$`. The roadmap's tool-managed sections change through `dirtree-rdm` only. This leaf is the coordinator's own: no subagent, no verifier, no code
**Success Gates**:
- ✅ [cmd] The build README's `[cmd]` Success Gates pass on the integration tip (STD, LINUX, SUBJ, DIGEST, GOLDEN, no `remedy` key, every leaf report)
- ✅ [cmd] The report exists with its migration note and its spend: `f=__reports__/m6b-surface-consolidation/03-knowledge_transfer_build_v0.md; grep -q '^## Migration' $f && grep -q '^## Spend' $f && grep -q -- '--format text' $f`
- ✅ [cmd] Nothing is left behind: `test -z "$(git worktree list | grep 'm6b-build-')" && test -z "$(git branch --list 'claude/m6b-build-*')"`
- ✅ [cmd] `bash $HOME/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.3.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m6b-surface-consolidation/` passes, and every build node is `done` or `blocked`
- ✅ [nl] The build README's `[nl]` gates hold: every accepted id built or carrying a ruled reason, the supervisor's conditions on `output_contract`, every verifier closed
**References**: [R16 §Design suggestions (the supervisor's conditions: the migration note)](../../../../../../../__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md); the nine leaf reports under `__reports__/m6b-surface-consolidation/build/`; the `writing-reports` skill (knowledge-transfer type); [R11 Cost report v2](../../../../../../../__reports__/agent-token-costs/03-findings_campaign_costs_v2.md) and `__reports__/agent-token-costs/scripts/spend.py`

## Step 1: Run the closing gates
**Goal**: The integration tip is shown green before anything is written about it.
**Implementation Logic**:
1. Run STD (workspace form), LINUX, SUBJ, DIGEST and GOLDEN from the build README's Standard gates, then the build README's other `[cmd]` gates; run long commands in the background and wait for their exit.
2. A red gate stops the close: name the leaf whose change it traces to, and send it to that leaf's context (the leaf's implementer if still addressable, else a fresh Sonnet fix agent with the leaf file) rather than fixing it here.
3. Tick the build README's gates that passed (prose sections only).
**Deliverables**: `__roadmap__/m6b-surface-consolidation/consolidated/build/README.md`: the ticked Success Gates
**Consistency Checks**: `cog check $BUILD_BASE..HEAD` (expected: PASS)
**Commit**: `rdm(api): tick the m6b build gates`

## Step 2: The knowledge-transfer report
**Goal**: The next milestone and the HTN lane learn what changed and what it cost.
**Implementation Logic**:
1. With the `writing-reports` skill (knowledge-transfer type), write `03-knowledge_transfer_build_v0.md`: what each leaf built, by id, with its merge commit; what went well and what hurt; each leaf's follow-ups gathered in one list, each marked for a Future row, a later milestone or nobody. The report also carries an open observation that is not a follow-up of any leaf: `candler-core::it guard_protocol_client::every_failure_path_of_an_evaluator_is_accounted_for`, row `late.sh`, fails only under the full `cargo nextest run --workspace` load. Its stub sleeps 0.9 s inside a 1.9 s window (a 400 ms deadline plus a 1.5 s cancel grace), and under load the run took 3.15 s, so the supervisor stopped the stub. It passes alone. It was first seen failing at the L0 close on `8872d38a`, and it passed in the full run on `0cbd72ff`. No build commit touches it. The supervising session asked that it be recorded so it gets fixed.
2. `## Migration`, for the HTN lane's coordinator, short and self-contained: each verb's default before and after (JSON on stdout and stderr), each retired flag (`--error-format`, `watch --json`, `resolve --json`) and its replacement (`--format text` for text), the inner refusal codes that replace `*.document_invalid`, the renamed `analyze` keys (`interval_reading`, the comparability key), the new `env` table and its refused names, the server start check, and the release it ships in (the next release after the supervisor's PR merges).
3. `## Spend`: measure with `python3 __reports__/agent-token-costs/scripts/spend.py <project transcripts dir> <session id>` for each level's coordinator and their subagents; compare with the build README's budget table, row by row.
**Deliverables**: `__reports__/m6b-surface-consolidation/03-knowledge_transfer_build_v0.md`: sections `## Migration`, `## Spend` and the skill's knowledge-transfer sections
**Consistency Checks**: `f=__reports__/m6b-surface-consolidation/03-knowledge_transfer_build_v0.md; grep -q '^## Migration' $f && grep -q '^## Spend' $f` (expected: PASS)
**Commit**: `reports(api): add the m6b build knowledge transfer`

## Step 3: Clean up and hand over
**Goal**: The machine is as the build found it, and the supervisor has what it needs for the PR.
**Implementation Logic**:
1. Add a build round to `__reports__/m6b-surface-consolidation/README.md` (the leaf reports and this report, with dates).
2. Clear every colgrep index the build created (`list_indexes`, then `index_clear` on each leaf worktree's index); remove any leaf worktree and branch still present (`git worktree remove`, `git branch -d`); delete scratch roots and any Docker container or image a leaf built, by id. Never prune Docker.
3. Set the build nodes to `done` with `dirtree-rdm update` (batched), write the Progress rows, and run `validate`.
4. Leave a note for the supervising session in the shared notes folder: the integration tip, the report's path, the migration note to relay to the HTN lane at merge, and the open follow-ups. Nothing is pushed.
**Deliverables**: `__reports__/m6b-surface-consolidation/README.md`: the build round; `__roadmap__/m6b-surface-consolidation/consolidated/build/`: statuses and Progress rows (through `dirtree-rdm`); the note in the shared notes folder
**Consistency Checks**: `test -z "$(git worktree list | grep 'm6b-build-')" && bash $HOME/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.3.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m6b-surface-consolidation/` (expected: PASS)
**Commit**: `rdm(api): close the m6b build`
