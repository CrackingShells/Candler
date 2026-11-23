# Analysis outputs verdicts

**Goal**: One verdict record for each of F2 F4 C6 C8 C9 C10 G13 X8: what `analyze`, `show`, `inspect` and evaluation give a reader after a run, judged on whether a reader can misread it and on who owns each number.
**Estimate**: about 40 Sonnet calls, $1.6; at 40 calls, stop, commit what is done, list what is left
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-condition (README, Gotchas): `git merge-base --is-ancestor $BASE HEAD`
- [ ] [nl] OWNED (regex): `^__reports__/m6b-surface-consolidation/leaves/analysis_outputs_v0\.md$`
**Success Gates**:
- ⬜ [cmd] Records: `R=__reports__/m6b-surface-consolidation/leaves/analysis_outputs_v0.md; for id in F2 F4 C6 C8 C9 C10 G13 X8; do test "$(grep -Ec "^### $id: " $R)" = 1 || exit 1; done; test "$(grep -Ec '^- \*\*Verdict:\*\* (accept|defer|m7|owner:(plugin|treatment|provider|consumer)|reject|done)$' $R)" = 8`
- ⬜ [cmd] Covers: `R=__reports__/m6b-surface-consolidation/leaves/analysis_outputs_v0.md; for id in G3 H5 H6 H7 G4; do grep -Eq "^- \*\*Covers:\*\* (.*[ ,])?$id([ ,.;]|$)" $R || exit 1; done`
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] Each `accept` has a design suggestion whose tests answer D3; each `defer` has a Future row with an evidence trigger; each claim about the code cites `path:line`
**References**: [R10](../../__reports__/m6a-paired-stats/knowledge_transfer_v0.md) — F2, F4 (F1 and F3 are not inputs); [R05 §4](../../__reports__/m7-mcp-server/01-findings_agent_consumability_v1.md) — C6, C8, C9, C10; [R06](../../__reports__/m7-mcp-server/00-inventory_gotchas_v0.md) — G3, G4, G13; [R07](../../__reports__/m7-mcp-server/01-input_htn_lane_v0.md) — H5, H6, H7; [R01 §Analysis statistics](../../__design__/architecture.md); `docs/statistics.md:165`

## Step 1: Check the inputs on the tip
**Goal**: Know which asks are live and what problem each one really names.
**Implementation Logic**:
1. For each primary id, read its source and check it against the code with colgrep (`analysis/`, `evaluation/`, `crates/candler/src/commands/{analyze,show,inspect}.rs`). Write `## Live on the tip` as a table `| Id | Live | Evidence |`, citing `path:line`.
2. Question each ask:
   - **F2 (A/B follow recording order) and F4 (reading judged before Holm), with G3.** Is each a wrong number, or a right number a reader can misread? A change to which arm is A, or to when the reading is judged, may move what `analyze` prints for the golden corpus: name the guards and the GOLDEN risk.
   - **C6 and H5 (a summary a reader cannot misread).** H5 asks for effects named by level labels and the Holm decision made visible. Is a summary a new operation, or a rendering of `analyze` and `ps` data? F5 (the text rendering) is judged by `cli_contract` under D1: name the seam.
   - **C8 and H6's spend half.** `cost_usd` is recorded; G4 says a provider cache counts cache hits as paid. Which number is Candler's to sum, and which is the provider's to report correctly?
   - **C9 and H7 (`show <run> --transcript`).** What `show` reads today, and whether rendering a harness's raw stdout belongs to Candler's harness drivers or to the consumer.
   - **C10 and G4 (identical-run flag).** Identical replicates come from a provider cache. Is flagging them Candler's measurement duty (validity of n) or the provider's? R04 records a ruling on the cache: apply it.
   - **G13 (re-grade surprises).** Behaviour to change, or a fact to document?
   - **X8.** `docs/statistics.md:165` says paired tests are not offered; M6a blocked tests. Is the line wrong, and are `analyze`'s field names what R01 says?
**Deliverables**: `__reports__/m6b-surface-consolidation/leaves/analysis_outputs_v0.md`: sections `## Live on the tip` and `## The asks questioned`
**Consistency Checks**: `grep -q '^## Live on the tip' __reports__/m6b-surface-consolidation/leaves/analysis_outputs_v0.md` (expected: PASS)
**Commit**: `reports(api): check the analysis output inputs on the tip`

## Step 2: Write the verdict records
**Goal**: One record per primary id, in the README's record format.
**Implementation Logic**:
1. Under `## Verdicts`, write one record per primary id in the README's record format, with the nine triad cells and the compromise.
2. Covered ids: G3 on F2 and F4; H5 on F2, F4 and C6 (split, with F5 in `cli_contract`); H6 on C8 (spend half only); H7 on C9; G4 on C10.
3. For each `accept`, the design suggestion names the `candler-core` computation, its files, its tests per D3, its size, and whether DIGEST or GOLDEN could move and why. For each `defer`, draft the R01 §Future features row with an evidence trigger.
4. End with `## Seams` (for example C6 and F2 with D1; C8 with `execution_preflight`'s C1).
**Deliverables**: `__reports__/m6b-surface-consolidation/leaves/analysis_outputs_v0.md`: sections `## Verdicts` (8 records) and `## Seams`
**Consistency Checks**: the Records gate above (expected: PASS)
**Commit**: `reports(api): judge the analysis output inputs`
