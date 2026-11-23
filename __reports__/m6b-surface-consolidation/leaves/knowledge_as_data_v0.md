# Knowledge as data: verdicts on C3, C7, C12, G1, G17

Leaf `knowledge_as_data` of M6b. Judged on the tip `a0df4d48`; code read through colgrep, paths relative to the repo root. Inputs: R05 (C3, C7, C12, G18-G20), R06 (G1, G17), R07 (H3, H4), R01 (Contract pages, Future features).

## Live on the tip

| Id | Live | Evidence |
|:--|:--|:--|
| C3 | partly | Generated and drift-tested already: the six schema pages, `driver-traits.md` and `harness-capabilities.md` (`crates/candler/tests/it/contract_pages.rs:1-25`, regeneration guard at `:73-102`, five-section frame at `:173-234`). The capability page reads each registered driver's `settings_table()` (`crates/candler-harness-opencode/src/driver.rs:189-191`, rows in `prepare.rs:131-195`, one set with the driver's command per `tests/it/guard_opencode_driver.rs:404-519`, row hygiene in `crates/candler-harness/src/conformance.rs:609-638`, page cells in `crates/candler/tests/it/support/contract_pages/capabilities.rs:79-95,240-265`). Not generated: the configuration path vocabulary is a hand-written predicate (`crates/candler-core/src/resolve/layers.rs:60-122`) plus a three-entry constant (`crates/candler/src/commands/refusal.rs:201-206`); the `treatment:<path>` source form lives in `crates/candler-core/src/resolve/steps/image.rs:414-443`. `docs/contract/experiment-file.md` has no row or note for `treatment:` (grep of the page: no match); its only hand note on settings is `:110`. |
| H3, G20 | partly | As C3. R05's "settings_table() exists" is right but it is the harness settings table only, and that table is already a generated page. |
| C7 | partly | The Candler build is recorded: `provenance.eval_infra.{tag,commits_ahead,commit,dirty}` and `provenance.sandbox_contract` (`crates/candler-core/src/runner/evidence/provenance.rs:15-76`), proven by `build.rs` (`crates/candler/src/commands/supervise.rs:47-62`; a field the build cannot prove is left out). A stratum is refused when these are unrecorded or differ (`crates/candler-core/src/manifest/compare.rs:25-31,98-106,414-458`). Not live: any model the endpoint reports (no colgrep hit for a reported, served or backing model in `crates/`); the manifest has the model the cell names (`prepare.rs:239-249`). Not live: an errata registry and `check`. |
| G18 | partly | R05 says "the only record is the coordinator's memory": wrong on the tip, the commit is recorded (above). What is missing is the errata, the list of commits known bad. |
| G19 | yes | The alias is what Candler holds; the backing model is not recorded. Whether the endpoint's stream or the gateway carries it is unchecked (R05 C7 row says so too); this leaf runs no model. |
| H4 | partly | As C7: commit recorded, model and errata not. |
| C12 | no | No scaffold command: the command modules found are `run`, `resolve`, `ls`, `analyze`, `supervise`, `preflight` and their kin (`crates/candler/src/commands/`), none writes an experiment, suite or manifest. |
| G1 | partly | The catalog is derived per run from the built image, never defaulted: `catalog@1` captures it and records its digest under `snapshots` (`crates/candler-core/src/resolve/steps/catalog.rs:54-86`); the census records the harness's tools and servers (`crates/candler-core/src/runner/evidence/census.rs:58-99`); strata gate on provenance (`compare.rs:414-458`). R06 says the registry refuses a `snapshots` or `inputs` need as unsupported: only `snapshots` is (`crates/candler-core/src/evaluation/registry/needs.rs:44-78`); an `inputs` need is refused only when it names the reserved kind. |
| G17 | yes (as a fact) | Pre-1.0, no external users (R06 G17). The version facts R06 wants pinned already exist as a generated, checked table: `docs/contract/index.md:30-36`, equal to the manifests per `contract_pages.rs:1-25`. |

## The asks questioned

**C3, H3, G20: capability reference as data.** The premise "answered from Rust" is half true. The idiom the ask wants already exists and runs: pages derived from the committed schemas, the trait definitions and the driver registry, under a drift test (R01 §Contract pages). What an agent still reads from Rust is the configuration path vocabulary, the source forms and the pairing rules. That is a fact with no single owner: `is_known_path` (`layers.rs:60-122`) and `REQUEST_KEYS` (`refusal.rs:201-206`) each hold part of it, and `preflight.rs:171-251` consumes both. The gap is therefore a missing generated page plus one table the code reads, not a new serving surface. R07 asks to serve it through `--json`; that is a delivery question (the interactive surfaces are self-contained and offline, docs are web-wide) which belongs to D1 and M7, not to this fact's owner. Caveats such as "the backend must be in the lock" are prose and rot; the existing Notes frame (stamped, 120 words, `contract_pages.rs:173-234`) is the guarded home for what cannot be derived.

**C7, H4, G18, G19: provenance and errata.** Two asks hide inside it. (1) Recording a fact Candler knows: the commit is done. The model the endpoint reports is the provider's fact; Candler's part is to record what it is told (I8, ground truth), never to ask the provider separately, and only if the stream or gateway already carries it, which nobody has checked. (2) An errata registry is a new fact source: a hand list of bad commit ranges beside git history and beside the recorded commit. It breaks "one source per fact" unless one file owns it under a drift test, and it answers a question (which stored runs a defect touched) that has one occurrence so far (before `1e6dc91a`, `mcp.*` gave no servers). A manifest change is a `schemas@` release: for the model, `provenance.*` entries are measured path/value pairs (`provenance.rs:29-76`), so a new path may need no schema change; this is not checked here.

**C12: scaffolds.** A scaffold "from factors and levels" is the experiment file written through flags: a second authoring surface that moves guessing from the agent into a template that can rot, against "every new option must say why it cannot be derived or live in the experiment file" (R01 Risks). The one derivable fragment (a manifest kind signature from a plug-in) is X11's, and tested sample files are X6's; both belong to `docs_and_skills`, whose D3 first checks whether fixtures that tests already parse can serve. Name the seam; do not decide it here.

**G1: wrong catalog or stratum reads as valid.** The cause named (default catalog, no `--servers-dir`, legacy grader) is the old HTN grader. On the tip the catalog comes from the image, its digest is recorded, the census records what the harness saw, and a stratum that differs or is unrecorded is refused. What remains is the choice of catalog by the experiment's author, which Candler cannot judge: it has no statement of intent to compare with. "List the catalog source per run" is `resolve --explain` (C4, `execution_preflight`).

**G17: compatibility without `!` markers.** Not an ask for Candler to build; the generated version table already gives a skill what to pin. The finding about four unmarked behaviour changes (M2c) goes to the supervising session, not to a verdict here.

## Verdicts

### C3: capability reference as data
- **Verdict:** accept
- **Covers:** H3, G20
- **Live on the tip:** partly: pages and the harness settings table are generated and drift-tested (`crates/candler/tests/it/contract_pages.rs:73-102`, `crates/candler/tests/it/support/contract_pages/capabilities.rs:240-265`); the path vocabulary is hand-coded (`crates/candler-core/src/resolve/layers.rs:60-122`, `crates/candler/src/commands/refusal.rs:201-206`); `treatment:` is in `crates/candler-core/src/resolve/steps/image.rs:414-443` and on no page.
- **The ask questioned:** The premise "answered from Rust" is true only for the path vocabulary, source forms and pairing rules. The real problem is a fact with two hand-written owners and no page. Serving it as `--json` is a delivery choice, not part of the fact.
- **Responsibility:** Candler (the core owns the path vocabulary; the generator is its drift test). Delivery to the CLI or MCP is D1 and M7's.
- **Blast radius:** `candler-core` `resolve/layers.rs` (table), `crates/candler` `preflight.rs` and `refusal.rs` (read the table), `tests/it/support/contract_pages` (one page), `docs/contract/` (one blessed page, index row). No `schemas@` release; no `codes.toml` row; guard `e2_slots_evaluators_is_a_known_path` (`crates/candler-core/tests/it/guard_resolve.rs:788-797`) stays; digests unaffected.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | + one table to edit, page regenerates | + removes two owners of one fact | + page cannot rot |
  | scalable | + a path is a row | + same idiom as the harness pages | 0 one more page to read |
  | efficient | 0 about 12 calls once | + no new surface or verb | + one lookup instead of reading Rust |
- **Compromise:** none. The page adds a sixth-kind page beside the five-section frame; it follows the harness pages (`HarnessPage`), which already read the binary's registry rather than the schema.
- **Idioms:** follows values derived (I12), facts for agents under drift tests (`contract_pages`), thin surfaces (the CLI reads the core's table). Breaks none. Delivery as JSON is left to D1 and M7.
- **Reason or owner:** Candler owns the fact; one table and one generated page make it single-sourced.
- **Design suggestion:** (1) `crates/candler-core/src/resolve/layers.rs`: a const table of rows (path, class, source forms, free below), with `is_known_path` and `REQUEST_KEYS` read from it. (2) `crates/candler/tests/it/support/contract_pages/`: render the table as `docs/contract/configuration-paths.md`, with stamped hand Notes for caveats that are not refusals (the lock caveat), and an index row. (3) Tests per D3: no new test. Role: the existing guard `d1_regenerating_the_pages_changes_nothing` (`contract_pages.rs:73-102`) covers the new page by listing it in the page table; the question it answers is whether the page equals what the code accepts. No existing test answers that for paths (the d1 guard covers schema and driver pages only), and because `is_known_path` reads the table the page cannot differ from the code without the guard turning red, so no tautological test is added. It stays sustainable because a path is one row, and scalable because the blessing command (`CANDLER_BLESS_CONTRACT=1`) and the idiom are unchanged. If a reviewer keeps `is_known_path` hand-coded, a guard would be needed: every accepted path is a row (role guard, one test). Size: about 12 Sonnet calls.
- **Future row:** none

### C7: provenance and errata
- **Verdict:** defer
- **Covers:** H4, G18, G19
- **Live on the tip:** partly: the Candler build is recorded and gates strata (`crates/candler-core/src/runner/evidence/provenance.rs:29-76`, `crates/candler/src/commands/supervise.rs:47-62`, `crates/candler-core/src/manifest/compare.rs:414-458`); the model is the cell's alias (`crates/candler-harness-opencode/src/prepare.rs:239-249`); no reported model, no errata (no colgrep hit in `crates/`).
- **The ask questioned:** The premise "the only record is the coordinator's memory" is wrong: the commit is recorded and a mismatch refuses a verdict. The two parts left are different. The reported model is the provider's fact, and Candler may only record what it is told; nobody has checked that the stream carries it. The errata registry is a new, hand-kept fact source with one occurrence so far.
- **Responsibility:** Candler for recording what the harness or gateway reports (I8); the provider for the model it serves; a consumer's study for the list of commits it distrusts until a second occurrence shows that Candler should own it.
- **Blast radius:** `candler-core` `runner/evidence/provenance.rs` and `manifest/compare.rs` (a gate decision), the harness driver's reading of the stream; new `provenance.*` measured path (likely no schema change, unchecked); a registry would add a file, a verb (`check`), a drift test and a CLI surface (D1). Digests: `provenance.*` values enter the manifest, so a new path changes new runs' manifests, not old ones.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | 0 a registry needs a curator | − a second source beside git and the recorded commit | + a run says what made it |
  | scalable | 0 grows with defects | − one list for every future defect | + one check over a store |
  | efficient | − medium work for one occurrence | − new verb and file | + replaces briefing habit |
- **Compromise:** The consumption gain loses to the codebase cost (one occurrence, a second source of truth), so the registry waits; the model record waits for a fact nobody has seen.
- **Idioms:** values are derived and record their source (I12): the commit already does; the registry would be asked, not derived. "Future features are built only when their evidence trigger is met". Ground truth (I8): Candler records, never asks the provider.
- **Reason or owner:** Candler owns it, but the unchecked stream and the single occurrence do not meet an evidence trigger.
- **Design suggestion:** none (not accept)
- **Future row:** | Reported model and errata | Record the model the endpoint reports as a `provenance.*` value, only as the stream or gateway gives it; an errata file of commit ranges known to give invalid runs, with a `check` over a run or store | A run's validity depends on a Candler commit and the alias hides which model answered | (a) a captured live stream or gateway log of a real run that carries a model field; (b) a second defect, after `1e6dc91a`, that invalidates stored runs and that the recorded commit does not already let `compare` refuse |

### C12: scaffolds
- **Verdict:** reject
- **Covers:** none
- **Live on the tip:** no: no command writes an experiment, suite or manifest (`crates/candler/src/commands/`).
- **The ask questioned:** A scaffold derives nothing; it moves guessing from the agent into a template that rots. "An experiment from factors and levels" is the experiment file written through flags. The derivable part (a manifest from a plug-in's declared kinds) is a documentation sample (X6, X11), not a generator.
- **Responsibility:** nobody as a generator; samples belong to `docs_and_skills`.
- **Blast radius:** would add a verb and a second authoring surface in `crates/candler` and a template set under tests; no schema release; no code row.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | − templates to keep in step with schemas | − a second spelling of the file | 0 agents still choose the values |
  | scalable | − one template per shape | − grows with each plug-in | 0 same |
  | efficient | − medium work | − new verb | 0 saves typing only |
- **Compromise:** none; every cell is 0 or below.
- **Idioms:** breaks "every new option must say why it cannot be derived or live in the experiment file" (R01 Risks); breaks the AGENTS.md fail list (a second layer). Fixtures that tests already parse serve as samples (X6, `docs_and_skills`).
- **Reason or owner:** no derived value; the same need is met by tested sample files, which `docs_and_skills` judges under D3.
- **Design suggestion:** none (not accept)
- **Future row:** none

### G1: wrong catalog or stratum reads as valid
- **Verdict:** owner:consumer
- **Covers:** none
- **Live on the tip:** partly: the catalog is derived and its digest recorded (`crates/candler-core/src/resolve/steps/catalog.rs:54-86`), the census records what the harness saw (`crates/candler-core/src/runner/evidence/census.rs:58-99`), and unrecorded or unequal strata are refused (`crates/candler-core/src/manifest/compare.rs:414-458`); only `snapshots` needs are unsupported (`crates/candler-core/src/evaluation/registry/needs.rs:44-78`), contrary to R06's "snapshots or inputs".
- **The ask questioned:** The cause named is the legacy grader's default catalog, which Candler removed by deriving the catalog from the image. What is left is that an experiment's author picks the catalog; Candler has no statement of intent to check it against, so a "wrong instrument" is the consumer's design error, caught by reading the recorded digest and census. Listing the catalog source per run is `resolve --explain` (C4).
- **Responsibility:** the experiment's author (consumer); Candler records and gates (done).
- **Blast radius:** none for Candler; a guard that guessed the intended catalog would add a second layer of validation against an author's own file.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | 0 nothing to add | 0 | + the record stays one source |
  | scalable | 0 | 0 | 0 |
  | efficient | + no work | + no code | 0 consumer still checks |
- **Compromise:** none.
- **Idioms:** values derived with their source (I12); a guard against the author's own intent breaks the AGENTS.md fail list (a second layer of validation).
- **Reason or owner:** consumer: choosing the instrument is the experiment author's decision; Candler's part (derive, record, gate) is live. The skill for HTN should say "read the catalog digest and census before trusting a fail" (M7 and `docs_and_skills`).
- **Design suggestion:** none (not accept)
- **Future row:** none

### G17: compatibility without `!` markers
- **Verdict:** done
- **Covers:** none
- **Live on the tip:** yes: pre-1.0, no external users; the core and schema versions are a generated, checked table (`docs/contract/index.md:30-36`, `crates/candler/tests/it/contract_pages.rs:1-25`).
- **The ask questioned:** G17 is a development fact, not an ask. Its one prevention (a skill pins core and schemas versions) has its data: the table is generated from the manifests and checked. Whether breaking changes should carry `!` markers is a compatibility finding and goes to the supervising session; nothing here is left to decide.
- **Responsibility:** the supervising session for the marker policy; `docs_and_skills` and M7 for a skill that reads the table.
- **Blast radius:** none.
- **Triad:**

  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | 0 | + the table cannot drift | + versions are stated |
  | scalable | 0 | 0 | 0 |
  | efficient | + no work | + no code | + one table |
- **Compromise:** none.
- **Idioms:** facts for agents generated under drift tests; no external users, so the compatibility finding goes to the supervising session.
- **Reason or owner:** the data exists and is guarded; the policy question is escalated, not judged here.
- **Design suggestion:** none (not accept)
- **Future row:** none

## Seams

- **C3 with X7 and X11 (`docs_and_skills`).** X7 (a page for `candler.toml` and the user config) and X11 (the Python plug-in API) are the same kind of fact as C3's path table: derived from one owner and rendered by `contract_pages`. If C3 is built, X7's rows are more rows of the same table, and X11's page reads the plug-in manifest schema. Build in one pass, so there is one page frame.
- **C3 with D1 and M7.** The page is web-wide. Serving the same table offline to the CLI or MCP is a delivery question: if `candler-schemas` holds output types (D1), the table's row type is a candidate; M7 decides the MCP shape.
- **C3 with C4 (`execution_preflight`).** `resolve --explain` reports, per resolved value, which layer set it; C3's table says which paths exist. `--explain` should read the same table, so the two never name different paths.
- **C7 with D1 (`cli_contract`).** A `check` verb would be a new output type; if C7 leaves `defer`, its output goes through D1's contract.
- **C7 with C4 and G1.** "List the catalog source per run" is C4's; the recorded commit and the census are what a consumer reads for G1.
- **C12 with X6 (`docs_and_skills`).** C12 is rejected as a generator; X6 first checks whether fixtures tests already parse can be the samples (D3).
- **G17 with the supervising session.** The four unmarked behaviour changes of M2c (R06 G17) are the supervisor's.
