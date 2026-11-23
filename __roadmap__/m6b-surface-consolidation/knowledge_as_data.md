# Knowledge as data verdicts

**Goal**: One verdict record for each of C3 C7 C12 G1 G17: the facts an agent now reads from Rust, from memory or by asking a provider, judged against Candler's rule that facts for agents are generated under drift tests.
**Estimate**: about 40 Sonnet calls, $1.6; at 40 calls, stop, commit what is done, list what is left
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-condition (README, Gotchas): `git merge-base --is-ancestor $BASE HEAD`
- [ ] [nl] OWNED (regex): `^__reports__/m6b-surface-consolidation/leaves/knowledge_as_data_v0\.md$`
**Success Gates**:
- ⬜ [cmd] Records: `R=__reports__/m6b-surface-consolidation/leaves/knowledge_as_data_v0.md; for id in C3 C7 C12 G1 G17; do test "$(grep -Ec "^### $id: " $R)" = 1 || exit 1; done; test "$(grep -Ec '^- \*\*Verdict:\*\* (accept|defer|m7|owner:(plugin|treatment|provider|consumer)|reject|done)$' $R)" = 5`
- ⬜ [cmd] Covers: `R=__reports__/m6b-surface-consolidation/leaves/knowledge_as_data_v0.md; for id in H3 G20 H4 G18 G19; do grep -Eq "^- \*\*Covers:\*\* (.*[ ,])?$id([ ,.;]|$)" $R || exit 1; done`
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] Each `accept` has a design suggestion whose tests answer D3; each `defer` has a Future row with an evidence trigger; each claim about the code cites `path:line`
**References**: [R05 §4, Results Tables](../../__reports__/m7-mcp-server/01-findings_agent_consumability_v1.md) — C3, C7, C12, G18–G20; [R06](../../__reports__/m7-mcp-server/00-inventory_gotchas_v0.md) — G1, G17; [R07](../../__reports__/m7-mcp-server/01-input_htn_lane_v0.md) — H3, H4; [R01 §Contract pages, §Future features](../../__design__/architecture.md)

## Step 1: Check the inputs on the tip
**Goal**: Know which asks are live and what problem each one really names.
**Implementation Logic**:
1. For each primary id, read its source and check it against the code with colgrep. Write `## Live on the tip` as a table `| Id | Live | Evidence |`, citing `path:line`.
2. Question each ask:
   - **C3, H3 and G20 (capability reference as data).** What `settings_table()` and the contract-page generator (`contract_pages` drift test) already produce, and what an agent still has to read from Rust. Is the gap a missing generated page, or a fact with no single owner?
   - **C7, H4, G18 and G19 (provenance and errata).** What a run manifest records today (Candler version or commit? the model alias or the backing model?). Recording the model the endpoint reports touches the provider: which part is Candler's (recording what it is told, I8 ground truth) and which is the provider's? An errata registry is a new fact source: who owns it, and does it break "one source per fact"? A manifest change is a `schemas@` release.
   - **C12 (scaffolds).** Do scaffolds derive anything, or do they move guessing from the agent into a template that can rot? Weigh against D3 and X6 (`docs_and_skills` owns X6; name the seam).
   - **G1 (wrong catalog or stratum reads as valid).** What resolve and census check today; is the remaining gap a Candler guard (like C2's ruling) or a consumer's design error?
   - **G17 (compatibility without `!` markers).** No external users: a compatibility finding goes to the supervising session. Is anything left to decide here?
**Deliverables**: `__reports__/m6b-surface-consolidation/leaves/knowledge_as_data_v0.md`: sections `## Live on the tip` and `## The asks questioned`
**Consistency Checks**: `grep -q '^## Live on the tip' __reports__/m6b-surface-consolidation/leaves/knowledge_as_data_v0.md` (expected: PASS)
**Commit**: `reports(api): check the knowledge inputs on the tip`

## Step 2: Write the verdict records
**Goal**: One record per primary id, in the README's record format.
**Implementation Logic**:
1. Under `## Verdicts`, write one record per primary id in the README's record format, with the nine triad cells and the compromise.
2. Covered ids: H3 and G20 on C3; H4, G18 and G19 on C7.
3. For each `accept`, the design suggestion names the generator or record that owns the fact, its files, its tests per D3 (a drift test is the existing idiom: say why it suffices or why not), and its size. For each `defer`, draft the R01 §Future features row with an evidence trigger.
4. End with `## Seams` (for example C3 with X7 and X11 in `docs_and_skills`, C7 with D1 in `cli_contract`).
**Deliverables**: `__reports__/m6b-surface-consolidation/leaves/knowledge_as_data_v0.md`: sections `## Verdicts` (5 records) and `## Seams`
**Consistency Checks**: the Records gate above (expected: PASS)
**Commit**: `reports(api): judge the knowledge inputs`
