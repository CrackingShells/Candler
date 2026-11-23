# Refusal codes review

**Goal**: Measure the refusal registry `docs/contract/codes.toml` before judging it, give a verdict on its number and granularity that keeps the append-only promise and adds no remedy key, and write one verdict record for each of G8 X9.
**Estimate**: about 50 Sonnet calls, $2.1; at 50 calls, stop, commit what is done, list what is left
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-condition (README, Gotchas): `git merge-base --is-ancestor $BASE HEAD`
- [ ] [nl] OWNED (regex): `^__reports__/m6b-surface-consolidation/leaves/codes_review_v0\.md$`
**Success Gates**:
- ⬜ [cmd] Records and sections: `R=__reports__/m6b-surface-consolidation/leaves/codes_review_v0.md; for id in G8 X9; do test "$(grep -Ec "^### $id: " $R)" = 1 || exit 1; done; test "$(grep -Ec '^- \*\*Verdict:\*\* (accept|defer|m7|owner:(plugin|treatment|provider|consumer)|reject|done)$' $R)" = 2 && grep -q '^## Measurements' $R && grep -q '^## Registry verdict' $R`
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] Every number in `## Measurements` comes with the command that produced it, so it can be rerun; the registry verdict proposes no `remedy` key and no deletion of a row (retire or lift instead)
**References**: [R13](../../docs/contract/codes.toml) — the registry and its header; [R01 §Error model, §Refusals, §Contract pages](../../__design__/architecture.md); [R04](../../__reports__/m7-mcp-server/README.md) — the refusal-hint ruling and the count (377; 375 live); [R06](../../__reports__/m7-mcp-server/00-inventory_gotchas_v0.md) — G8; [R09](../../__reports__/m7-mcp-server/00-inventory_docs_v0.md) — X9

## Step 1: Measure the registry
**Goal**: Numbers, not impressions, with the commands that produce them.
**Implementation Logic**:
1. Count rows per section and per side from `codes.toml` (a short `awk` over the file; `python3` here has no `tomllib`). R04 gives `internal` 85; a recount in planning gave 83: settle it.
2. Split the rows that are not caller codes: the `[internal]` allowlist (errors that print `candler.internal`), the `[helper]` rows (Python side), and the rows marked `retired` or `lifted`.
3. Find which caller codes a path can emit. Read `candler_core::refusal::catalog()` and the code-to-variant mapping with colgrep; count each code's emit sites with `git grep -c` on its literal string or variant. List the codes no path emits, and the inner codes hidden behind `*.document_invalid` (G8).
4. Group the codes that share one fix or one cause (candidates to merge), and the sections whose granularity differs most from the others.
5. Write `## Measurements` with each table and the command that produced it.
**Deliverables**: `__reports__/m6b-surface-consolidation/leaves/codes_review_v0.md`: section `## Measurements`
**Consistency Checks**: `grep -q '^## Measurements' __reports__/m6b-surface-consolidation/leaves/codes_review_v0.md` (expected: PASS)
**Commit**: `reports(api): measure the refusal registry`

## Step 2: Judge the registry, G8 and X9
**Goal**: A verdict on number and granularity, and the two records.
**Implementation Logic**:
1. Write `## Registry verdict`: for each finding of Step 1, keep, merge (by retiring rows into one), retire, or re-section, with the reason on the nine dimensions. A code is a contract: name what a caller sees change, and the contract-page and guard blast radius. Is 377 a lot because of the caller codes, or because the allowlist and helper rows share the file? Keep fact and advice apart: no remedy key, by ruling.
2. Under `## Verdicts`, write the G8 and X9 records in the README's record format. G8: should the inner code reach `causes[]` rather than print `*.document_invalid`? X9: contract-page rows that no longer apply and stale stamps.
3. End with `## Seams` (for example new codes named by `execution_preflight` for G5 and G6; the error shape under D1 in `cli_contract`).
**Deliverables**: `__reports__/m6b-surface-consolidation/leaves/codes_review_v0.md`: sections `## Registry verdict`, `## Verdicts` (2 records), `## Seams`
**Consistency Checks**: the Records and sections gate above (expected: PASS)
**Commit**: `reports(api): judge the refusal registry`
