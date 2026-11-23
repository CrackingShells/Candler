# Execution and preflight verdicts

**Goal**: One verdict record for each of C1 C2 C4 C5 G2 G5 G6 G10 G14 G15 G21 X2, judged on what `resolve`, the runner, isolation and jobs do on the tip, with Eliott's container ruling applied to C2.
**Estimate**: about 45 Sonnet calls, $1.8; at 45 calls, stop, commit what is done, list what is left
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-condition (README, Gotchas): `git merge-base --is-ancestor $BASE HEAD`
- [ ] [nl] OWNED (regex): `^__reports__/m6b-surface-consolidation/leaves/execution_preflight_v0\.md$`
**Success Gates**:
- ⬜ [cmd] Records: `R=__reports__/m6b-surface-consolidation/leaves/execution_preflight_v0.md; for id in C1 C2 C4 C5 G2 G5 G6 G10 G14 G15 G21 X2; do test "$(grep -Ec "^### $id: " $R)" = 1 || exit 1; done; test "$(grep -Ec '^- \*\*Verdict:\*\* (accept|defer|m7|owner:(plugin|treatment|provider|consumer)|reject|done)$' $R)" = 12`
- ⬜ [cmd] Covers: `R=__reports__/m6b-surface-consolidation/leaves/execution_preflight_v0.md; for id in H1 H2 H6 H8 C15; do grep -Eq "^- \*\*Covers:\*\* (.*[ ,])?$id([ ,.;]|$)" $R || exit 1; done`
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] Each `accept` has a design suggestion whose tests answer D3; each `defer` has a Future row with an evidence trigger; each claim about the code cites `path:line`
**References**: [R05 §4](../../__reports__/m7-mcp-server/01-findings_agent_consumability_v1.md) — C1, C2, C4, C5, C15; [R06](../../__reports__/m7-mcp-server/00-inventory_gotchas_v0.md) — G2, G5, G6, G10, G14, G15; [R07](../../__reports__/m7-mcp-server/01-input_htn_lane_v0.md) — H1, H2, H6, H8; [R04](../../__reports__/m7-mcp-server/README.md) — the container ruling; [R01 §Operation API, §Jobs, runs and supervision, §Future features](../../__design__/architecture.md)

## Step 1: Check the inputs on the tip
**Goal**: Know which asks are live and what problem each one really names.
**Implementation Logic**:
1. For each primary id, read its source and check it against the code with colgrep (`paths` under `crates/`). Write `## Live on the tip` as a table `| Id | Live | Evidence |`, citing `path:line`.
2. Question each ask before judging its value:
   - **C2 and H1 (the agent-world probe).** Eliott ruled that the container follows from the experiment files and parameters; a box that differs from them is a Candler defect for Candler's own guards and preflight. List what ties the built container to its files today (I8 `i8_canary_never_reaches_the_agent`, image identity in `isolation/image.rs`, the rendered `.mcp.json`, census) and which mismatches nothing catches. The verdict judges that gap, not a probe verb.
   - **C1 and H6's estimate half.** R01 says the experiment is never called a plan. Check what `resolve` already computes (cells, reuse, drift) and whether cost and wall time are derivations of resolve's output rather than a new verb. H6's spend half belongs to `analysis_outputs` (C8): judge only the estimate.
   - **C4 and H2 (`resolve --explain`), C5 (validate without Docker).** What `resolve` prints and records today, which layer sets each value (I12 says each value records its source), and whether the Docker preflight is separable.
   - **G5 and G6.** A missing `.claude/skills/<name>` and a `/private/tmp` bind mount fail at run time with a code that names no cause. Could resolve refuse them earlier with a code of their own? Name the `codes.toml` rows in the blast radius; `codes_review` judges the registry, so name, do not decide, any new row.
   - **G21, C15 and H8 (Docker lock, `gc`).** I16 (no daemon); the machine-wide flock already used for slot pools; R01 §Future features rows on image content identity and orphaned env images.
   - **X2.** `run` detaches while `evaluate` blocks: is the difference designed (R01 §Jobs) or accidental?
   - **G2, G10, G14, G15.** What is recorded per stratum; the fixed-value traps in the experiment file; the detached supervisor; the isolation facts. Which of these are Candler behaviour to change, and which are facts a consumer should be told?
**Deliverables**: `__reports__/m6b-surface-consolidation/leaves/execution_preflight_v0.md`: sections `## Live on the tip` and `## The asks questioned`
**Consistency Checks**: `grep -q '^## Live on the tip' __reports__/m6b-surface-consolidation/leaves/execution_preflight_v0.md` (expected: PASS)
**Commit**: `reports(api): check the execution and preflight inputs on the tip`

## Step 2: Write the verdict records
**Goal**: One record per primary id, in the README's record format.
**Implementation Logic**:
1. Under `## Verdicts`, write one record per primary id in the README's record format (Gotchas, Verdict record), with the nine triad cells and the compromise.
2. Name the covered ids on their primary's `Covers:` line: H1 on C2, H2 on C4, H6 on C1 (estimate half only), H8 and C15 on G21.
3. For each `accept`, the design suggestion names the operation in `candler-core` that computes it (thin surfaces), its files, its tests per D3, and its size. For each `defer`, draft the R01 §Future features row with an evidence trigger.
4. End with `## Seams`: each place a record depends on a sibling leaf's verdict (for example G5 and G6 on `codes_review`, C1 on `analysis_outputs`' C8). Name them; do not decide them.
**Deliverables**: `__reports__/m6b-surface-consolidation/leaves/execution_preflight_v0.md`: sections `## Verdicts` (12 records) and `## Seams`
**Consistency Checks**: the Records gate above (expected: PASS)
**Commit**: `reports(api): judge the execution and preflight inputs`
