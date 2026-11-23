# m6b-surface-consolidation

## Context
Milestone M6b of `candler-v1`, the leaf `surface/m6b_surface_consolidation.md` (R02), inserted before M7 by amendment A11. This campaign is the **verdict study only**: it judges every input raised by the M7 pre-study and gives design suggestions; it builds nothing. Eliott set this scope in plan review (2026-11-23): this session explores, judges and suggests; he reviews the result over discussion rounds; after he says **Go**, this session starts a new session that plans and runs the build from the approved report. Build nodes are therefore not in this roadmap.
- **L0, this level:** six verdict leaves in parallel, one per code area: `execution_preflight`, `knowledge_as_data`, `analysis_outputs`, `cli_contract`, `docs_and_skills`, `codes_review`. Each writes `__reports__/m6b-surface-consolidation/leaves/<leaf>_v0.md`.
- **L1, `consolidated/`:** `verdicts` folds the six reports into `__reports__/m6b-surface-consolidation/01-findings_verdicts_v0.md`, with a verifier.
- **Discussion rounds** (not nodes): this session presents the report, records Eliott's rulings in the report folder's README, and revises the report (`_v1`, `_v2`, …) until Go.
No gate needs cargo, Docker or a model call. No file outside `__reports__/m6b-surface-consolidation/` and this campaign changes.

## Reference Documents
- [R01 Architecture](../../__design__/architecture.md) — §Proposed State (Layer responsibilities), §Operation API, §CLI surface, §Contracts & Invariants (Error model, Refusals, Contract pages), §Risks & Mitigations, §Future features
- [R02 M6b node](../candler-v1/contracts/parity/alignment/surfaces/isolation/m5b/stats/surface/m6b_surface_consolidation.md) — the milestone's gates (read only; its Step 2 is now split: verdicts here, build in a new session)
- [R03 Approved plan](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m6b-approved-plan.md) — scope, ids, D1–D3, leaf map, execution model, budget
- [R04 M7 pre-study README](../../__reports__/m7-mcp-server/README.md) — Eliott's round-02 rulings, which bind every verdict
- [R05 Consumability findings v1](../../__reports__/m7-mcp-server/01-findings_agent_consumability_v1.md) — G18–G21, the chores C1–C15 (§4), patterns P0–P9, the form section, the extra proposals
- [R06 Gotchas inventory](../../__reports__/m7-mcp-server/00-inventory_gotchas_v0.md) — G1–G17 in full, with their sources
- [R07 HTN lane input](../../__reports__/m7-mcp-server/01-input_htn_lane_v0.md) — the asks H1–H12
- [R08 Surface inventory](../../__reports__/m7-mcp-server/00-inventory_surface_v0.md) and [R09 Docs inventory](../../__reports__/m7-mcp-server/00-inventory_docs_v0.md) — the operation table, refusal surface, doc table and gaps
- [R10 M6a knowledge transfer](../../__reports__/m6a-paired-stats/knowledge_transfer_v0.md) — F2, F4, F5
- [R11 Cost report v2](../../__reports__/agent-token-costs/03-findings_campaign_costs_v2.md) — the per-call rates the budget uses
- [R12 AGENTS.md](../../AGENTS.md) — the triad and what fails it
- [R13 Refusal registry](../../docs/contract/codes.toml) — the 377 rows `codes_review` judges
- [R14 designing-tests skill](/Users/hacker/.claude/plugins/cache/cracking-shells/designing-tests/1.1.0/skills/designing-tests/SKILL.md) — roles (probe, confirm, guard) for D3
- [R15 writing-reports skill](/Users/hacker/.claude/plugins/cache/cracking-shells/writing-reports/2.1.0/skills/writing-reports/SKILL.md) — the findings report type and round-versioned naming

## Goal
Every input raised before M7 has one explicit, reasoned verdict, judged on responsibility, blast radius, the nine triad dimensions and idiom fit, and the accepted ones come with design suggestions that a build session can plan from; Eliott has discussed and approved them and said Go.

## Pre-conditions
- [x] [nl] Eliott approved the plan (2026-11-23) with his review comments, recorded here as D1–D3 and the verdict-study scope
- [ ] [cmd] `$BASE` is the plan commit on this branch, and every leaf branch starts from it: the coordinator records `$BASE` in the Progress table; `git merge-base --is-ancestor $BASE HEAD`
- [ ] [nl] The coordinator has built one colgrep index over this worktree's `crates/` and `python/` dirs before dispatching L0

## Success Gates
- ⬜ [cmd] G1 every id has exactly one verdict row in the report, with a verdict from the closed set: COVER (below) exits 0
- ⬜ [cmd] G2 each design question and the codes review has its section: `R=__reports__/m6b-surface-consolidation/01-findings_verdicts_v0.md; for h in '## D1' '## D2' '## D3' '## Codes review' '## Responsibility record (draft)' '## Design suggestions' '## Steering questions'; do grep -q "^$h" $R || exit 1; done`
- ⬜ [cmd] G3 no remedy key was added: `! grep -Eq '^remedy *=' docs/contract/codes.toml`
- ⬜ [cmd] G4 nothing outside the reports and this campaign changed: `test -z "$(git diff --name-only $BASE..HEAD | grep -vE '^__reports__/m6b-surface-consolidation/|^__roadmap__/m6b-surface-consolidation/')"`
- ⬜ [nl] G5 the responsibility record draft names the four audiences (store window, web-wide, interactive, development) and the rule that interactive surfaces are self-contained and offline
- ⬜ [nl] G6 every `accept` that brings a test answers D3's four questions; every verifier finding is fixed or adjudicated in `leaves/verdicts_v0.md`
- ⬜ [nl] G7 Eliott said Go, recorded in the rulings table of `__reports__/m6b-surface-consolidation/README.md`; only then does the build session start

## Gotchas
- **Rulings bind.** R04 round 02: items are inputs, not a backlog; each verdict explicit; no feature stitched from parts of others; Candler's idioms; no user-behaviour baseline; no refusal remedy hints by default; docs are the web-wide surface, while the CLI, MCP and skills are interactive, self-contained, offline and never link out; M7's MCP shape is decided at M7 planning; a container that differs from its files is a Candler defect for Candler's own guards and preflight.
- **Input ids** (63). G1–G17 are R06's gotchas, G18–G21 R05's. C1–C15 are R05 §4's chores in its numbering (1 pre-spend plan and cost, 2 agent-world probe, 3 capability reference, 4 `resolve --explain`, 5 validate without Docker, 6 summary, 7 provenance and errata, 8 spend, 9 `show --transcript`, 10 identical-run flag, 11 remedies, 12 scaffolds, 13 plug-in check, 14 rules inference, 15 Docker lock and gc). H1–H8 are R07's "Automate in code" list in order; H9 its experiment-design guidance and worked fixtures; H10 its agent-environment checklist; H11 its gap-ownership notes; H12 its view on form. F2, F4, F5 are R10's. X1–X12 are proposals with no other id:
  - X1 JSON output for every verb (`ps` and `ls` print TSV only; `resolve` prints nothing on success)
  - X2 one way to wait (`run` detaches while `evaluate` blocks)
  - X3 CLI help with value domains, examples and exit codes
  - X4 output schemas for `analyze`, `show` and `inspect` (R01 §Future features lists "schemas for CLI outputs")
  - X5 `README.md` as the store window (D2)
  - X6 sample files: experiment, suite, manifest, import rules
  - X7 a page for `candler.toml` and the user config
  - X8 `docs/statistics.md:165` ("Paired tests are not offered yet") read against M6a; the `analyze` field names
  - X9 contract-page rows that no longer apply, and stale version stamps
  - X10 `show` and `diff` require `--format` and have no default
  - X11 the Python plug-in API (kind signature, `RunContext`, `KINDS`, `needs`, `params_schema`) is visible only in code
  - X12 no single glossary
  Not inputs: M6a's F1 and F3 (M6a follow-ups), the contributor chores of R06 §3, and P0–P9 (evidence).
- **Leaf map.** Each id has one primary leaf, which writes its record; a covered id is named on its primary's `Covers:` line; a split id is covered by two primaries and `verdicts` writes its row from both.
  | Leaf | Primary ids | Covers |
  |:--|:--|:--|
  | `execution_preflight` | C1 C2 C4 C5 G2 G5 G6 G10 G14 G15 G21 X2 | H1 (C2), H2 (C4), H8 and C15 (G21), H6 split (C1) |
  | `knowledge_as_data` | C3 C7 C12 G1 G17 | H3 and G20 (C3), H4, G18 and G19 (C7) |
  | `analysis_outputs` | F2 F4 C6 C8 C9 C10 G13 X8 | G3 (F2, F4), H7 (C9), G4 (C10), H5 split (F2 F4 C6), H6 split (C8) |
  | `cli_contract` | X1 X4 X10 G11 F5 G9 C11 C13 C14 X3 | G7 (C13), G12 (C14), H5 split (F5) |
  | `docs_and_skills` | X5 G16 H9 H10 H11 H12 X6 X7 X11 X12 | — |
  | `codes_review` | G8 X9 | — |
  47 primary + 14 covered + 2 split (H5, H6) = 63.
- **Design questions from Eliott's review** (carried verbatim into each brief):
  - **D1 One output contract** (`cli_contract`, `## D1`). Eliott asks: should every verb take one format option (`--format json|text` or `--json`), with `text` a readable rendering of the same JSON data, so the CLI stays predictable for consumers? And if JSON becomes the default, should `candler-schemas` hold the ground truth? The repo today: `candler-schemas` holds versioned records (`experiment/1`, `job-record/1`, `refusal/1`) under `schema_drift`; `analyze`'s JSON is built by hand with `json!` in the CLI crate (`crates/candler/src/commands/analyze.rs`, `arm_json`, `entry_json`, `comparison_json`), with no schema or drift test; `analyze` also has `Csv` and `Table`; `show` and `diff` require `--format`; `ps` and `ls` print TSV; `--error-format` is never derived from a terminal (R01 §CLI surface). The records X1, X4, X10, G11 and F5 each refer to `## D1`, which says which output types move into `candler-schemas`, the blast radius as `schemas@` releases, how text stays a rendering and never a second source, and how the CLI and M7's MCP would share the types without deciding M7's shape.
  - **D2 README as the store window** (`docs_and_skills`, `## D2`). Four audiences: the store window (`README.md`: what Candler is, its main features, a quick start, where to go for more), the web-wide surface (`docs/`), the interactive surfaces (CLI, MCP, skills: self-contained, offline), and development (`AGENTS.md`, `CLAUDE.md`, `__design__/`: for agents developing Candler, never a consumer entry point). X5 is judged against this, and the responsibility record is drafted over the four.
  - **D3 Tests are designed, not added** (every leaf; `verdicts` writes `## D3`). Every `accept` that brings a test uses R14 and states its role (probe, confirm or guard), the question it answers, why no existing test answers it, and why it stays sustainable and scalable. X6 first checks whether fixtures that tests already parse can serve as the samples, so no new test is needed.
- **Verdict record** (one per primary id, in the leaf report under `## Verdicts`; no other line in the report starts `### <id>: `):
  ```
  ### <ID>: <short title>
  - **Verdict:** <accept | defer | m7 | owner:<plugin|treatment|provider|consumer> | reject | done>
  - **Covers:** <covered ids, or none>
  - **Live on the tip:** <yes or no, with `path:line` citations from colgrep>
  - **The ask questioned:** <is the premise right? what is the real problem?>
  - **Responsibility:** <whose, judged against README.md and R01 §Layer responsibilities>
  - **Blast radius:** <crates and modules; schemas@ release yes/no; codes.toml rows; guards; digests at risk>
  - **Triad:** <3×3 table: rows sustainable, scalable, efficient; columns development, codebase, consumption; each cell +, 0 or − with one line>
  - **Compromise:** <where two cells conflict, which wins and why; or none>
  - **Idioms:** <which idioms it follows or breaks, from the catalogue>
  - **Reason or owner:** <for reject and owner:…, the reason; otherwise one line>
  - **Design suggestion:** <accept only: files, tests per D3, size in Sonnet calls>
  - **Future row:** <defer only: a drafted R01 §Future features row with its evidence trigger>
  ```
  `accept` means Candler owns it and the build session builds it; `defer` means Candler owns it but it waits for an evidence trigger (R01 §Future features); `m7` means M7's planning owns it; `owner:<who>` means someone other than Candler owns it; `reject` means nobody should do it; `done` means it is already true on the tip.
- **Idiom catalogue** (facts each verdict weighs, not verdicts): one operation API in `candler-core`, thin surfaces (R01 Layer responsibilities); values are derived, never asked for, each records its source (I12); "every new option must say why it cannot be derived or live in the experiment file" (R01 Risks); the experiment is never called a plan (R01); `--error-format` is never derived from a terminal (R01 §CLI surface); facts for agents are generated under drift tests (`contract_pages`, `schema_drift`); a refusal is a code, a side and a path, and `codes.toml` is hand-written, append-only, with `retired` and `lifted` keys and an `[internal]` allowlist; Future features are built only when their evidence trigger is met; invariants I1 (no domain), I8 (ground truth), I11 (append-only), I13 (budget), I16 (nothing runs when no job runs: no daemon), I19 (secrets); AGENTS.md's fail list (band-aids, defensive checks against own code, a second layer of validation, fixture special cases, a test per incident); a crate's modules are its namespace; schema changes are scoped `schemas` and touch no other crate; no external users, so a compatibility finding goes to the supervising session.
- **Evidence and tools.** Leaves read code through colgrep only (`search`, then `expand` on the hits that matter), scoped with `paths` to this worktree's `crates/` or `python/`, `limit` 5–10. Docs and reports are read as files. `codes_review` may count literal code strings with `git grep -c`. No baseline, no model runs, no Docker, no cargo. A claim about the code cites `path:line`.
- **Owned files.** Each L0 leaf owns only `__reports__/m6b-surface-consolidation/leaves/<leaf>_v0.md`; `verdicts` owns the report, the folder README and `leaves/verdicts_v0.md`. The ownership check is `git diff --name-only $BASE_LEAF..HEAD | grep -vE '<owned regex>'`, which must print nothing. No leaf edits `__roadmap__/` or any other file.
- **Every leaf.** Pre `[cmd]`: `git merge-base --is-ancestor $BASE HEAD` in the leaf's own worktree. Post `[cmd]`: the ownership check and the leaf's record check. Commits are `reports(api): …`, with `-F` messages and the trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **COVER** (G1; seen red before the report existed):
  ```
  R=__reports__/m6b-surface-consolidation/01-findings_verdicts_v0.md
  IDS="$(printf 'G%s ' $(seq 1 21))$(printf 'C%s ' $(seq 1 15))$(printf 'H%s ' $(seq 1 12))F2 F4 F5 $(printf 'X%s ' $(seq 1 12))"
  for id in $(echo $IDS); do
    rows=$(awk -F'|' -v id="$id" '{k=$2; gsub(/ /,"",k)} k==id' "$R" | wc -l | tr -d ' ')
    test "$rows" = 1 || { echo "$id: $rows rows"; exit 1; }
    v=$(awk -F'|' -v id="$id" '{k=$2; gsub(/ /,"",k)} k==id {v=$4; gsub(/^ +| +$/,"",v); print v}' "$R")
    echo "$v" | grep -Eqx 'accept|defer|m7|owner:(plugin|treatment|provider|consumer)|reject|done' || { echo "$id: bad verdict '$v'"; exit 1; }
  done
  ```
  The report's verdict table has the columns `| Id | Leaf | Verdict | Folded into | Reason |`; no other table row in the report starts with a bare id cell.
- **Spec defects.** Verify each claim against the code before judging it. A claim in R05–R09 that is wrong on the tip is a finding (verdict `done` or `reject` with the evidence), never a premise.
- **Execution model.** This session is the coordinator (Opus): it dispatches every L0 leaf at once, each a Sonnet subagent with `model: "sonnet"` passed explicitly, in its own worktree branched from `$BASE`; integrates each leaf (rebase onto the tip, record check, ownership check, `git merge --no-ff -F <file>`, confirm with `git log`); reviews the seams after L0 (id coverage, covered ids named by their primary, two leaves claiming one fact, one leaf assuming another's verdict) and sends gaps back to the agent that holds the context; then dispatches `verdicts`. It never verifies its own dispatches. Each brief carries the leaf file, D1–D3, colgrep-only reading, the shared-machine rules, "report spec defects, do not act on them" and the call cap as an action ("at N calls, stop, commit what is done, list what is left"). It batches `dirtree-rdm update` calls per level and re-reads `dirtree-rdm ls` at each level close.
- **Verifier.** `verdicts` is the one `[V]` leaf. Its implementer, once its gates pass, dispatches a fresh Sonnet verifier with the leaf's Verifier brief verbatim, adding only the branch name, fixes the findings, has the same verifier re-check through SendMessage, and reports once. If a Sonnet subagent cannot dispatch Agent, the coordinator dispatches the verifier with the same brief and relays its findings to the implementer.
- **Shared machine.** No builds, no Docker. Worktrees are removed after their merge; the colgrep index is cleared at Go.
- **Budget** (R11 rates: $0.041 per Sonnet call, $0.063 per coordinator call). L0 leaves 275 calls, $11.3; `verdicts` with verifier and fixes 120, $4.9; discussion revisions by Sonnet agents (about 3 rounds) 90, $3.7; this session as coordinator 150, $9.5; contingency 15 % $4.4. **Cap $34; the coordinator stops and reports to Eliott at $27.** The milestone coordinator (the supervising session) has its own row, $18. The build's budget is set by the build session's plan. Spend is measured from transcripts after L0, after L1 and at Go. Each leaf's call cap is in its Estimate line.


## Status
```mermaid
graph TD
    execution_preflight[Execution and preflight verdicts]:::done
    knowledge_as_data[Knowledge as data verdicts]:::done
    analysis_outputs[Analysis outputs verdicts]:::done
    cli_contract[CLI contract verdicts]:::done
    docs_and_skills[Docs and skills verdicts]:::done
    codes_review[Refusal codes review]:::done
    consolidated[Consolidated verdicts]:::inprogress
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `execution_preflight.md` | 📄 Leaf Task | ✅ Done |
| `knowledge_as_data.md` | 📄 Leaf Task | ✅ Done |
| `analysis_outputs.md` | 📄 Leaf Task | ✅ Done |
| `cli_contract.md` | 📄 Leaf Task | ✅ Done |
| `docs_and_skills.md` | 📄 Leaf Task | ✅ Done |
| `codes_review.md` | 📄 Leaf Task | ✅ Done |
| `consolidated/` | 📁 Directory | 🔄 In Progress |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
