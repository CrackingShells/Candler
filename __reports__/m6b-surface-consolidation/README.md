# M6b surface consolidation: verdict study reports

## Round 00: leaf reports (2026-11-23)

Six read-only leaves judged the 63 inputs raised by the M7 pre-study, one report per code area. Evidence: each carries its own `path:line` citations from the tip `a0df4d48`.

- `leaves/execution_preflight_v0.md`: C1, C2, C4, C5, G2, G5, G6, G10, G14, G15, G21, X2
- `leaves/knowledge_as_data_v0.md`: C3, C7, C12, G1, G17
- `leaves/analysis_outputs_v0.md`: F2, F4, C6, C8, C9, C10, G13, X8
- `leaves/cli_contract_v0.md`: X1, X4, X10, G11, F5, G9, C11, C13, C14, X3, and D1
- `leaves/docs_and_skills_v0.md`: X5, G16, H9, H10, H11, H12, X6, X7, X11, X12, and D2
- `leaves/codes_review_v0.md`: G8, X9, and the registry review

## Round 01: the verdicts report (2026-11-23)

- `01-findings_verdicts_v0.md` (latest): one verdict row per id, the seams, the triad, the blast radius, D1 to D3, the codes review, the responsibility record draft, design suggestions and the steering questions.
- `leaves/verdicts_v0.md`: the verifier's findings and their dispositions.

## Round 02: candidate inputs from HTN Phase 6 run 2 (2026-11-23)

- `02-input_htn_run2_gaps_v0.md`: a verbatim copy of the HTN lane's `CANDLER_GAPS.md`. It records what the lane hit while running Candler `5199afc7` (v0.13.0). The supervising session routed it; Eliott has not approved it. The run store it cites is read-only evidence and may move.
- These are candidates, not inputs, until Eliott takes them in. Ids keep the file's numbering under the prefix N. Item 7 is the skill's to fix and is left out. So is the note that the local `main` checkout is behind, which describes the lane's machine, not Candler.
- None of these items is checked in code yet. Only the ones Eliott takes in are checked, read-only through colgrep.

| Id | Item | Relation to the judged inputs |
|:--|:--|:--|
| N1 | A native run cannot supply metadata a plug-in suite needs, so `evaluate` and `analyze` were blocked | New. Next to C3/G20 (one path table) and C13 (`check_suite`), neither of which covers it. Open question of ownership: Candler (a declared metadata surface) or the plug-in (it asks for a path no run records) |
| N2 | `--set replicates=1` ignored without a warning | Extends G10 (`done`): its leaf left the `--set` residuals unread |
| N3 | `budget.turns=200`: one run stopped at 201, another completed at 250 | Extends G14 (`done`): its leaf left cap metering unverified. A possible budget-enforcement defect |
| N4 | `[treatment]` needs a source and tag even when `request.treatment` is the factor | New. Nearest: G10 (traps in the experiment file) |
| N5 | `treatment:` refused in a plug-in's `isolation.build.context` | New. Nearest: G15 (isolation facts an author trips over) |
| N6 | A prompt starting with `/` passes `resolve` and is refused at run time | New. Nearest: C5 (what runs before the image step); C5's ask was a skip switch, this one moves a check earlier |
| N8 | `CANDLER_STORE` may not be honoured by `resolve` | Extends G9 (`done`): its leaf read `store_root` honouring it (`commands/common.rs:106-116`), not the path `resolve` takes |
| N9a | Stored workspaces keep 0-byte placeholders for mounted server files | New observation |
| N9b | No way found to set the harness's maximum output tokens | New observation |

## Rulings

Eliott's rulings on the verdicts report, one row per discussion round. Empty until the first round.

| Round | Question | Ruling |
|:--|:--|:--|

## Status

Verdict study, discussion not started. Round 02's candidates wait for Eliott to take them in or leave them out. The build session starts only after Eliott says Go, recorded in the rulings table.
