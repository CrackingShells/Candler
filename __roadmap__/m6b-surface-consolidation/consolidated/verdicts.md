# Verdicts report

**Goal**: Fold the six L0 reports into `__reports__/m6b-surface-consolidation/01-findings_verdicts_v0.md`, a findings report (R15) with one verdict row per id, the seams, the triad over the accepted set, the blast radius, D1–D3, the codes review, the responsibility record draft, design suggestions and steering questions; attacked by a verifier before Eliott reads it.
**Estimate**: about 70 Sonnet calls for the implementer ($2.9) and 50 for the verifier and fix rounds ($2.0); at 70 calls, stop, commit what is done, list what is left `[V]`
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-condition (campaign README, Gotchas): `git merge-base --is-ancestor $BASE HEAD`, and the L0 pre-condition of `consolidated/README.md`
- [ ] [nl] OWNED (regex): `^__reports__/m6b-surface-consolidation/(01-findings_verdicts_v0\.md|README\.md|leaves/verdicts_v0\.md)$`
**Success Gates**:
- ⬜ [cmd] COVER (campaign README, Gotchas) exits 0
- ⬜ [cmd] Campaign gate G2 passes
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] Each steering question stands alone for a reader who saw no agent traffic: its terms explained, checked against the repo first, its options and what each changes; every verifier finding is fixed or adjudicated in `leaves/verdicts_v0.md`
**References**: the six `__reports__/m6b-surface-consolidation/leaves/<leaf>_v0.md`; the campaign README (ids, leaf map, D1–D3, record format, idiom catalogue); [R04](../../../__reports__/m7-mcp-server/README.md) — the rulings and the rulings-table form; R15 `writing-reports`, findings type

## Step 1: The verdict table
**Goal**: Every id has exactly one row, and the overlaps are visible.
**Implementation Logic**:
1. Create the report with the R15 findings layout and `__reports__/m6b-surface-consolidation/README.md` (rounds: 00 the leaf reports, 01 this report; an empty `## Rulings` table for the coordinator).
2. Under `## Verdicts`, one table with the columns `| Id | Leaf | Verdict | Folded into | Reason |` and one row per id, in the order G, C, H, F, X. A primary id takes its record's verdict. A covered id takes its primary's verdict and names it under "Folded into". A split id (H5, H6) gets its own verdict from its parts, with the reason citing each part. No other row in the report starts with a bare id cell.
3. Run COVER and fix until it exits 0.
**Deliverables**: `__reports__/m6b-surface-consolidation/01-findings_verdicts_v0.md`: section `## Verdicts`; `__reports__/m6b-surface-consolidation/README.md`
**Consistency Checks**: COVER (expected: PASS)
**Commit**: `reports(api): table the verdicts of every m6b input`

## Step 2: Seams, triad and blast radius
**Goal**: The leaves' verdicts made coherent, and their cost visible.
**Implementation Logic**:
1. `## Seams`: each `## Seams` entry of the leaves, resolved (with the reason) or raised as a steering question. Check in particular: C1, C4 and C5 on `resolve`; C3, X6, X7 and X11 on settings and plug-in facts; D1 with C6, F2 and F5; G5 and G6 with the registry verdict; two accepted items that would own one fact.
2. `## Triad over the accepted set`: the nine cells summed over the accepted items, and the compromises taken.
3. `## Blast radius`: crates and modules, `schemas@` releases, `codes.toml` rows added, retired or lifted, guards, and the digests at risk (DIGEST, GOLDEN) with why.
**Deliverables**: `__reports__/m6b-surface-consolidation/01-findings_verdicts_v0.md`: sections `## Seams`, `## Triad over the accepted set`, `## Blast radius`
**Consistency Checks**: `R=__reports__/m6b-surface-consolidation/01-findings_verdicts_v0.md; for h in '## Seams' '## Triad over the accepted set' '## Blast radius'; do grep -q "^$h" $R || exit 1; done` (expected: PASS)
**Commit**: `reports(api): reconcile the seams and cost of the m6b verdicts`

## Step 3: Design questions, record and suggestions
**Goal**: What the build session will plan from.
**Implementation Logic**:
1. `## D1` and `## D2` from `cli_contract` and `docs_and_skills`, condensed; `## D3`: every test the accepted items propose, with its role, question, why no existing test answers it, and why it stays sustainable and scalable.
2. `## Codes review` from `codes_review`'s registry verdict.
3. `## Responsibility record (draft)` from `docs_and_skills`, with the seams' outcomes folded in.
4. `## Design suggestions`: the accepted items grouped into proposed build leaves, each with its ids, owned files, level (by shared files and dependencies), estimate in Sonnet calls, and whether it needs a verifier; then the drafted R01 §Future features rows; then the owners list (`owner:` verdicts with what to tell each owner).
**Deliverables**: `__reports__/m6b-surface-consolidation/01-findings_verdicts_v0.md`: sections `## D1`, `## D2`, `## D3`, `## Codes review`, `## Responsibility record (draft)`, `## Design suggestions`
**Consistency Checks**: `R=__reports__/m6b-surface-consolidation/01-findings_verdicts_v0.md; for h in '## D1' '## D2' '## D3' '## Codes review' '## Responsibility record (draft)' '## Design suggestions'; do grep -q "^$h" $R || exit 1; done` (expected: PASS)
**Commit**: `reports(api): draft the m6b record and design suggestions`

## Step 4: Steering questions and the verifier
**Goal**: The report is attacked before Eliott reads it, and asks him only what he must decide.
**Implementation Logic**:
1. `## Steering questions`: each decision the leaves could not settle, each standing alone (terms explained, checked against the repo first, options and what each changes). A headline result at the top of the report gives the verdict counts.
2. Dispatch a fresh Sonnet verifier (model passed explicitly) with this brief verbatim, adding only the branch name.
3. **Verifier brief:** "You verify the leaf `__roadmap__/m6b-surface-consolidation/consolidated/verdicts.md` on branch <branch>. Read the campaign README (rulings, ids, leaf map, D1–D3, record format, idiom catalogue), the six leaf reports under `__reports__/m6b-surface-consolidation/leaves/`, and the code through colgrep. Do not read `01-findings_verdicts_v0.md`'s headline or questions until you have your own findings from the leaf reports and the code. Assume something is wrong. Attack: an accepted item stitched from parts of others; a Candler responsibility pushed onto a consumer, plug-in or provider, or a consumer's job pulled into Candler; an idiom break the report does not name (thin surfaces, derived values, the plan naming rule, the error-format rule, generated facts, append-only codes, Future-feature triggers, no daemon); a blast radius understated (a hidden `schemas@` release, a digest that would move, a guard that would change); two accepted items that own one fact; a conflict between triad cells with no compromise; a `defer` with no evidence trigger; a test without D3's four answers; a 'live' claim that is false on the tip; a ruling of R04 contradicted. Report each finding with the id, the report line and the evidence (`path:line`). Change nothing."
4. Fix the findings; have the same verifier re-check through SendMessage. Record each finding and its disposition (fixed, or disputed with the reason for the coordinator) in `leaves/verdicts_v0.md`.
**Deliverables**: `__reports__/m6b-surface-consolidation/01-findings_verdicts_v0.md`: section `## Steering questions` and the headline; `__reports__/m6b-surface-consolidation/leaves/verdicts_v0.md`: section `## Verifier`
**Consistency Checks**: `grep -q '^## Steering questions' __reports__/m6b-surface-consolidation/01-findings_verdicts_v0.md && grep -q '^## Verifier' __reports__/m6b-surface-consolidation/leaves/verdicts_v0.md` (expected: PASS)
**Commit**: `reports(api): verify the m6b verdicts and ask what is open`
