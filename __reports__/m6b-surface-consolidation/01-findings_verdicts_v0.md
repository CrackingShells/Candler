# M6b surface consolidation: verdicts (findings, v0)

Date: 2026-11-23

---
type: findings
topic: m6b_surface_consolidation
date: 2026-11-23
version: v0
prior-version: none
key-metric: inputs with an explicit verdict: 63 of 63 (prior: N/A, delta: N/A)
decision-required: confirm
---

## Headline Result

metric: inputs judged (63) and what Candler is asked to build
value: 19 accept, 7 defer, 9 done, 4 m7, 1 owner:consumer, 1 owner:provider, 6 reject (47 primary records, 14 covered ids, 2 split ids H5 and H6)
unit: ids (primary records counted once; covered and split ids follow their primaries)
prior: N/A
direction: new

Counts above are over the 47 primary records. Over the 63 rows of the table below, which add the 14 covered ids and the two split ids, the tally is 25 accept, 14 defer, 10 done, 4 m7, 1 owner:consumer, 2 owner:provider (G4 follows C10), 7 reject.

Decision wanted from Eliott: the eight steering questions at the end, then Go or amend. Nothing is built until Go.

## How to read this report

Eliott's round-02 rulings bind every row (items are inputs, not a backlog; Candler's idioms; no user-behaviour baseline; no refusal remedy hints; docs are the web-wide surface while the CLI, MCP and skills are self-contained and offline; M7's MCP shape is decided at M7 planning). Verdicts: `accept` (Candler owns it and the build session builds it), `defer` (Candler owns it, waits for an evidence trigger), `m7`, `owner:<who>`, `reject`, `done` (already true on the tip). Tip judged: `a0df4d48`. The leaf reports under `leaves/` hold each record in full (live on the tip, the ask questioned, responsibility, blast radius, triad, idioms); this report folds them, resolves the seams between them and adds what no leaf could see alone.

## Verdicts

| Id | Leaf | Verdict | Folded into | Reason |
|:--|:--|:--|:--|:--|
| G1 | knowledge_as_data | owner:consumer | - | Choosing the catalog is the experiment author's call; Candler derives, records and gates it (catalog digest, census, stratum refusal are live). Premise "snapshots or inputs unsupported" is half wrong: only snapshots. |
| G2 | execution_preflight | done | - | Treatment-as-factor is refused and differing `eval_infra`/`sandbox_contract` strata are refused by design (`compare.rs`). |
| G3 | analysis_outputs | accept | F2 (accept), F4 (reject) | Orientation half accepted via F2 (A and B follow a natural order of the levels, not the cell digest). The Holm half is rejected via F4 (`reading` and `reject` answer two questions) and handled by C6's decision list. |
| G4 | analysis_outputs | owner:provider | C10 | RiVault's cache and its cost counting are the provider's; the 2026-11-09 ruling stands (no workaround in Candler). |
| G5 | execution_preflight | accept | - | The treatment step pins a commit and never checks that the skill subtree exists in it; add the check as a preflight check id (no `codes.toml` row). |
| G6 | execution_preflight | defer | - | A bind-mount probe is a platform special case on one incident. Naming the cause in `jobs.session_controls_failed` is not covered by G8 (see Seams); first-pass Future row, trigger now half met. |
| G7 | cli_contract | defer | C13 | A plug-in check verb adds little over the checks `evaluate` runs; author traps are page content. Future row. |
| G8 | codes_review | accept | - | 39 caller codes are shadowed by `*.document_invalid`; make the inner code the printed `code` (one `schemas@` release). |
| G9 | cli_contract | done | - | Store root fallback is sourced (I12) and `config.no_store` is raised only when no root resolves at all (see Seams, seam 1); R01's sentence is nearly right. |
| G10 | execution_preflight | done | - | Fixed-value traps are refused or recorded loudly; the image-id trap already has a Future row. Two residuals unread. |
| G11 | cli_contract | done | - | `--error-format` text by default is R01's deliberate rule; one skill sentence is M7's. |
| G12 | cli_contract | reject | C14 | Inferring import rules would put domain strings in the core (I1) and fail silently into an append-only store (I11). |
| G13 | analysis_outputs | defer | - | Already a Future row (`architecture.md:1800`); trigger not met; widen it with double grading. |
| G14 | execution_preflight | done | - | Detached supervisor with no daemon is R01's design (I16); consumer sentences are docs. |
| G15 | execution_preflight | done | - | Isolation facts are as designed and open residuals already carry rows. |
| G16 | docs_and_skills | done | - | Pages and schemas are generated under drift tests; residual hand copies go to X5, X8 and X11. Its "skills link the pages" cure is overruled by R04. |
| G17 | knowledge_as_data | done | - | Policy already in `CONTRIBUTING.md:20,82` (`:75` the bump, minor while 0.y.z): pre-1.0 breaking changes carry `!` or a BREAKING CHANGE footer; the four unmarked M2c changes are past misses the supervisor accepted, with no history rewrite; the version table (`docs/contract/index.md:30-36`) is correct. |
| G18 | knowledge_as_data | defer | C7 | The commit is recorded and gates strata; only an errata list is missing, with one occurrence so far. |
| G19 | knowledge_as_data | defer | C7 | Candler records the alias; whether the stream carries the backing model is unchecked. |
| G20 | knowledge_as_data | accept | C3 | The path vocabulary has two hand-written owners and no page; one table plus one generated page. |
| G21 | execution_preflight | defer | - | No Docker lock or `gc` is needed by Candler's own runs; `gc` waits on the image identity row. Future row. |
| C1 | execution_preflight | accept | - | Cells, new and reused runs are already computed by `PlanMode::Check` and printed nowhere; print the summary, no new verb or option. |
| C2 | execution_preflight | defer | H1 | The probe verb is rejected by ruling; the real gap (no comparison of declared servers with the census) waits for an evidence trigger. Future row. |
| C3 | knowledge_as_data | accept | - | One const table of paths in `candler-core` read by the code, rendered as one generated page under the existing d1 guard. |
| C4 | execution_preflight | done | H2 | `resolve` already prints each value with its source (`derived {step}`); JSON form is X1's. |
| C5 | execution_preflight | reject | - | Shape, unknown fields and factor rules already run before any Docker step; a skip switch is a second validation path. |
| C6 | analysis_outputs | accept | - | A list of decisions derived in core from `Report` (one per tested entry: labels, B minus A, interval, `p_holm`, `reject`, family size, omitted reasons); no job state. |
| C7 | knowledge_as_data | defer | H4, G18, G19 | Reported model and errata registry: one occurrence and an unchecked stream. Future row. |
| C8 | analysis_outputs | accept | - | Candler sums the per-run costs it already records, counting runs covered and runs without a total; tokens are a Future row. |
| C9 | analysis_outputs | accept | H7 | The normalised trace is already stored at seal time; print it with one renderer. Verb: `inspect <run>` (see Seams, seam 4). |
| C10 | analysis_outputs | owner:provider | G4 | A flag cannot tell a cache hit from a deterministic agent; the ruling stands. |
| C11 | cli_contract | reject | - | Remedies are ruled out; valid values as facts already live in messages where useful; a field would change `refusal/1`. |
| C12 | knowledge_as_data | reject | - | A scaffold is the experiment file written through flags; tested samples (X6) meet the need. |
| C13 | cli_contract | defer | G7 | `check_suite` exists in core; exposing it waits for a lost job start. Future row. |
| C14 | cli_contract | reject | G12 | See G12. |
| C15 | execution_preflight | defer | G21 | See G21. |
| H1 | execution_preflight | defer | C2 | See C2. |
| H2 | execution_preflight | done | C4 | See C4. |
| H3 | knowledge_as_data | accept | C3 | See C3; serving it as `--json` is a delivery question (D1, M7). |
| H4 | knowledge_as_data | defer | C7 | See C7. |
| H5 | analysis_outputs, cli_contract | accept | F2, F4, C6, F5 | Split: label half F2 (accept), Holm-headline half F4 (reject, the headline is `reject` through C6), summary half C6 (accept), text half F5 (accept). Net: the summary is built from F2, C6 and F5. |
| H6 | execution_preflight, analysis_outputs | accept | C1, C8 | Split: estimate half C1 (accept), spend half C8 (accept). Two times of one fact; the estimate from history waits on C8's record. |
| H7 | analysis_outputs | accept | C9 | See C9. |
| H8 | execution_preflight | defer | G21 | See G21. |
| H9 | docs_and_skills | m7 | - | Skill content is M7's to shape; fixtures are X6's. |
| H10 | docs_and_skills | m7 | - | Checklist wording is M7's; the facts it needs come from C3. |
| H11 | docs_and_skills | m7 | - | `side` in the registry is the map; M7 derives its notes from it with no copy. |
| H12 | docs_and_skills | m7 | - | CLI half is D1; the MCP half is M7's shape. |
| F2 | analysis_outputs | accept | - | A and B depend on cell digests today; order the levels by a natural order in core (no option). |
| F4 | analysis_outputs | reject | - | `reading` is the interval's and `reject` is Holm's; merging states an interval fact from a p-value. |
| F5 | cli_contract | accept | H5 | `analyze --format text` rendered from the record, after X4, F2 and C6. |
| X1 | cli_contract | accept | - | One `--format` option (json or text) with one default across the data verbs (D1); the premise "resolve prints nothing" is wrong. |
| X2 | execution_preflight | reject | - | `evaluate` waits so its report is one stdout document; `run` returns an id because it is long. Designed in R01. |
| X3 | cli_contract | accept | - | Narrow form: exit codes and value domains from one constant feeding `--help` and the page; examples are rejected. |
| X4 | cli_contract | accept | - | `analysis-report/1` and `inspect-*` records in `candler-schemas` under `schema_drift`; the R01 Future trigger is met. |
| X5 | docs_and_skills | accept | - | `README.md` becomes the store window (60 to 80 lines); inventories leave it. |
| X6 | docs_and_skills | accept | - | Samples come from fixtures tests already parse, as an `Example` section; needs a `CONTRIBUTING.md` rule change. |
| X7 | docs_and_skills | accept | - | One generated config page over the two files (project `candler.toml`, user config); `slots.*` and `gateway.*` are user config. |
| X8 | analysis_outputs | accept | - | Correct `docs/statistics.md:165` ("not offered yet") and R01 `:328`, `:1335`, `:1757`. |
| X9 | codes_review | accept | - | Fix the `config.no_store` row text; stamps are a record of the last check and are re-stamped only after a real check. |
| X10 | cli_contract | accept | - | Dissolved by D1: `show` and `diff` get the default format. |
| X11 | docs_and_skills | accept | - | Docstrings stay the source; rewrite the helper README as an author page with one tested example. |
| X12 | docs_and_skills | defer | - | A hand glossary is a second source; the real defect is the word "stratum" in two senses. Future row. |

Where this table differs from a leaf's wording: G9's reason is corrected by seam 1; G3 is a covered id whose two primaries disagree (F2 accept, F4 reject), judged accept for the orientation half only; H5 and H6 are the two split ids.
