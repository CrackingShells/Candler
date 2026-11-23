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

- `01-findings_verdicts_v0.md`: one verdict row per id, the seams, the triad, the blast radius, D1 to D3, the codes review, the responsibility record draft, design suggestions and the steering questions.
- `leaves/verdicts_v0.md`: the verifier's findings and their dispositions.

## Round 02: candidate inputs from HTN Phase 6 run 2 (2026-11-23)

- `02-input_htn_run2_gaps_v0.md`: a verbatim copy of the HTN lane's `CANDLER_GAPS.md`. It records what the lane hit while running Candler `5199afc7` (v0.13.0). The supervising session routed it; Eliott has not approved it. The run store it cites is read-only evidence and may move.
- These are candidates, not inputs, until Eliott takes them in. Ids keep the file's numbering under the prefix N. Item 7 is the skill's to fix and is left out. So is the note that the local `main` checkout is behind, which describes the lane's machine, not Candler.
- None of these items is checked in code yet. Only the ones Eliott takes in are checked, read-only through colgrep.

| Id | Item | Relation to the judged inputs |
|:--|:--|:--|
| N1 | A native run cannot supply metadata a plug-in suite needs, so `evaluate` and `analyze` were blocked | New. Next to C3/G20 (one path table) and C13 (`check_suite`), neither of which covers it. Open question of ownership: Candler (a declared metadata surface) or the plug-in (it asks for a path no run records) |
| N2 | `--set replicates=1` ignored without a warning | Extends G10 (`done`): its leaf left the `--set` residuals unread |
| N3 | `budget.turns=200`: run `a16b0809…-r2-a1` stopped at 201 by the cap, run `a16b0809…-r4-a1` completed at 250 | Extends G14 (`done`): its leaf left cap metering unverified. A possible budget-enforcement defect |
| N4 | `[treatment]` needs a source and tag even when `request.treatment` is the factor | New. Nearest: G10 (traps in the experiment file) |
| N5 | `treatment:` refused in a plug-in's `isolation.build.context` | New. Nearest: G15 (isolation facts an author trips over) |
| N6 | A prompt starting with `/` passes `resolve` and is refused at run time | New. Nearest: C5 (what runs before the image step); C5's ask was a skip switch, this one moves a check earlier |
| N8 | `CANDLER_STORE` may not be honoured by `resolve` | Extends G9 (`done`): its leaf read `store_root` honouring it (`commands/common.rs:106-116`), not the path `resolve` takes |
| N9a | Stored workspaces keep 0-byte placeholders for mounted server files | New observation |
| N9b | No way found to set the harness's maximum output tokens (run `be1123c5…-r4-a1` died at the 32000-token limit) | New observation |

## Round 03: Eliott's rulings on the steering questions (2026-11-23)

- `01-findings_verdicts_v1.md`: v0 with the rulings below applied. G11 and C2 (with H1) move to `accept`; the build leaves grow to about 272 calls. Open: Q3 (answered with Eliott's question), the Q4 border, the round 02 candidates.
- The supervising session accepted Q1 and Q2 as a breaking change and set conditions on the `output_contract` leaf (no aliases, `!` commits with a migration path, every in-repo caller swept in the same merge, a migration note for the HTN lane). They are in v1 under Proposed build leaves.

## Round 04: Q3 ruled, Q4 asked back (2026-11-23)

- `01-findings_verdicts_v2.md`: X6 is rejected (Q3), and the build leaves come to about 262 calls. Q4c answers Eliott's question on how a check would find a server in any language, with three options. The round 02 candidates now have their own section in the report.

## Round 05: Q4c ruled, round 02 taken in (2026-11-23)

- `01-findings_verdicts_v3.md` (latest): C2 takes Q4c's form, and N1 to N9b get verdict rows (72 in all). The build leaves come to about 302 calls, with a new `experiment_env` leaf for N9b. Nothing is left open.
- `leaves/round02_candidates_v0.md`: the code check of N1 to N9b and of the start-command gate. `leaves/n9b_env_scout_v0.md`: the scout that found N9b is not a duplicate. The coordinator checked N3's turn counts and arguments in the run store.
- The HTN coordinator was told N1's ruling, the dogfooding direction and Q4c's install advice (2026-11-23).

## Round 06: Go (2026-11-23)

- Eliott said Go on `01-findings_verdicts_v3.md` and asked this session to write the build leaves itself. Amendment A1 adds them under `__roadmap__/m6b-surface-consolidation/consolidated/build/`: nine leaves over four levels, from v3's §Design suggestions, with a build cap of $55.
- The leaf reports of the build will go under `build/`, the knowledge transfer as `03-knowledge_transfer_build_v0.md`.
- Eliott ruled the Q7 word: "comparability class", key `comparability_class`. `analysis_record` and `docs_chores` use it.

## Round 07: build L0 closed (2026-11-23)

- The four L0 leaves of the build are merged (`21ed4ae7`). The build had spent $32.6 of its $55 cap, and the five leaves left projected to about $37, so the coordinator stopped and asked.
- Eliott rejected N6: the tip has no rule that refuses a prompt starting with `/` (`crates/candler-harness/src/spec.rs:1389-1467` has none), so there was nothing to move into `resolve`.
- Eliott kept S1 and S2, the two defects of the HTN smoke run, out of M6b: they go in a PR of their own, with a report set of their own (`__reports__/htn-smoke-defects/`).
- Eliott raised the build cap to $90 and said to proceed.

## Rulings

Eliott's rulings on the verdicts report, one row per question and round.

| Round | Question | Ruling |
|:--|:--|:--|
| 03 | Q1 default output format | JSON is the default for every data command |
| 03 | Q2 `--format json` and errors | Errors are JSON by default too; `--format json` has no meaning; only a text option, a pretty print of the JSON |
| 03 | Q3 samples and the five-section rule | Open: Eliott asked why a generated page needs a sample; answered in v1 |
| 03 | Q4 guard for a box lacking a declared server | The box is the author's, but a check after the image build that the box holds what the experiment declares is wanted; fix the border of Candler's responsibility (drafted in v1, to confirm) |
| 03 | Q5 where the estimate prints | Option A: in `resolve` |
| 03 | Q6 Docker cause in `jobs.session_controls_failed` | Defer |
| 03 | Q7 two senses of "stratum" | Align to the standard meaning (option A) |
| 03 | Q8 name of the interval's reading | Option B |
| 03 | Q9 path vocabulary offline | Option A: offline delivery is M7's responsibility |
| 04 | Q3 samples | The field list is enough; X6 is rejected |
| 04 | Q4 border | Asked back: how would the check find servers written in different languages? Answered as Q4c in v2 |
| 05 | Q4c the check's form | Run each declared server's start command; no MCP client. Software the agent uses at run time belongs in the image, and mounting it is the plug-in's fault. Accepted: the check runs once per job before the first run, in a container set up as a run's |
| 05 | N1 plug-in metadata | owner:plugin. Tell the HTN coordinator; plug-ins dogfood the latest Candler instead of reproducing the old harness; the domain prompt is generated, so its values are known |
| 05 | N9b env vars | Wanted; scout first for a duplicate. The scout found none: accept |
| 05 | N8 `CANDLER_STORE` | Accept |
| 05 | N2, N3, N4, N5, N6, N9a | Proposals acknowledged: accept, owner:provider, accept, defer, accept, reject |
| 06 | Go | Go on v3; this session writes the build leaves (`consolidated/build/`, amendment A1) |
| 06 | Q7 word | "comparability class", key `comparability_class` |
| 07 | N6 | Reject: no slash-prompt rule exists on the tip, so none moves into `resolve` |
| 07 | S1, S2 | Not in M6b: a separate PR and a dedicated report set |
| 07 | Build budget | Cap raised from $55 to $90; proceed |

## Status

Verdict study closed: v3 is approved (round 06). The build runs in `__roadmap__/m6b-surface-consolidation/consolidated/build/`: L0 is merged (round 07), L1 is next.
