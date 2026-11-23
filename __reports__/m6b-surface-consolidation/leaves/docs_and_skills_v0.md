# Docs and skills verdicts (leaf `docs_and_skills`, v0)

Tip judged: `a0df4d48` (core v0.13.0, schemas@0.11.3, candler-evaluator@0.1.0; `Cargo.toml:6`, `crates/candler-schemas/Cargo.toml:4`, `python/candler-evaluator/pyproject.toml:3`). Eliott's rulings (R04 round 02) and D2, D3 bind. Code was read through colgrep; paths below are the same files in this worktree.

## Documents today

Audiences: **S** store window, **W** web-wide (`docs/`), **I** interactive (CLI, MCP, skills: self-contained, offline), **D** development.

| Document | Audience | What it holds | Generated or hand-written | Drift test |
|:--|:--|:--|:--|:--|
| `README.md` (31 lines) | S, but also D | one-line identity and the two halves (`README.md:3-8`); crate inventory and verb list (`README.md:16-25`); links to the design doc and the roadmap (`README.md:13-14`), `CONTRIBUTING.md`; no link to `docs/`, no install or quick start | hand | none |
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
| Quick start | none. Nothing is published (`README.md:26`), so the honest quick start is a source build and one command that spends nothing. The sample to run must exist: see X6 | lacks |
| Where to go for more | no link to `docs/contract/index.md` or `docs/statistics.md`; the links it has go to the design doc and the roadmap (`README.md:13-14`) | lacks |
| Crate inventory (`README.md:16-25`) | development-level: the workspace layout is `architecture.md` §Workspace. It is also a hand copy of what `Cargo.toml` members and the clap verbs own, with no guard | belongs to D, and is a second source |
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
