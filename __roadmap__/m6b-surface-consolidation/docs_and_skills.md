# Docs and skills verdicts

**Goal**: Answer D2 (README as the store window), draft the responsibility record over the four audiences, and write one verdict record for each of X5 G16 H9 H10 H11 H12 X6 X7 X11 X12.
**Estimate**: about 45 Sonnet calls, $1.8; at 45 calls, stop, commit what is done, list what is left
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-condition (README, Gotchas): `git merge-base --is-ancestor $BASE HEAD`
- [ ] [nl] OWNED (regex): `^__reports__/m6b-surface-consolidation/leaves/docs_and_skills_v0\.md$`
**Success Gates**:
- ⬜ [cmd] Records: `R=__reports__/m6b-surface-consolidation/leaves/docs_and_skills_v0.md; for id in X5 G16 H9 H10 H11 H12 X6 X7 X11 X12; do test "$(grep -Ec "^### $id: " $R)" = 1 || exit 1; done; test "$(grep -Ec '^- \*\*Verdict:\*\* (accept|defer|m7|owner:(plugin|treatment|provider|consumer)|reject|done)$' $R)" = 10`
- ⬜ [cmd] Sections: `R=__reports__/m6b-surface-consolidation/leaves/docs_and_skills_v0.md; grep -q '^## D2' $R && grep -q '^## Responsibility record (draft)' $R`
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] The record draft names the four audiences and the offline rule; X6's record answers D3 first (can fixtures that tests already parse serve as samples?); each claim about a doc or the code cites `path:line`
**References**: README Gotchas, D2 and D3; [R04](../../__reports__/m7-mcp-server/README.md) — docs are web-wide, interactive surfaces are self-contained; [R09](../../__reports__/m7-mcp-server/00-inventory_docs_v0.md) — the doc table and gaps; [R06](../../__reports__/m7-mcp-server/00-inventory_gotchas_v0.md) — G16; [R07 §Keep authoring, §Form](../../__reports__/m7-mcp-server/01-input_htn_lane_v0.md) — H9–H12; `README.md`, `AGENTS.md`, `CLAUDE.md`, `docs/`, `python/candler-evaluator/`; R14 `designing-tests`

## Step 1: Map the documents to the audiences and answer D2
**Goal**: Each consumer-facing document placed in one audience, and D2 answered.
**Implementation Logic**:
1. Write `## Documents today`: one row per document a consumer or developer reads (`| Document | Audience | What it holds | Generated or hand-written | Drift test |`). The audiences are the README's four: store window, web-wide, interactive, development.
2. Write `## D2`: judge `README.md` against the store-window role (what Candler is, its main features, a quick start, where to go for more). Say what it lacks and what it holds that belongs to another audience. `AGENTS.md` and `CLAUDE.md` stay development-level.
3. Write `## Responsibility record (draft)`: the text proposed for R01. For each audience, what serves it, what it may and may not hold, and its source of truth (generated or hand-written). State the rule that interactive surfaces (CLI, MCP, skills) are self-contained, work offline and never link to docs. Do not decide M7's MCP shape: say only what the rule requires of it.
**Deliverables**: `__reports__/m6b-surface-consolidation/leaves/docs_and_skills_v0.md`: sections `## Documents today`, `## D2`, `## Responsibility record (draft)`
**Consistency Checks**: the Sections gate above (expected: PASS)
**Commit**: `reports(readme): map the documents to their audiences`

## Step 2: Write the verdict records
**Goal**: One record per primary id, in the README's record format.
**Implementation Logic**:
1. Under `## Verdicts`, write one record per primary id in the README's record format, with the nine triad cells and the compromise.
2. Question each ask:
   - **X5** refers to `## D2`.
   - **G16 (docs lag).** Which docs are hand-written copies of facts the code owns? The idiom is a generated page under a drift test; a hand-written copy is a second source.
   - **H9, H10, H11 (experiment-design guidance, the environment checklist, gap-ownership notes).** Skill content is M7's to shape (`m7`), but some of it may be web-wide docs: say which, by the audiences table.
   - **H12 (form: CLI with a stable `--json`, MCP only for state).** Defers to D1 for the CLI half and to M7 for the MCP half: name the seam.
   - **X6 (sample files).** Apply D3 first: list the fixtures that tests already parse (`golden_design/experiment.toml` and others) and whether a sample page can point to or render them, so no new test is needed. Only if not, design the test with R14.
   - **X7 (`candler.toml` page), X11 (the Python plug-in API), X12 (glossary).** Generated from code or hand-written? Which audience?
**Deliverables**: `__reports__/m6b-surface-consolidation/leaves/docs_and_skills_v0.md`: sections `## Verdicts` (10 records) and `## Seams`
**Consistency Checks**: the Records gate above (expected: PASS)
**Commit**: `reports(readme): judge the docs and skills inputs`
