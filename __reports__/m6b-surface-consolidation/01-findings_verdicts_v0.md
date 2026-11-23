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

## Seams

Each seam the leaves raised, resolved with its reason, or sent to a steering question (Q1 to Q8, at the end). Seams 1 to 7 are the ones the coordinator marked; the table after them lists the rest.

### 1. `config.no_store`: is the refusal ever raised?

Two leaves disagree: `cli_contract` says the code never raises it, `codes_review` says it is emitted at `crates/candler/src/commands/common.rs:103`. Outcome: **it is emitted, and it is reachable, but only when no store root resolves at all.**

- `resolved_root` takes the `store.root` entry from the resolved settings and builds `CliRefusal::NoStore` when none exists (`common.rs:89-104`).
- The core default for `store.root` is `<project root>/.candler/store` when a project file is found, else `<state home>/candler/store` (`$XDG_STATE_HOME`, else `~/.local/state`); "with neither, there is no default" (`crates/candler-core/src/resolve/layers.rs:284-344`; guard `the_default_store_root_follows_the_project_or_the_user_state_directory`, `crates/candler-core/tests/it/guard_resolve.rs:631-689`).
- So the refusal fires when there is no `--store`, no `CANDLER_STORE`, no `candler.toml` found and no state home (no `HOME`, no `XDG_STATE_HOME`): for example a bare container. That is exactly the environment Eliott describes for agents.
- Consequences. (a) The registry row text "Not raised today: a verb given no store root takes the default store" (`crates/candler/src/commands/refusal.rs:233-241`, `docs/contract/index.md:70`) is wrong or at least misleading: X9's finding stands. (b) R01 §CLI surface (`architecture.md:333-335`, "a verb that finds none refuses, naming the three ways") is true in that case, so the R01 drift claim of `cli_contract` is mostly unfounded: do not "correct" R01, reword the registry row. (c) G9 stays `done`; its trap (a mistyped project dir quietly using the default store) is real but is the zero-configuration design, and D1's `store` field in `ps`/`ls` records shows which store answered.
- Whether `Config.state_home` is ever empty on a normal laptop was not read (the leaves did not trace `Config` construction); the claim is "reachable by construction", not "seen".

### 2. G6: is "name the cause in the refusal" covered by G8?

**It is a gap.** G8 fixes one thing: the inner code of a failed experiment or suite file read (`*.document_invalid`, 39 rows, `candler-schemas`). G6's ask is different: when Docker cannot share the store root, a run's session control fails and `jobs.session_controls_failed` (`docs/contract/codes.toml:979-982`) says "No model's behaviour causes this" without the Docker host's cause (`docs/contract/index.md:170`). `codes_review` never mentions G6, and `execution_preflight` handed it to them. What I could check: the refusal row exists (`crates/candler-core/src/jobs/refusal.rs`); whether the refusal message carries the Docker error text was not established by any leaf or by me (one colgrep search found the table only, not the construction site). G6's Future row already says its trigger includes "the cause of `jobs.session_controls_failed` still unnamed after `codes_review`": that half is now true, because the review left it unnamed. Raised as **Q6**; until answered G6 stays `defer`.

### 3. C3, X6, X7, X3 and X11: one generator and one fact source, or two?

What the leaves show, together:

| Item | Fact | Owner of the fact today | Rendered by |
|:--|:--|:--|:--|
| Item C3 | path vocabulary and source forms (`model.id`, `runner.mode`, `treatment:<path>`, settings classes) | `is_known_path` (`layers.rs:60-122`) and `REQUEST_KEYS` (`refusal.rs:201-206`), two hand-written owners | none; proposes one const table plus a page `configuration-paths.md` |
| Item X7 | the five keys of the two config files (`store.root`, `plugins.dirs` in `candler.toml`; `slots.count`, `slots.evaluators`, `gateway.upstream_ca` in the user config) | `PROJECT_TABLES` and `USER_TABLES` (`crates/candler-core/src/resolve/config.rs:13-23`, `:149-161`, `:242-273`), private constants | none; proposes a page `config-file.md` |
| Item X6 | sample files (experiment, suite, plugin manifest, import rules) | fixtures that tests already parse | proposes an `Example` section on the pages |
| Item X3 | exit codes and value domains | `Exit` (`commands/mod.rs:27-39`) and clap | proposes one `EXIT_CODES` constant feeding `--help` and a page row |
| Item X11 | the Python plug-in API | docstrings in `plugin.py` | none (no generator; README rewrite) |

Outcome:
- **C3 and X7 do own one fact.** The configuration keys `store.root`, `plugins.dirs`, `slots.count` and `slots.evaluators` are both "configuration paths" in C3's table (`layers.rs:27-58` names them as settings) and the keys of X7's tables. Built as proposed there would be two tables for the same four keys. Resolution, binding on the build session: **one const table in `candler-core::resolve`, with a `file` column (project, user, none), read by `is_known_path`, `REQUEST_KEYS`, `PROJECT_TABLES` and `USER_TABLES`; two generated pages (`configuration-paths.md`, `config-file.md`) that render different slices of it.** `gateway.upstream_ca` is user-config only and not a settable path; the table needs a class for "config key, not an experiment path". This keeps one fact, one owner, and the generator change (below) is paid once. Build C3 and X7 in one leaf.
- **X6 and X7 and C3 share one generator change, owned by X6.** X6's `Example` section conflicts with `CONTRIBUTING.md:149-160` ("five sections, no other"). The rule change is one amendment (Q3 asks Eliott whether to amend or to avoid the sixth section). C3 and X7 reuse whichever form is chosen; they must not each invent a page shape. X7's Example is the project fixture `observe/ls/candler.toml`; the user config has no tested fixture and gets no Example (build adds none).
- **The generator is test-support code** (`crates/candler/tests/it/support/contract_pages/`), so the interactive surfaces cannot reuse it. None of the accepted items needs it to: C3, X6 and X7 are web-wide pages (the generator is right for them). X3's exit codes are interactive and live in the binary (`after_help`) from one constant that the page generator also reads: that is one source, two readers, and it works because the constant sits in the `candler` crate the test crate already depends on. What no accepted item solves is offline delivery of C3's table to an agent (H3 asked for it through `--json`): that stays D1 and M7's, and no verdict depends on it. State this plainly to Eliott: the path vocabulary will be readable on the web and in the CLI only through refusals and `resolve` until M7 decides.
- X11 shares nothing with the generator (docstrings remain the source), and its README example is the one a test imports.

### 4. D1 with F2, F4, C6, C8, C9 and F5: do the analysis records and D1's schema list agree?

D1 proposes `analysis-report/1` (arms, entries, comparisons, runs, summary), `inspect-run/1`, `inspect-job/1`, small list, diff and rates records, cut after F2 and F4 settle. Checked against the records:

| Record | What it asks of the analysis output | Fits D1? |
|:--|:--|:--|
| F2 accept | `Comparison.levels` in a natural order | Yes: it changes values, not fields; build before the record is cut. |
| F4 reject | `reading` stays the interval's reading, `reject` stays Holm's | Yes: the fields stay as they are. Open: rename `reading` to say it is the interval's (Q8). |
| C6 accept | a list of decisions derived in core, one key of `analyze` | Yes: it is D1's "summary" member of `analysis-report/1`; D1 listed it, so it is one list, not two. |
| C8 accept | `spent` per arm in `analyze`, `spent` for the job in `inspect <job>` | Not in D1's list: D1 names `arm_json` as it is today. Add `spent` (cost sum, runs counted, runs without a total) to `analysis-report/1` arms and to `inspect-job/1`. |
| F5 accept | `analyze --format text` rendered from the record | Yes: needs the record first. |
| C9 accept | transcript of a run | Not in D1's list; see below. |

Therefore **the record is cut once, after F2, C6 and C8 are built** (build levels L0 then L2, below); cutting it earlier forces a `/2`. The inspect records and the small records can go first (L1).

**C9's verb.** `analysis_outputs` left it to `cli_contract`, which does not mention C9. Outcome: **`inspect <run> --transcript`.** `show` takes no run (`show.rs:11-27`), `inspect` is the per-run verb (`inspect.rs:205-275`), and a new verb would break "every new option must say why" for no gain. The flag chooses what is shown, not the format, so D1's `--format` applies: `json` prints the events as the stored normalised trace (one JSON object per line, as `watch` does), `text` one line per event. What no leaf checked: whether the normalised trace is already a `candler-schemas` type (the registry has bare `[trace]` and `[frames]` sections, `codes.toml:786,1325`, which suggests a trace type exists in `candler-core`); if it is not a schema type, the transcript's JSON becomes a record in `candler-schemas` (a small addition to phase two). The build session's first read decides; not guessed here.

### 5. "resolve is not a plan" against `PlanMode::Check`

Adjudication in one line: **the idiom is "the experiment is never called a plan" (R01), not a ban on computing a plan**; `PlanMode::Check` is the core's no-lock planning pass (`crates/candler-core/src/runner/plan.rs:83-92`) and `run` already uses it (`run.rs:1-14`). C1's accept prints cells, new and reused runs from `PlanMode::Check` and says "the experiment stays an experiment", so it does not call the experiment a plan. Two cautions for the build, both for **Q5**: (a) `resolve` stops before planning today (`resolve.rs:1-12`), so adding planning output to it joins two operations in one verb; (b) planning keys cells by resolved identity, which includes the environment image the Docker step resolves, so a printed plan from `resolve` probably needs Docker. Not read; if true, C1 does not create the no-Docker path that C5's rejection forbids, and it does not need one.

### 6. Docs drift raised by the leaves

All in one table. Owner is the build session's docs leaf unless stated; none needs a test (a test per stale sentence is a test per incident, AGENTS.md fail list).

| Where | Drift | Fix | Owner |
|:--|:--|:--|:--|
| `architecture.md:326` | says `resolve <experiment \| suite>`; the code takes an experiment only (`resolve.rs:39-52`) | edit R01 | docs leaf |
| `architecture.md:328` | says `analyze` finds the experiment by digest; the code reads `--experiment` only (`analyze.rs:77-127`) | edit R01 | docs leaf (X8) |
| `architecture.md:1335` and `:1757` | one says a paired-design Future row exists (none does), the other that "M6a builds the paired tests" | drop `:1757`'s clause; add the row with trigger "a design that pairs runs" or drop `:1335`'s clause | docs leaf (X8) |
| `docs/statistics.md:165` | "Paired tests are not offered yet" promises what R01 rules out | say runs are not paired so a paired test does not apply | docs leaf (X8) |
| `docs/statistics.md:80`, `:166-167` against `blocking.rs:29-38` | "stratum" means a comparability class in the page and one block of a comparison in the code | not a docs-only fix: **Q7** | steering |
| `python/candler-evaluator/README.md:5` | "M2a fills it in" | rewritten by X11 | X11 |
| R09 gap 2 (and the README Gotchas) | places `slots.*` and `gateway.*` in `candler.toml`; they are user config (`config.rs:19-23`) | not a repo file: X7's page states it; no edit | none |
| `README.md` | "Csv and Table" as two formats; `Table` is a sub-option of `--format csv` (`analyze.rs:61-75`) | the leaves' and the campaign's wording only; the repo README never says it | none |
| X1 input | "resolve prints nothing on success"; it prints values with sources and has `--json` (`resolve.rs:39-52`, `288-298`) | none (an input, corrected in the X1 row) | none |
| `docs/contract/index.md:70` | `config.no_store` "Not raised today" | X9, row text in `refusal.rs:233-241` (seam 1) | X9 |
| `architecture.md:333-335` | none after seam 1: R01 is true when no root resolves | leave | none |
| 21 `[checked: v@...]` stamps | 10 at 0.6.0 and 11 at 0.7.0 against core 0.13.0 | restamp only after a real re-read | X9 |

### 7. Verbs `cli_contract` did not examine

`cli_contract` examined the args only of `run`, `kill`, `ingest`, `evaluate`, `show`, `diff`, and not the row types of `show`, nor whether `watch` prints the committed `event-envelope`. **D1's phrase "every data verb" therefore covers, with evidence: `analyze`, `inspect`, `resolve`, `ps`, `ls`, `diff`, and `show --rates`. It does not cover `run`, `kill`, `ingest`, `evaluate`, `show`'s row types or `watch`'s event envelope; for those the report says "unknown" and does not guess.** They appear as a gap in the design suggestions (`evaluate`'s report is the one most likely to be a data document: the leaf's own table says `evaluate` "prints the report" and that its shape was not examined).

### The other seams

| Seam | Outcome |
|:--|:--|
| C1, C4, C5 on `resolve` | C4 `done`: `resolve` is already the explain (`resolve.rs:150-179`). C5 `reject`: no skip-Docker switch. C1 adds a derived summary, no option, no new validation path; whether it lives in `resolve` is Q5. They do not conflict. |
| C3, X7 and the plug-in facts (X11) | C3 and X7: one table, seam 3. X11 is docstrings and a README; no overlap with the table. |
| G5 and G6 with the registry verdict | G5 is a preflight check id (`<step>.<check>`, `resolve.rs:1-12`), not a registry code, so `codes_review`'s note that a new pre-spend refusal is a new row does not apply to G5: no `codes.toml` row; the run-time `IsolationError::Plan` row (`isolation/refusal.rs:53-340`) stays. G6: seam 2. |
| X11 with X6 | The helper README's example is the file an engine fixture imports (`engine_second.py:3`); X6's plug-in manifest sample is a different fixture (`plugins/needs-inputs/plugin.toml`). No shared fact. |
| X6 import-rules sample | No page documents `ingest --rules` (`ImportRules`, `import/rules.rs:93-117`); the sample has no home. Left to the build session with `codes_review`'s count of `config.import_*` rows; not decided here. |
| C8 with `execution_preflight` C1 (H6 split) | One fact at two times: C1 estimates from `PlanMode::Check`, C8 sums sealed manifests. The live meter stays untouched (`schedule.rs:128-136`); C8 says how many runs its sum covers, so the two never need comparing. A cost-from-history estimate waits on C8's record and has no verdict of its own. |
| C2 with `codes_review` and `analysis_outputs` | Deferred, so no code is added now. A census mismatch would flag a run and enter the integrity record, which would be a new code; Q4 asks whether to build it now. |
| G8 with `docs_and_skills` | After the G8 build, "match on side plus path, not on code" in any skill is false (`codes_review`); M7's skill text must be written after, not before. |
| C7 with D1 | A `check` verb would be a new output type under D1. Deferred. |
| G21 with the image identity row | `gc` waits on `architecture.md:1773`. |
| H5 and H6 splits | H5: F2 gives labels and order, C6 the list, F5 the text; F4's Holm half is answered by C6's headline being `reject`. H6: C1 then C8; no stitched feature: the estimate and the sum are two operations that read different data. |
| Covered id G3 | A covered id with two primaries that disagree: accept for orientation (F2), the Holm half rejected (F4). |

## Triad over the accepted set

The nine cells summed over the 19 accepted records (S sustainable, Sc scalable, E efficient; each record has a +, 0 or minus per cell).

| | Development | Codebase | Consumption |
|:--|:--|:--|:--|
| S | 8 plus, 11 zero | 12 plus, 7 zero | 19 plus |
| Sc | 7 plus, 12 zero | 11 plus, 5 zero, 3 minus | 13 plus, 5 zero, 1 minus |
| E | 6 plus, 12 zero, 1 minus | 1 plus, 18 zero | 17 plus, 2 zero |

No cell has more minus than plus; no accepted record is net negative on consumption. The weakest cells are codebase scalability (3 minus) and development efficiency (1 minus).

Compromises taken, each where two cells conflict:

| Where | Conflict | Winner and why |
|:--|:--|:--|
| X1 default flip (`resolve`, `ps`, `ls` to JSON by default) | codebase and consumption gain; a person at a terminal loses | consumption, because stdout's reader is a program; reversible by one default. Q1 asks Eliott. |
| F2 natural order of levels | a stable sign gains; the experimenter's declared order is not used | stability, because `analyze` runs without the experiment file (`analyze.rs:25-46`); C6's labels make the order harmless |
| G8 inner code as `code` | consumption gains; codebase pays to carry the typed error through serde (post-parse validation) | consumption; post-parse validation moves the existing validator, it adds no second layer |
| X6, X7 generators reading fixtures and private constants | codebase scalability minus (visibility changes, a docs page reads a test fixture) | no copy; a hand copy would be a second source |
| X11 README repeats one signature as an example | consumption gains over a pure no-repeat rule | an example a test imports cannot rot silently |
| X4 and X1 cost (about 105 Sonnet calls together) | development efficiency | one record per output instead of `json!` growth; cost paid once |
| C8 stored sum beside the live meter | two numbers for one fact | the sum is computed from sealed manifests only and states its coverage |
| C9 no paging | a long transcript is large | one line per event, a consumer may stop reading |
| X3 narrow form (no examples in `--help`) | an example helps consumption | an example is hand prose in a second place; exit codes and domains come from one constant |
| X9 no hand restamp | stamps stay old | a stamp is a record of a check; restamping without checking asserts a check nobody made |

## Blast radius

Over the 19 accepted records (the others change nothing, by definition).

### Crates and modules

| Crate or area | Touched by | Notes |
|:--|:--|:--|
| `candler-core::analysis` (`report.rs`, new `summary.rs`, `spend.rs`) | F2, C6, C8 | `analyze` never writes the store (`analyze.rs:1-20`) |
| `candler-core::runner::plan`, `resolve::steps::identity`, `resolve::layers`, `resolve::config` | C1, G5, C3, X7 | `layers.rs`/`config.rs` gain one table |
| `candler-core::trace` | C9 | one render function |
| `candler-harness::skills` | G5 | names the missing path |
| `candler-schemas` (`execution.rs`, `evaluation.rs`, `refusal.rs`, `refusal_impls.rs`, new `analysis.rs`, `inspect.rs`, `generate.rs`) | G8, X4, X1 | the one crate whose releases move |
| `candler` (`commands/*`, `main.rs`, new shared `OutputArgs`) | X1, X10, X4, F5, C9, X3, C1 | every data verb's `Args` |
| `candler/tests/it/support/contract_pages`, `docs/contract/`, `CONTRIBUTING.md`, `README.md`, `docs/statistics.md`, `python/candler-evaluator/README.md`, `__design__/architecture.md` | C3, X6, X7, X3, X9, X5, X8, X11 | docs and generator |

### `schemas@` releases

| Release | What lands | Reason |
|:--|:--|:--|
| 1 | G8: the mapping from variant to wire `code` changes for the same invalid file (`experiment.document_invalid` to `experiment.factor_without_levels`); `path` and `side` stay | a behaviour change on the wire; `codes.toml` rows unchanged |
| 2 | D1 phase one: `analysis-report/1`, `inspect-run/1`, `inspect-job/1` (including C6's summary and C8's `spent`) | additive files under `crates/candler-schemas/schema/`, blessed by `schema_drift` |
| 3 | D1 phase two: the small records (job summary list, experiment listing, diff, rates), plus C9's trace record if the trace is not already a schema type | additive |

G8 and phase one could share a release if they land close; they are different leaves at different levels, so plan for three and treat two as the saving. **Hidden release to watch:** if the analyze record is cut before F2, C6 and C8 are built, it needs a `/2`; the level order below prevents that. C1, G5, C3, F2, C6 (as a core struct), C8, C9 (core render), X3, X5 to X11, X9 and X10 need no release on their own.

### `codes.toml`

| | Count | Detail |
|:--|:--|:--|
| Rows added | 0, possibly 1 | G5, C1, F2, C6, C8 add none; G5 is a check id. C9 may add one "run without a trace" refusal if the build finds the registry has none (to be checked) |
| Retired | 0 | seam 1 says the `config.no_store` variant is live; do not retire |
| Lifted | 0 | |
| Text edits | 1 row text, 2 Notes lines | `config.no_store` row text (`refusal.rs:233-241`); `experiment-file.md:108` and `suite-file.md:66` for G8, with new stamps |
| `remedy` key | 0 | gate G3 stays true |

### Guards that change, and digests at risk

| Guard | Changed by | How |
|:--|:--|:--|
| `guard_golden_design.rs:402-433`, `guard_analysis_report.rs:1428-1514`, `guard_known_truth.rs:581-724`, `guard_analyze.rs:1252-1312` | F2 | re-pinned once: which level is A |
| U1, U2 in `guard_cli_surface.rs` | X1, X10 | the verb and option matrix gains `--format`; U2's stamp is already the text on `show` and `diff` |
| `d1_regenerating_the_pages_changes_nothing` (`contract_pages.rs:73-102`) and the section-order check (`:173-234`) | C3, X6, X7, X3 | pages added to the table; a sixth section allowed if Q3 says so |
| every test asserting `document_invalid` on a value-invalid file | G8 | expectation changes (the defect) |
| `schema_drift` | X4, X1 | blesses the new files, no new code |
| `e2_slots_evaluators_is_a_known_path` (`guard_resolve.rs:788-797`) | C3 | stays; the table feeds the same predicate |

Digests: none moves. Outputs of `analyze`, `inspect` and the CLI are not hashed; cell and run digests come from manifests, which no accepted item changes (C8 reads manifests; C2's census check, deferred, would have been the one near `comparable`). The ordering by natural level in F2 changes which level is called A, not any digest. GOLDEN: the 240-run golden parity compares outcome rows (`architecture.md:29`), untouched; only the three sign-pinning guards above move.
