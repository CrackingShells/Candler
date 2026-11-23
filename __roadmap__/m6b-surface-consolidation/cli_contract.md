# CLI contract verdicts

**Goal**: Answer D1 (one output contract, and whether `candler-schemas` holds its ground truth) from the code, and write one verdict record for each of X1 X4 X10 G11 F5 G9 C11 C13 C14 X3.
**Estimate**: about 55 Sonnet calls, $2.3; at 55 calls, stop, commit what is done, list what is left
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-condition (README, Gotchas): `git merge-base --is-ancestor $BASE HEAD`
- [ ] [nl] OWNED (regex): `^__reports__/m6b-surface-consolidation/leaves/cli_contract_v0\.md$`
**Success Gates**:
- ⬜ [cmd] Records: `R=__reports__/m6b-surface-consolidation/leaves/cli_contract_v0.md; for id in X1 X4 X10 G11 F5 G9 C11 C13 C14 X3; do test "$(grep -Ec "^### $id: " $R)" = 1 || exit 1; done; test "$(grep -Ec '^- \*\*Verdict:\*\* (accept|defer|m7|owner:(plugin|treatment|provider|consumer)|reject|done)$' $R)" = 10`
- ⬜ [cmd] D1 and covers: `R=__reports__/m6b-surface-consolidation/leaves/cli_contract_v0.md; grep -q '^## D1' $R || exit 1; for id in G7 G12 H5; do grep -Eq "^- \*\*Covers:\*\* (.*[ ,])?$id([ ,.;]|$)" $R || exit 1; done`
- ⬜ [cmd] Ownership check prints nothing
- ⬜ [nl] `## D1` answers both of Eliott's questions with evidence, and X1, X4, X10, G11 and F5 each refer to it; each `accept` has a design suggestion whose tests answer D3; each claim about the code cites `path:line`
**References**: README Gotchas, D1 (Eliott's questions and today's facts); [R01 §CLI surface, §Error model, §Refusals, §Risks & Mitigations, §Future features](../../__design__/architecture.md); [R06](../../__reports__/m7-mcp-server/00-inventory_gotchas_v0.md) — G7, G9, G11, G12; [R05 §4, §Form](../../__reports__/m7-mcp-server/01-findings_agent_consumability_v1.md) — C11, C13, C14; [R08](../../__reports__/m7-mcp-server/00-inventory_surface_v0.md) — the operation table; [R10](../../__reports__/m6a-paired-stats/knowledge_transfer_v0.md) — F5; [R04](../../__reports__/m7-mcp-server/README.md) — no remedy hints

## Step 1: Map today's outputs and answer D1
**Goal**: A fact table of every verb's output, and D1 answered from it.
**Implementation Logic**:
1. With colgrep over `crates/candler/src/` and `crates/candler-schemas/src/`, build `## Outputs today`: one row per verb (`| Verb | Formats | Default | Shape owner | Schema | Drift test |`). Shape owner is where the output's fields are decided: a typed record in `candler-schemas`, a type in `candler-core`, or `json!`/`println!` in the CLI crate. Start from the README's D1 facts and check each.
2. Write `## D1`, answering Eliott's two questions:
   - **One format option.** Would one option for every verb (`--format json|text`, or `--json`) with text rendered from the same data make the CLI predictable? What do `analyze`'s `Csv` and `Table`, `show`/`diff`'s required `--format`, `ps`/`ls`'s TSV and `--error-format` become? Weigh R01 Risks: "every new option must say why it cannot be derived or live in the experiment file". Say what the default should be and why, against R01's rule that the error format is never derived from a terminal.
   - **Ground truth in `candler-schemas`.** Which output types would move into it, as versioned records with `schema_drift`, and which stay in the operation API? What is the blast radius in `schemas@` releases? How does text stay a rendering, never a second source? How would M7's MCP reuse the same types without this study deciding M7's shape?
**Deliverables**: `__reports__/m6b-surface-consolidation/leaves/cli_contract_v0.md`: sections `## Outputs today` and `## D1`
**Consistency Checks**: `R=__reports__/m6b-surface-consolidation/leaves/cli_contract_v0.md; grep -q '^## Outputs today' $R && grep -q '^## D1' $R` (expected: PASS)
**Commit**: `reports(cli): map the output contract and answer d1`

## Step 2: Write the verdict records
**Goal**: One record per primary id, in the README's record format.
**Implementation Logic**:
1. Under `## Verdicts`, write one record per primary id in the README's record format, with the nine triad cells and the compromise. X1, X4, X10, G11 and F5 refer to `## D1` and differ only where D1 gives them different verdicts.
2. Question the other asks:
   - **G9 (store root fallback).** R06 says `config.no_store` is "not raised today": check, and judge whether the fallback is designed (I12: each value records its source) or a trap.
   - **C11 (remedies).** Ruled: no remedy hints by default. Is anything left, such as valid values in a refusal that are already facts (not advice)? Keep fact and advice apart.
   - **C13 and G7 (`plugin check`).** Plug-in author traps: Candler's protocol checks, or the plug-in author's? What does the evaluator protocol already check?
   - **C14 and G12 (rules inference for `ingest`).** Does inference move guessing into code that can be wrong silently?
   - **X3 (help text).** Value domains and exit codes in `--help`: generated from the same source as the contract pages, or a second source?
3. Covered ids: G7 on C13; G12 on C14; H5 on F5 (split with `analysis_outputs`).
4. End with `## Seams` (for example D1 with C6 and F2 in `analysis_outputs`, C3 in `knowledge_as_data`, the error shape with `codes_review`).
**Deliverables**: `__reports__/m6b-surface-consolidation/leaves/cli_contract_v0.md`: sections `## Verdicts` (10 records) and `## Seams`
**Consistency Checks**: the Records gate above (expected: PASS)
**Commit**: `reports(cli): judge the cli contract inputs`
