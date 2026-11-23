# Docs and skills verdicts (leaf `docs_and_skills`, v0)

Tip judged: `a0df4d48` (core v0.13.0, schemas@0.11.3, candler-evaluator@0.1.0; `Cargo.toml:6`, `crates/candler-schemas/Cargo.toml:4`, `python/candler-evaluator/pyproject.toml:3`). Eliott's rulings (R04 round 02) and D2, D3 bind. Code was read through colgrep; paths below are the same files in this worktree.

## Documents today

Audiences: **S** store window, **W** web-wide (`docs/`), **I** interactive (CLI, MCP, skills: self-contained, offline), **D** development.

| Document | Audience | What it holds | Generated or hand-written | Drift test |
|:--|:--|:--|:--|:--|
| `README.md` (31 lines) | S, but also D | one-line identity and the two halves (`README.md:3-8`); crate inventory and verb list (`README.md:16-21`); links to the design doc and the roadmap (`README.md:12`), `CONTRIBUTING.md`; no link to `docs/`, no install or quick start | hand | none |
| `docs/contract/index.md` and 8 pages | W (reader: a coding agent, tables and rules, `CONTRIBUTING.md:149`) | Surface, Version, Fields, Refusals, Notes per surface; compatibility table; refusal table | tables generated from the schema files, the manifests and `codes.toml`; Surface and Notes by hand | `crates/candler/tests/it/contract_pages.rs:73` (d1: regenerate, no byte changes), `:236` (d4: versions equal manifests) |
| `docs/contract/codes.toml` | W, machine-readable | the refusal registry (`side`, append-only, never renamed) | hand, append-only (`codes.toml:3-5`) | judged by `codes_review`, not here |
| `docs/statistics.md` (195 lines) | W | what each default method answers, where it stops being valid, what Candler does not claim | hand | none on the prose; the numbers are guarded elsewhere (`guard_golden_design.rs:1-23`) |
| `crates/candler-schemas/schema/*.json` and its README | W, machine-readable | JSON Schema of every wire type | generated from the structs | `crates/candler-schemas/tests/it/schema_drift.rs:14` |
| `candler --help` (clap) | I | one-line verb help, flag help; exit codes only in the `main.rs` module doc (`crates/candler/src/main.rs:1-33`, `commands/mod.rs:27-39`) | hand (doc comments) | none found |
| Python helper docstrings: `plugin.py` module, `RunContext`, `Plugin` | I (travels with the package, readable by `help()`) | the kind type `Kind` (`plugin.py:24`), what a kind reads (`plugin.py:90-121`), `KINDS` and the optional `NAME`, `VERSION`, `DIST`, `INSTRUMENTS`, `KIND_SPECS`, `DETERMINISTIC` (`plugin.py:124-160`) | hand | the example `examples/synthetic_plugin/synthetic_plugin.py` is imported by an engine fixture (`crates/candler-core/tests/fixtures/engine/kinds/engine_second.py:3`) |
| `python/candler-evaluator/README.md` | D (maintainer) | regenerate `_models/`, versions; line 5 still says "M2a fills it in" | hand | none |
| MCP server, skills | I | do not exist yet (M7) | n/a | n/a |
| `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md` | D | rules for agents and contributors; `CLAUDE.md` is a pointer to `AGENTS.md` | hand | none |
| `__design__/architecture.md` | D | layers (`architecture.md:102-115`), workspace, invariants, Future features (`architecture.md:1752`) | hand | none |

Two facts the table shows. The generator of the contract pages is test-support code (`crates/candler/tests/it/support/contract_pages/index.rs:11`, `pages.rs:28`), not part of the binary, so the pages and the interactive surfaces have no shared producer. And `docs/` has no entry page of its own: the only index is `docs/contract/index.md`, and nothing links to it from `README.md`.

## D2

**Question.** Is `README.md` the store window: what Candler is, its main features, a quick start, where to go for more?

| Store-window part | On the tip | Verdict |
|:--|:--|:--|
| What it is | `README.md:3-8`: the one-line identity, the two equal halves, the thin surfaces, the domain rule. Good, keep. | holds |
| Whether it fits me | not said: no "who it is for", no "what it is not" | lacks |
| Main features | spread over the crate list; a visitor reads six crate names, not capabilities (a matrix of runs, isolation, trace readers, evaluators, statistics) | lacks |
| Quick start | none. Nothing is published (`README.md:23`), so the honest quick start is a source build and one command that spends nothing. The sample to run must exist: see X6 | lacks |
| Where to go for more | no link to `docs/contract/index.md` or `docs/statistics.md`; the links it has go to the design doc and the roadmap (`README.md:12`) | lacks |
| Crate inventory (`README.md:16-21`) | development-level: the workspace layout is `architecture.md` §Workspace. It is also a hand copy of what `Cargo.toml` members and the clap verbs own, with no guard | belongs to D, and is a second source |
| Status paragraph | the plan and roadmap links are D. "Built / not published" is a status fact a visitor needs, one line | trim |

`AGENTS.md` and `CLAUDE.md` stay development-level, as Eliott said: they inform an agent that develops Candler and are not an entry point for a consumer. `CLAUDE.md` pointing at `AGENTS.md` is right for that audience. The inventory's complaint (R09 §1) that a consumer agent landing in the repo gets contributor rules is correct on the tip, and the fix is the store window plus a link, not a change to the rules.

**Verdict: X5 accept.** `README.md` becomes a store window of about 60 to 80 lines: what it is and who it is for, six to eight features in consumer words, a quick start, a "Learn more" block (`docs/contract/index.md`, `docs/statistics.md`, `candler --help`), and one line each for status and contributing. The crate inventory and the verb list leave `README.md`. No test: a hand-written page with no copy of a code fact has nothing to drift, and the quick start's sample is a file a test already parses (X6).

## Responsibility record (draft)

Text proposed for R01 (a section of §Layer responsibilities, or a new §Documents and surfaces).

> **Four audiences, one source per fact.** Candler's text is written for four audiences. Each fact has one source, and every other place that shows it is generated from that source or does not show it.
>
> | Audience | Serves | Files | May hold | May not hold | Source of truth |
> |:--|:--|:--|:--|:--|:--|
> | Store window | a person or agent deciding whether Candler fits | `README.md` | what Candler is and is not, its main features, a quick start, the way to more | crate or verb inventories, version numbers, plans, anything a code fact owns | hand-written, short, with no copy of a code fact |
> | Web-wide | a visitor who reads, with internet, what Candler specifies | `docs/` (contract pages, statistics, samples, config pages, glossary), `crates/candler-schemas/schema/` | reference: fields, rules, refusals, versions, method and caveats, worked samples | design rationale, roadmap, contributor rules | tables and samples generated from schemas, manifests, `codes.toml` and fixtures that tests parse, under a drift test; hand text only where the generated parts cannot speak (Surface, Notes, method prose) |
> | Interactive | an agent or person using Candler in an isolated environment, offline | the CLI (`--help`, refusals, JSON), the MCP server, the skills, the package docstrings | everything the caller needs to act: values, domains, exit codes, examples, the same facts the web pages show | a link to `docs/`, a URL, a pointer that needs a network; a copy of a fact kept by hand | the binary and the package: the same schemas and registry the pages are generated from, so both reach the same bytes |
> | Development | an agent or person who changes Candler | `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, `__design__/`, `__reports__/`, `__roadmap__/` | rules, rationale, plans, history | a consumer entry point; a claim a consumer relies on and no guard checks | hand-written; a rule that a test can hold is held by a guard |
>
> **The offline rule.** The interactive surfaces (CLI, MCP, skills) are self-contained and work with no network: they never link to `docs/` and never need it. What a page of `docs/` says that an interactive caller needs must reach that caller from the same source, through the binary or the package. This rule requires of the MCP server only that its content is served from what it ships; the shape of its tools and resources is decided at M7 planning.
>
> **Consequences.** (1) A hand-written copy of a code fact, in any audience, is a second source: remove it or generate it. (2) A page never says in prose what a guard or a refusal already says. (3) Web pages may link each other and the repository; interactive text may not link them.

Not decided here: how the interactive surfaces get the generated content (the generator is test code today, see `## Seams`).

## Verdicts

Triad tables: rows sustainable (S), scalable (C, for "one source per fact"), efficient (E); columns development (Dev), codebase (Code), consumption (Use).

### X5: README.md as the store window
- **Verdict:** accept
- **Covers:** none
- **Live on the tip:** yes. `README.md:3-8` is a good identity paragraph; the rest is a crate and verb inventory (`README.md:16-21`), links to the design doc and roadmap (`README.md:12`), no quick start, no link to `docs/`. Not the "nothing is implemented yet" of the v1 review: `README.md:14` says the schemas, evaluation and execution are built.
- **The ask questioned:** the premise holds, and the real problem is two audiences in one 31-line file. See `## D2`: five store-window parts are missing or thin, and the inventory is development-level and unguarded.
- **Responsibility:** Candler's, store-window audience; README says why the repo exists (a domain-agnostic core), so the page must say fit and features, not layout.
- **Blast radius:** `README.md` only. No crate, no `schemas@` release, no `codes.toml` row, no guard, no digest. A quick start names one sample file (X6).
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | + one place to keep the crate list: `architecture.md` | 0 | + a visitor decides in a minute |
  | C | + inventories leave a file with no guard | 0 | + one entry, links out |
  | E | 0 | 0 | + fewer reads to reach the right page |
- **Compromise:** none.
- **Idioms:** follows one source per fact (R01 Layer responsibilities, AGENTS.md "scalable"); follows the four-audience rule. Breaks none.
- **Reason or owner:** Candler owns the store window; Eliott defined its role.
- **Design suggestion:** rewrite `README.md` to 60-80 lines: what and for whom; what it is not; features in consumer words; quick start (build from source, then `candler resolve` on the sample of X6, which per R07 prints values and starts no job; the builder checks once by hand that `resolve` needs no Docker, and words the quick start to match); Learn more (`docs/contract/index.md`, `docs/statistics.md`, `candler --help`); one line each on status and contributing. Drop the crate inventory and the verb list (they stay in `architecture.md` §Workspace and in `--help`). No test (D3: nothing generated, no copy of a code fact). About 6 Sonnet calls.

### G16: contract pages and the registry are the only agent-facing truth; R01 and README lag
- **Verdict:** done
- **Covers:** none
- **Live on the tip:** no for the symptom, in part for the cause. The README no longer says "nothing implemented" (`README.md:14`). The contract pages are generated and held by a drift test (`crates/candler/tests/it/contract_pages.rs:73`, `:236`), as is the schema set (`schema_drift.rs:14`). Three hand copies of code facts remain: the README crate and verb lists (`README.md:16-21`), `python/candler-evaluator/README.md:5` ("M2a fills it in", roadmap talk the source-files rule of AGENTS.md forbids in code and that is stale here), and the hand Notes beside generated tables, which carry per-note stamps (`docs/contract/experiment-file.md:106-110`, `[checked: v@0.7.0]` against core v0.13.0) that no guard reads. The stamps are a record of the last check, not a claim about the version, so they are not a lag.
- **The ask questioned:** the premise (docs lag) was true when written and is mostly closed. The prevention clause ("the MCP server serves `docs/contract/*.md` as resources; skills link, never restate") is overruled by Eliott: skills do not link, and web docs are not the interactive source. The idiom that fits, a generated page under a drift test, is already in place for the pages.
- **Responsibility:** Candler's; development owns the guard, store window and web-wide own their text.
- **Blast radius:** none for the verdict. The residual copies are removed by X5 (README lists) and one line in `python/candler-evaluator/README.md` (a build-session chore inside X5's session, no id of its own); `docs/statistics.md:165` is X8's.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | + guards already hold the pages | 0 | + pages cannot drift from schemas |
  | C | + one source (schemas, `codes.toml`) | + one generator | 0 |
  | E | 0 | 0 | 0 |
- **Compromise:** none.
- **Idioms:** follows "facts for agents are generated under drift tests (`contract_pages`, `schema_drift`)".
- **Reason or owner:** already true on the tip; residuals routed to X5 and X8 above.
- **Design suggestion:** none (not accept).
- **Future row:** none (not defer).

### H9: experiment-design guidance and worked fixtures
- **Verdict:** m7
- **Covers:** none
- **Live on the tip:** no skill exists. One worked experiment exists as a parsed fixture (`crates/candler/tests/fixtures/golden_design/experiment.toml`, read by `guard_golden_design.rs:127-190`), and a domain-free one (`observe/ls/experiments/a.toml`, read by the `ls` guard `guard_cli_observe.rs:486`).
- **The ask questioned:** two asks in one. "A skill that walks question, factors, arms, controls, n" is skill content: interactive, so self-contained, and M7's to shape. "More worked fixtures, one per common shape" has one consumer's evidence (HTN) and is X6's question. A web page may hold what an experiment is (the experiment-file page); the walk-through belongs to the skill, so there is one owner, not a skill and a page that copy each other. "n" guidance (R07 asks that n=3 warns of a ±28 % interval) touches `docs/statistics.md:163` ("No power analysis is made"); that is a statistics claim, not skill prose.
- **Responsibility:** the walk is the skill's (M7). Fixtures are Candler's through X6.
- **Blast radius:** none now; M7 skill text.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | 0 | 0 | + a first design takes fewer tries |
  | C | − a skill that restates the pages would be a second source | 0 | 0 |
  | E | 0 | 0 | + fewer failed runs, each of which spends tokens |
- **Compromise:** Scalable (no copy) wins over a richer skill: M7 writes the walk by pointing at facts it serves from the binary, not copying the page.
- **Idioms:** follows the four-audience rule and one source per fact; "no user-behaviour baseline" means no claim that agents fail without it.
- **Reason or owner:** M7's planning owns the skill content; X6 owns the samples.
- **Design suggestion:** none (not accept).
- **Future row:** none (not defer).

### H10: agent-environment checklist
- **Verdict:** m7
- **Covers:** none
- **Live on the tip:** no skill. The guarantee the checklist asks the author to keep ("never put the grading reference in the build context or treatment") is held by Candler's own I8 guard (`i8_canary_never_reaches_the_agent`, named in R04 round 02).
- **The ask questioned:** a checklist that tells an author to avoid a leak asks the caller to do what Candler's guard and preflight should refuse (Eliott: a box that differs from its files is a Candler defect). What a checklist can still hold is the facts a caller cannot derive: the pairing rules of build layers and the lock/requirements caveats (R07 asks for them as data, H3). Those are code facts, so they come from `knowledge_as_data` (C3), not from prose.
- **Responsibility:** the facts are Candler's (generated, C3); the checklist wording is M7's.
- **Blast radius:** none now.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | 0 | 0 | + fewer silent mistakes |
  | C | − a hand checklist copies rules the code owns | + rules served as data, once | 0 |
  | E | 0 | 0 | + fewer rebuilt images |
- **Compromise:** the checklist keeps only what C3 serves; a rule that is only in prose is a defect to fix in code.
- **Idioms:** follows "values are derived, not asked for"; breaks "defensive checks against the repository's own code" if written as caller advice.
- **Reason or owner:** M7 (skill) with `knowledge_as_data` C3 as the source.
- **Design suggestion:** none (not accept).
- **Future row:** none (not defer).

### H11: gap-ownership notes (Candler or the plug-in)
- **Verdict:** m7
- **Covers:** none
- **Live on the tip:** partly. Every refusal row carries `side` (plugin, suite, experiment, config, environment, candler; `docs/contract/codes.toml:11-12`), a machine-readable "who must fix it". The layer table says who owns what (`architecture.md:102-115`).
- **The ask questioned:** for refusals the map exists and is generated; a second hand-written symptom table would go stale (Eliott on remedy hints: they rot and become workarounds). For non-refusal symptoms (a wrong result), the owner is a judgement a note cannot hold. What the ask lacks is a way for an offline agent to reach `side`: it is in the refusal record (`--error-format json`, `refusal/1`) and in the registry.
- **Responsibility:** the map is Candler's and already generated; the skill phrasing is M7's.
- **Blast radius:** none.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | 0 | 0 | + the caller knows whose fix it is |
  | C | − a symptom table copies `side` and the layer table | + `side` is the one source | 0 |
  | E | 0 | 0 | + fewer misrouted gaps |
- **Compromise:** `side` wins; M7 derives its notes from `side` and the layer table, with no copy.
- **Idioms:** follows "a refusal is a code, a side and a path"; no remedy key.
- **Reason or owner:** M7; source `codes.toml` `side` and R01 §Layer responsibilities.
- **Design suggestion:** none (not accept).
- **Future row:** none (not defer).

### H12: form (CLI with a stable `--json`; MCP only for state)
- **Verdict:** m7
- **Covers:** none
- **Live on the tip:** the CLI half is partly there (`resolve --json`, `watch --json`, `--error-format json`; `ps` and `ls` print TSV only, per R09 §2 and R08). The skill half ("inject live `--help`, never a curated flag list") fits the offline rule: help travels in the binary. `--help` carries no examples or exit codes today (`crates/candler/src/main.rs:1-33`).
- **The ask questioned:** two halves with one seam. **CLI half:** the stable `--json` is D1's question (`cli_contract`), judged there, not here. **MCP half:** "only where it holds state an agent would otherwise rediscover" is M7's shape; Candler's I16 (no daemon) means any state lives in the store, so M7 plans a server that reads the store, not one that owns state. **Seam:** the CLI is the base and defines the JSON types; M7's server reuses those types and adds nothing the CLI cannot say (this is D1's "share the types without deciding M7's shape"). I decide nothing for M7.
- **Responsibility:** CLI half Candler's (D1); MCP half M7.
- **Blast radius:** none from this record.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | 0 | 0 | + agents drive Bash well |
  | C | 0 | + one set of output types | + one contract, two surfaces |
  | E | 0 | 0 | + no wrapper over `run` |
- **Compromise:** none; D1 and M7 meet at the shared output types.
- **Idioms:** follows "one operation API, thin surfaces" (R01 Layer responsibilities) and I16.
- **Reason or owner:** seam to D1 (CLI) and M7 (MCP), named above.
- **Design suggestion:** none (not accept).
- **Future row:** none (not defer).

### X6: sample files: experiment, suite, manifest, import rules
- **Verdict:** accept
- **Covers:** none
- **Live on the tip:** no sample in `docs/` (R09 gap 1). Samples exist as fixtures that tests parse, so D3's first question is answered yes. See the table.
- **The ask questioned:** the real problem is that a TOML cannot be assembled from field tables. A new set of sample files with a test per sample would be a second copy. Of the four kinds, "manifest" is not an authored file (the run store writes it), so a sample would be a second source of the schema: leave it out.
- **Responsibility:** Candler's, web-wide audience (samples beside the page of their surface). Interactive surfaces cannot link to them: if X3 wants an example in `--help` it needs its own path (see `## Seams`).
- **Blast radius:** `docs/contract/` (one example section on four pages), the page generator in `crates/candler/tests/it/support/contract_pages/`, and the shape rule of `CONTRIBUTING.md:149-160`. No `schemas@` release, no `codes.toml` row. Digests: none.
- **D3 answer, first: the fixtures that tests already parse.**

  | Page | Fixture | Read by | Domain-free |
  |:--|:--|:--|:--|
  | experiment-file | `crates/candler/tests/fixtures/observe/ls/experiments/a.toml` (20 lines: `fixed`, one factor, budget, harness, treatment) | the `ls` guard `guard_cli_observe.rs:486-529` | yes |
  | experiment-file (a larger shape) | `crates/candler/tests/fixtures/golden_design/experiment.toml` (25 lines, a factorial) | `ExperimentFile::from_toml_str` in `guard_golden_design.rs:127-190` | no: names `htn-creator` |
  | suite-file | `crates/candler/tests/fixtures/plugins/needs-inputs/suite.toml` | the evaluate guards (`guard_cli_evaluate.rs:42`) | yes |
  | plugin-manifest | `crates/candler/tests/fixtures/plugins/needs-inputs/plugin.toml` | the same | yes |
  | import rules (the `ingest --rules` file; no page exists, the sample goes where the rules are documented, see below) | `crates/candler/tests/fixtures/cli/mini-corpus-rules.toml` | `guard_cli.rs:271-371`, `:373-412`, `guard_ingest.rs:486-574` | no: names `htn-results@1` |

  So no new test is needed to prove a sample is valid. What is not covered: that the page shows the file. Today a page quoting a fixture would be a hand copy.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | + a sample cannot rot: a test parses it | 0 | + a valid TOML to start from |
  | C | + one file serves guard and page | − the generator reads test fixtures | 0 |
  | E | + no new test | 0 | + fewer failed first attempts |
- **Compromise:** Scalable (no copy) and Use win over a tidy fixture directory. Cost: the fixture's owner knows a docs page reads it; a fixture edit shows as a page change under the existing d1 guard, regenerated like a schema. Fixtures stay where their guards read them, so no fixture exists for a page (AGENTS.md "a special case kept for a fixture" is not at play).
- **Idioms:** follows generated pages under a drift test; one source per fact. Breaks the five-section page rule (`CONTRIBUTING.md:149-160` allows "no other" section), so the rule is amended to add an `Example` section "generated from a fixture a test parses".
- **Reason or owner:** Candler.
- **Design suggestion:**
  - Files: extend the page spec (`pages.rs:28`, `PAGES`) with an optional sample path, render it as a fenced block in an `## Example` section, update the section-order guard to allow the sixth section, and amend `CONTRIBUTING.md` §Contract pages. The import-rules sample goes where the rules format is told; no page covers it today, so M6b's X7 style page or a Notes entry of `experiment-file.md`'s neighbour does it, and the build session decides with `codes_review`'s count of `config.import_*` rows. If a domain-free rules file is wanted instead of the HTN one, that is a new fixture whose guard is `guard_cli.rs:373` unchanged: not proposed (no consumer other than HTN).
  - Tests, per D3: **no new test.** The existing d1 guard (`contract_pages.rs:73`) is extended, role **guard**; the question: does a page's example equal the file a test parses? No existing test answers it because no page shows a file; sustainable because the generator reads the file, so a new sample adds a row, not a test; scalable because the sixth section is declarative.
  - Size: about 8 Sonnet calls.

### X7: a page for `candler.toml` and the user config
- **Verdict:** accept
- **Covers:** none
- **Live on the tip:** no page. The code holds two files, not one: the project `candler.toml` with `store.root` and `plugins.dirs` (`crates/candler-core/src/resolve/config.rs:13-17`, `:149-161`), and the user config `$XDG_CONFIG_HOME/candler/config.toml` (else `~/.config/candler/config.toml`) with `slots.count`, `slots.evaluators` and `gateway.upstream_ca` (`config.rs:19-23`, `:242-273`).
- **The ask questioned:** R09 gap 2 says `candler.toml` holds `plugins.dirs`, `store.root`, `slots.*` and `gateway.*`. On the tip, `slots` and `gateway` are in the user config, not the project file: a premise that would put keys in the wrong file. The real problem stays: five keys in two files are documented only by refusal texts.
- **Responsibility:** Candler's: config is a surface an author writes (the contract page's own definition, `CONTRIBUTING.md:149`). Web-wide audience; interactive callers see each value with its source through `resolve` (R09 §2).
- **Blast radius:** a new `docs/contract/config-file.md` and its index row; the key tables `PROJECT_TABLES` and `USER_TABLES` become readable by the page generator (today private constants in `candler-core`, so a read accessor or `pub` constants: crate `candler-core`, module `resolve::config`). No wire type, so no `schemas@` release. No `codes.toml` row.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | 0 | 0 | + fewer failed starts |
  | C | + keys generated from the tables the code reads | − five keys exposed to a generator | 0 |
  | E | 0 | 0 | + the config is found in one read |
- **Compromise:** generating the Fields table from the constants costs a visibility change; a hand table would be a second source of five keys. Generated wins.
- **Idioms:** follows generated pages under a drift test; "every new option must say why it cannot be derived or live in the experiment file" (R01 Risks) is about options, not this page.
- **Reason or owner:** Candler.
- **Design suggestion:** one generated page: Fields from the two key tables, with the file's location and precedence in hand Notes, and an Example (project file: `crates/candler/tests/fixtures/observe/ls/candler.toml`, parsed by the `ls` guard; the user config has no fixture, so the Example is hand text only if a test already holds one: the build session checks, adds none). Tests per D3: no new test; the d1 guard covers the page (role guard; question: do the page's keys equal the keys the reader accepts; no other test compares the two; scalable because the page reads the constants). About 6 Sonnet calls.

### X11: the Python plug-in API
- **Verdict:** accept
- **Covers:** none
- **Live on the tip:** partly. The API is documented in the helper's docstrings: the module docstring, `Kind` (`python/candler-evaluator/src/candler_evaluator/plugin.py:1-24`), `RunContext` with its ten fields (`plugin.py:90-121`), `Plugin` with `KINDS` and the optional `NAME`, `VERSION`, `DIST`, `INSTRUMENTS`, `KIND_SPECS` (`params_schema`, `needs`, `default_deadline_ms`) and `DETERMINISTIC` (`plugin.py:124-160`). Not in any page: `docs/contract/evaluator-helper.md` carries refusals and an outcome fields copy and names `KINDS` only inside a refusal row (`evaluator-helper.md:93`, `:103`); the helper README is maintainer text (`python/candler-evaluator/README.md:3-5`).
- **The ask questioned:** "visible only in code" is right, but the code is where Python authors read an API, offline, in an installed package. The problem is discoverability, not absence. A new generated Python reference (pdoc, mkdocs) adds a tool for one module; the docstrings are already the single source.
- **Responsibility:** Candler's helper, interactive audience (package docstrings and the helper README, which travels with the distribution), plus one web-wide pointer.
- **Blast radius:** `python/candler-evaluator/README.md` (rewritten from maintainer text to a plug-in author page, keeping the maintainer part below), one sentence in the hand Surface of `docs/contract/evaluator-helper.md`. No `schemas@` release, no `codes.toml` row; the helper version moves by a docs-only change.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | 0 | 0 | + a plug-in is writable offline from the package |
  | C | + docstrings stay the source | 0 | − the README repeats a signature | 
  | E | 0 | 0 | + one read, not a source walk |
- **Compromise:** the README states the shape in one example, `examples/synthetic_plugin/synthetic_plugin.py`, which an engine fixture imports (`engine_second.py:3`), and says to run `help(candler_evaluator.plugin)` for fields. Use wins over a pure no-repeat rule; the example cannot rot silently because a test imports it.
- **Idioms:** follows one source per fact; interactive surfaces self-contained; avoids a second doc tool.
- **Reason or owner:** Candler.
- **Design suggestion:** rewrite the helper README's plug-in section (also removes `README.md:5`'s "M2a fills it in"); add a hand Surface sentence to `evaluator-helper.md` pointing at the README; no new page, no generator. Tests per D3: none new. About 4 Sonnet calls.

### X12: no single glossary
- **Verdict:** defer
- **Covers:** none
- **Live on the tip:** yes. Terms (cell, arm, run key, stratum, block, role, treatment, harness, census, gateway, instrument, seal) are defined in struct docs, not in one place. One collision is real: the code's `Stratum` struct is "one block of a comparison" (`crates/candler-core/src/analysis/blocking.rs:29-38`), while `docs/statistics.md:80` and `:166-167` use "stratum" for the comparability class that cells must share. Two senses of one word.
- **The ask questioned:** a glossary is not generated from anything: a hand page copies meanings the code docs hold, a second source whose only guard would be a test per term, the "test per incident" AGENTS.md calls a failure. The harm that evidence shows is the inconsistent word, which a glossary would record, not fix. The vocabulary also moves with M6b's analysis verdicts (`analysis_outputs`) and M7's text.
- **Responsibility:** Candler's, web-wide; the collision belongs to `analysis_outputs` (names in analysis) before any glossary.
- **Blast radius:** a docs page; if the collision is resolved by renaming `Stratum` it touches `candler-core::analysis::blocking` and its callers.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | − hand page with no guard | 0 | + one definition per term |
  | C | − a second source of each meaning | 0 | 0 |
  | E | 0 | 0 | + fewer lookups |
- **Compromise:** Scalable wins now: no hand glossary until the vocabulary is settled and a consumer misreads a term.
- **Idioms:** "Future features are built only when their evidence trigger is met"; one source per fact.
- **Reason or owner:** Candler's, waits for an evidence trigger.
- **Future row:** | Glossary page | One page of the terms the contract pages use, each entry quoting its owner's doc comment | A consumer reads cell, arm, stratum or block in the wrong sense, or M7's text needs a term the pages never define; and `analysis_outputs` has settled `Stratum` versus "comparable stratum" | A recorded misreading by a consumer agent, or M7 planning names the term |


## Seams

- **X6 and `cli_contract` X3.** The page sample (web-wide) and an example in `--help` (interactive) share a file but not a path: `--help` may not link. If X3 accepts examples, it embeds the same fixture bytes; `include_str!` from `crates/candler/src` into `tests/fixtures` is a cross-dependency the build session decides. This leaf does not.
- **X6, X10 and `knowledge_as_data` C3.** The legal setting paths (`model.id`, `runner.mode` ...) are missing from `docs/contract/experiment-file.md:33-35` (`fixed` is a `map`). C3 serves them as data; the experiment page's Fields table should be generated from it when C3 lands.
- **Generator location.** The contract-page generator is test support (`tests/it/support/contract_pages/`), so interactive surfaces cannot reuse it. If M7 needs the same tables offline, the producer must move into a library crate; that is `knowledge_as_data`'s C3 and M7's call, and the responsibility record requires only the same source, not the same code.
- **G16 and `analysis_outputs` X8.** `docs/statistics.md:165` ("Paired tests are not offered yet") is X8's fact; this leaf does not judge it.
- **H12 and `cli_contract` D1.** The CLI half is D1; the MCP half M7.
- **X12 and `analysis_outputs`.** The `Stratum` versus "comparable stratum" collision is theirs to settle.
- **X7 and `codes_review`.** The `config.*` refusals name the keys; the page's Fields should equal them, a check for the build session, not a new test.
- **X9.** The dead rows in `docs/contract/index.md` ("Not raised today", for example `config.no_store`, `index.md:70`) are `codes_review`'s.

## Spec defects found

- README Gotchas (X7) and R09 gap 2 put `slots.*` and `gateway.*` in `candler.toml`; they are in the user config (`config.rs:19-23`).
- `CONTRIBUTING.md:149-160` fixes the page shape at five sections and "no other"; X6's examples need a rule change, now named in X6's design suggestion.
- R09 says the helper README is "contributor only"; true, and it is the file X11 rewrites, which makes the leaf's "no file outside" rule matter at build time, not now.
- G16's prevention ("MCP serves `docs/contract/*.md`; skills link") contradicts Eliott's round-02 ruling.
