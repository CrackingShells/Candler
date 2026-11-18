# Design record

**Goal**: `__design__/architecture.md` gains §Analysis statistics: the question, the design facts, the comparisons rule, the battery with every validity source cited, the readings, Holm, N3's residuals and the Future rows; docs that still describe Fisher's test are corrected (G3).
**Estimate**: about 50 Sonnet calls, $2.1; no verifier (docs only; G3 and SUBJ are `[cmd]`)
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (campaign README, Gotchas); L2 is merged
- [ ] [nl] OWNED (regex): `^__design__/.*\.md$|^docs/.*\.md$|^README\.md$|^__reports__/m6a-paired-stats/leaves/design_record_v0\.md$`. Docs only: no code file
**Success Gates**:
- ⬜ [cmd] G3: `for k in 10.1002/sim.3531 Mantel Fleiss Robins Phipson Efron; do sed -n '/^## Analysis statistics/,/^## /p' __design__/architecture.md | grep -q "$k" || exit 1; done`
- ⬜ [cmd] No doc still names Fisher's test as what `analyze` runs: `git grep -n -i 'fisher' -- '*.md' ':!__roadmap__' ':!__reports__'` lists only the mid-p's definition and the history line
- ⬜ [cmd] SUBJ passes; ownership check prints nothing
**References**: [R03 §Statistical design, §Treatment arms](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6a-approved-plan.md); the L0 leaves' citation probes in `__reports__/m6a-paired-stats/leaves/`

## Step 1: Write §Analysis statistics
**Goal**: The design is recorded where readers look for it.
**Implementation Logic**:
1. Sections: question and estimand; design facts (unit, no pairing, fixed margin, blocks); comparisons rule; the battery table; readings ("effect", "inconclusive", never "no difference"); Holm per comparison and role; known truth and the golden check.
2. Citations, from the L0 probes: Lydersen, Fagerland & Laake 2009 (doi:10.1002/sim.3531); Mantel & Haenszel 1959; Mantel & Fleiss 1980; Robins, Breslow & Greenland 1986; Breslow & Day 1980; Phipson & Smyth 2010; Efron 1987.
3. N3's residuals; Future rows: paired designs with McNemar, a declared margin read with TOST, `lock_split`.
**Deliverables**: `__design__/architecture.md`: §Analysis statistics
**Consistency Checks**: the G3 loop above (expected: PASS)
**Commit**: `docs(design): record the analysis statistics and their sources`

## Step 2: Correct the docs that describe Fisher
**Goal**: No doc contradicts the design record.
**Implementation Logic**:
1. `git grep -n -i 'fisher' -- '*.md' ':!__roadmap__' ':!__reports__'`; rewrite each hit that says `analyze` runs Fisher's exact test, pointing to §Analysis statistics. List each change in the report.
**Deliverables**: the docs found; `__reports__/m6a-paired-stats/leaves/design_record_v0.md`: section "Docs changed"
**Consistency Checks**: `grep -q '^## Docs changed' __reports__/m6a-paired-stats/leaves/design_record_v0.md` (expected: PASS)
**Commit**: `docs(analysis): describe the blocked battery in place of Fisher`
