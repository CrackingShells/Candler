# Refusal codes review (v0)

Leaf `codes_review` of M6b. Tip `a0df4d48`. Facts first (this section), advice after (`## Registry verdict`). `F=docs/contract/codes.toml` in every command; `python3` has no `tomllib`, so every count is `awk`/`grep` over the file. Code citations come from colgrep reads of the coordinator worktree, which holds the same code.

## Measurements

### M1. The 377 rows by table, and the 85-versus-83 question
The file is append-only, so a row added later sits under whatever header came last: seven rows sit after the `[helper]` header (`F:1898-1930`). Counting by the header above a row and counting by the table in the row's own key give different numbers.

| Count | `internal` | `helper` | `resolve` | `schemas` |
|:--|--:|--:|--:|--:|
| by header above the row | 83 | 34 | 12 | 83 |
| by table in the row's key | **85** | **27** | **17** | 83 |

- by header: `awk '/^\[[a-z_]+\]$/{s=$0} /^\[[a-z_]+\./{n[s]++} END{for(k in n)print k,n[k]}' $F`
- by key: `awk '/^\[[a-z_]+\."/{t=$0;sub(/^\[/,"",t);sub(/\..*/,"",t);n[t]++} END{for(k in n)print k,n[k]}' $F`
- the strays: `grep -n '^\[internal\.' $F | awk -F: '$1>1761'` prints `1918` and `1922` (two internal rows); `grep -n '^\[resolve\.' $F | awk -F: '$1>1761'` gives five resolve rows (`1898`..`1926`).

**Settled: `internal` is 85.** R04's 85 counts by key and is right; the planning recount of 83 counted by header and is wrong for rows 1918 and 1922 (`candler_core::tree::WalkError`, `...::plugin_environment::FragmentError`). Total check: 85 + 27 + 265 = 377 (`grep -c '^\[[a-z_]*\."' $F` after removing the 18 bare section lines gives 377).

All tables by key: schemas 83, internal 85, evaluation 40, isolation 32, helper 27, jobs 20, import 18, resolve 17, cli 17, runner 10, harness 9, analysis 7, store 6, supervise 3, experiment 2, manifest 1. Bare empty sections: `[frames]`, `[trace]` (`F:786`, `F:1325`).

### M2. Rows that are not caller codes
| Kind | Rows | Command |
|:--|--:|:--|
| `[internal]` allowlist (print `candler.internal`, no `side`, key is `crate::path::Type::Variant`) | 85 | `grep -c '^\[internal\.' $F` |
| `internal` rows carrying a `side` | 0 | `awk '/^\[internal\./{a=1;next} /^\[/{a=0} a&&/^side/{c++} END{print c+0}' $F` |
| `[helper]` rows (Python side: `side` plugin 22, candler 4, suite 1; key `helper.*`) | 27 | `awk '/^\[helper\./{h=1;next} /^\[/{h=0} h&&/^side/{c++} END{print c+0}' $F` |
| retired (`F:675`, replaced by `isolation.harness_version_unreadable`) | 1 | `grep -n '^retired' $F` |
| lifted (`F:1930`, `resolve."isolation.egress_unsupported"`) | 1 | `grep -n '^lifted' $F` |
| caller codes, live | 263 | 377 - 85 - 27 - 2 |

So 112 of the 377 rows (30 %) are not caller codes; 263 are. R04's "375 live" is right but hides that only 263 are codes a caller can parse as `refusal/1`.

### M3. Caller codes by side (265 rows with a side outside `internal` and `helper`)
Command: `awk '/^\[[a-z_]+\."/{t=$0;sub(/^\[/,"",t);sub(/\..*/,"",t)} /^side *=/{c[$3]++} END{for(k in c)print k,c[k]}' $F` (includes the 27 helper rows' sides; helper-free counts below subtract them).

| side | all rows with a side | of which helper |
|:--|--:|--:|
| environment | 91 | 0 |
| plugin | 58 | 22 |
| config | 56 | 0 |
| experiment | 39 | 0 |
| candler | 28 | 4 |
| suite | 20 | 1 |

Caller rows with side `candler` (a defect in Candler, never an author's error): 24, all in `schemas` (`manifest.*`, `catalog.*`, `outcome.*` builders). By table: environment is 100 % of `isolation` (23), `jobs` (17), `runner` (7), `store` (6), `harness` (6), `supervise` (3), `import` (15 of 18); `evaluation` mixes four sides (plugin 17, suite 12, environment 7, config 4).

### M4. The table is not the code prefix
Caller codes by the first segment of the code (265 rows): isolation 37, experiment 26, config 23, jobs 20, format 20, evaluation 18, plugin 16, outcome 16, import 16, suite 15, runner 8, manifest 8, analysis 7, store 6, harness 6, catalog 6, canonical 5, resolve 4, supervise 3, protocol 3, values 1, cli 1 = 22 prefixes over 14 caller tables (`awk '{print $1}' caller | sed 's/\..*//' | sort | uniq -c`, where `caller` is the 265 keys). `experiment.*` codes live in five tables (schemas, experiment, runner, resolve, harness); `config.*` in cli, resolve, import. The registry's header says "one section per surface" (`F:7`); in practice a section is a Rust module's catalog, and a surface (the code prefix) spans sections. A caller reads the prefix; the table is Candler's own bookkeeping.

### M5. Emit sites
Method: for each of the 265 caller rows, `git grep -hE "(Enum|Self)::Variant([^A-Za-z0-9_]|$)" -- crates ':!*refusal.rs' ':!*refusal_impls.rs' ':!*/tests/*' ':!*tests.rs'`, comment lines dropped, lines counted. The catalog reads (`candler_core::refusal::catalog`, `crates/candler-core/src/refusal.rs:13-42`, which chains the per-module `catalog()` and `document_value_rows`, `refusal.rs:252`) and the code-to-variant tables (`refusal_table!` blocks, e.g. `crates/candler-schemas/src/refusal_impls.rs:61-198`) are the only places that name every row, and are excluded.

| Emit sites outside tables and tests | Codes |
|:--|--:|
| 0 | 8 |
| 1 | 129 |
| 2 or more | 128 |

The eight at zero: `format.schema_tag_mismatch`, `protocol.version_mismatch` (rows with no `Enum::Variant` shape), `experiment.value_invalid`, `store.value_invalid`, `import.value_invalid`, `resolve.value_invalid` (wrappers built by `#[from]`/delegate over `format.*`), `cli.usage_invalid`, `evaluation.evaluator_not_started` (`ClientError::Spawn`). Zero hits is a static signal, not proof of a dead code: a `#[from]` conversion or a macro builds a variant without naming it. **No caller code is proven unemitted by this count; none of the eight is proven emitted either.** One site is not safe: it can be a single constructor behind a guard.

Emit versus page: `config.no_store` has one site, `CliError::refused_by(CliRefusal::NoStore)` in `resolved_root` (`crates/candler/src/commands/common.rs:103`), and its row text says "Not raised today: a verb that is given no store root takes the default store" (`crates/candler/src/commands/refusal.rs:235-240`, `docs/contract/index.md:70`), while the variant's own message says "no store: pass --store, set CANDLER_STORE or configure store.root" (`refusal.rs:24` region). The row and the code disagree; whether the site can fire depends on whether the settings always carry a default store root (R06 G9 says a missing root takes the default).

### M6. Codes behind `*.document_invalid` (G8)
`ExperimentFile::from_toml_str` flattens every deserialization error of the document into `ExecutionError::Document { detail, path }` unless the error is an unknown field (`crates/candler-schemas/src/execution.rs:1285-1301`, via `read_value`, `crates/candler-schemas/src/refusal.rs:747`, which keeps the pointer from `serde_path_to_error` but turns the inner error into a string). The suite file's own wrapper is the same shape (`suite.document_invalid`, `EvaluationError::Toml`). So the inner reason of a failed document is a sentence in `detail`, never a code, on the file-read path.

Candidate rows whose code the file-read path cannot print (static evidence from the page Notes and from the flattening; programmatic builders and `ExperimentFile::new` callers are not traced here):

| Group | Rows | Count |
|:--|:--|--:|
| experiment inner | `count_zero`, `cost_not_positive`, `factor_without_levels`, `table_empty`, `path_overlap`, `input_invalid`, `entry_repeated`, `reserved_root_misused`, `reserved_value_malformed`, `absolute_path` | 10 |
| suite inner | `suite.list_empty`, `id_repeated`, `dependency_unknown`, `dependency_cycle` | 4 |
| `format.*` (the three author documents never print them, per `docs/contract/experiment-file.md:108`) | 19 of 20 (`schema_tag_mismatch` excepted) | 19 |
| `canonical.*`, `values.null` | | 6 |

Command for the experiment/suite rows: `grep -E 'count_zero|cost_not_positive|factor_without_levels|table_empty|path_overlap|input_invalid|entry_repeated|reserved_root_misused|reserved_value_malformed|absolute_path' docs/contract/codes.toml`. Total shadowed candidates: 39 of 263 live caller codes (15 %).

### M7. Codes that share one cause or one fix
Counted on the 265 keys with `awk '{print $1}' caller | grep -c '<pattern>'`:

| Family | Rows | Pattern |
|:--|--:|:--|
| `*.value_invalid` wrapper of `format.*` | 6 | experiment, resolve, store, import, outcome, cell value |
| `*.io_failed` | 6 | store, supervise, jobs (2), evaluation (2) |
| `*.field_unset` / `*_field_unset` (builder left a required field unset; side `candler`) | 5 | manifest, protocol, outcome, catalog, config |
| `*.unknown_field` | 3 (+ `config.unknown_key`) | experiment, suite, plugin |
| `isolation.gateway_*` | 8 | unavailable, start_failed, misconfigured, no_answer, unauthorized, capped, upstream_error, not_serving |
| `*docker_*` | 7 | one cause (Docker not answering) in isolation and jobs |
| `isolation.mount_*` | 5 | |
| `isolation.env_name_*` | 3 | |
| `*dependency_*` | 4 | `suite.*` and `evaluation.*` twins (`suite.dependency_cycle` vs `evaluation.dependency_cycle`; `dependency_unknown` likewise) |
| `*toml_invalid`/`json_invalid` | 4 | |
| `*unreadable` | 17 | different files, different fixes |

Granularity range: `isolation` carries 37 codes under 9 prefixes of mounting, env, gateway, image, sweep and docker; `manifest` 8 and `catalog` 6 are all `candler`-side builders; `experiment` (the table) holds 2 and `manifest` 1.

### M8. Contract pages
- Caller codes on no page Refusals table: 24 (`format.*` 10, `manifest.*` 8, `catalog.*` 3, `outcome.*` 2, `protocol.field_unset` 1), `comm` of the page rows against the registry keys (page rows: `grep -hoE '^\| \`[a-z_]+\.[a-z_0-9]+\`' docs/contract/*.md`, which also matches field rows, so the page-minus-registry side is noise and is not used).
- Version stamps `[checked: v@x]` (`grep -n 'checked: v@' docs/contract/*.md`): 21 stamps. `v@0.6.0` on 10 (`suite-file.md:66-69`, `plugin-manifest.md:62-66`, `evaluator-protocol.md:511`), `v@0.7.0` on 11 (`experiment-file.md:106-110`, `driver-traits.md:72-75`, `harness-capabilities.md:41-43`). The workspace is `0.13.0` (`Cargo.toml:6`), `candler-schemas` is `0.11.3` (`crates/candler-schemas/Cargo.toml:4`).
- `docs/contract/index.md:70` says `config.no_store` is "Not raised today" (see M5).

## Registry verdict

Reading rule: a code is a contract, so each line names what a caller sees change, then the blast radius (the row, `refusal_table!` in the owning crate, the contract page, the guards that read `codes.toml` and `candler_core::refusal::catalog()`). The nine dimensions are the triad: sustainable, scalable, efficient, each for development, codebase and consumption.

### V1. Is 377 a lot? Mostly it is not the callers' number
112 of 377 rows (30 %) are not caller codes: 85 `[internal]` allowlist entries, 27 `[helper]` rows (M2). A caller parses 263 live codes. The headline "377" mixes three audiences in one file: the caller (`refusal/1`), the allowlist of errors that print `candler.internal`, and the Python helper's own codes. **Keep all three in the file; re-section by audience, not by count.** The allowlist rows exist so that a test can prove no coded error escapes unlisted (architecture §Refusals); the helper rows are read by the helper's tests and the generator. Moving either to another file would split the oracle ("the oracle of the tests that read the codes") and change the two readers; the cost is high and a caller sees nothing. The change that costs nothing is to state the split once, in `docs/contract/index.md` and the header of `codes.toml`: "N caller codes, M allowlist entries, K helper codes", with the count generated, not hand-kept (M1: three counts in play, 83, 85 and 375, were each right for a different counting rule).
- Caller sees: nothing. Blast radius: header comment of `codes.toml`, one line in `index.md`; no row moves, no `schemas@` release. Triad: sustainable +, scalable 0, efficient 0.

### V2. Section is not surface (M1, M4)
The file appends, so a row lands under the last header (seven rows after `[helper]`), and 22 code prefixes sit in 14 tables (`experiment.*` in five). The header promise "one section per surface, in the order below" is false in two ways. **Re-section: yes, as a rule, no row rewritten.** (a) State in the header that a section is a module's catalog and the prefix is the surface. (b) Append later rows under their own header (the seven strays are the example); a guard that the table of a row equals the table of its key would fail today on 7 rows. A guard here fails the AGENTS.md fail list ("a test per incident")? It is not a per-incident test: it is a probe-grade structural check on the file's own promise. **I do not propose it**: the seven strays are readable, no caller sees them, and the registry's own append-only rule makes moving them an edit of the file's history. Verdict: fix the header sentence only; leave the strays.
- Caller sees: nothing. Blast radius: header comment. Triad: sustainable +, others 0.

### V3. Merge: no
Merge here means retire N rows into one (`retired` key). Candidates by shared cause or fix (M7): the `*.value_invalid` wrappers (6), `*.io_failed` (6), `*.field_unset` (5), `isolation.gateway_*` (8), docker (7), `suite.`/`evaluation.dependency_*` twins (4). **Keep every one.** A code is promised once a release carries it and the project has no external users, so retiring costs nothing in compatibility; the reason to keep is the other direction: the codes differ in `side` or `path` (`*.io_failed` is environment in each table but names a different file; `isolation.gateway_*` splits into causes the caller can act on differently, `capped` is a budget, `unauthorized` a secret, `no_answer` a network). A merge would remove a distinction a caller parses and add nothing, and each merge touches a variant mapping, a page row and every guard that checks the row. The one candidate that is a real duplicate is the dependency pair: `suite.dependency_cycle`/`evaluation.dependency_cycle` and `dependency_unknown` x2 are the same fault found at two stages (suite read, then plan), side differs (suite versus the suite's plan). Whether a caller needs to tell the stage apart is a consumer question for the agent-consumability work, not mine; **defer, with the evidence trigger "an MCP client matches on one and not the other"**.
- Caller sees: nothing change. Blast radius if merged: two rows retired, `refusal_table!` of `crates/candler-core/src/evaluation/refusal.rs` and `suite`'s, the suite and evaluator pages. Triad: 0 across.

### V4. Retire: no new retirement; the two existing exits stand
`isolation.claude_version_unreadable` is retired into `harness_version_unreadable` (`F:675`) and `isolation.egress_unsupported` is lifted (`F:1926-1930`): both rows stay and no variant emits them, which is the promise working as written. **Verdict: done.** No row in M5 is proven unemitted, so no retire is justified by evidence; the eight zero-site rows need a runtime or `#[from]` trace the M6b study does not run. If the build session wants this, the check is one `candler_core::refusal::catalog()` consumer that lists rows with zero constructions: a probe, run once, then deleted.

### V5. The G8 shadow: the largest granularity defect in the file
39 caller rows (15 %) are rows the file-read path flattens into `document_invalid` (M6). These are the codes a skill most wants to name (`factor_without_levels`, `count_zero`, `path_overlap`) and the ones it cannot match. The registry promises 39 codes that the CLI path does not print; a caller who reads the page learns a code, then sees `document_invalid`. This is a fact about the code and the pages, not a choice. **Verdict on the registry itself: keep every row (append-only), and decide G8 in `## Verdicts` below**, which fixes where the code prints, not how many rows exist.

### V6. No remedy key, no deletion
Nothing here adds `remedy` (Eliott's ruling) and nothing deletes a row; the only forms used are `retired` and `lifted`, as already in `F:675` and `F:1930`. Gate: `! grep -Eq '^remedy *=' docs/contract/codes.toml`.

### V7. Summary
| Finding | Verdict |
|:--|:--|
| 377 headline | keep; state the 263 / 85 / 27 split in the header and `index.md` |
| section is not surface; 7 strays | header sentence only; strays left |
| value_invalid, io_failed, field_unset, gateway, docker families | keep, no merge |
| dependency twins | defer, trigger named |
| existing retired / lifted | done |
| 8 zero-site rows | not proven dead; no action |
| 39 rows shadowed by `document_invalid` | G8 |

## Verdicts

### G8: refusal codes behind `*.document_invalid`
- **Verdict:** accept
- **Covers:** none
- **Live on the tip:** yes. `ExperimentFile::from_toml_str` turns every non-unknown-field deserialization error into `ExecutionError::Document { detail, path }` (`crates/candler-schemas/src/execution.rs:1285-1301`), and `read_value` keeps the pointer but only the message of the inner error (`crates/candler-schemas/src/refusal.rs:747`). The page documents it (`docs/contract/experiment-file.md:108`, `suite-file.md:66`). One sub-claim of R06 is not verified here: "an unknown key under `[budget]` reports path `/foo`, not `/budget/foo`" (not read; it belongs to the build session's first probe).
- **The ask questioned:** R06 asks for the inner code in `causes[]`. The premise is right for 39 rows (M6) and wrong in its ask in one respect: `causes[]` is "counted with their causes" (architecture, the `refusal/1` record `{schema, code, side, path?, file?, message, causes[]}`), the field for a chain of causes. The inner reason here is not a second cause; it is the cause. Three options: (1) `causes[]` carries the inner code and `code` stays `document_invalid`: a caller matching on `code` still cannot act, and an agent must read an array to find the code; (2) `code` becomes the inner code and `document_invalid` remains for the cases with no inner code (type errors serde cannot name): the code a caller matches is the real one; (3) leave it, and document "match on side plus path". (2) is what the registry already promises. The real problem is that validation lives in `new()` and runs inside serde's `try_from`, where the typed error is lost to a string: `serde::de::Error::custom`.
- **Responsibility:** Candler's. R01 §Layer responsibilities: `candler-schemas` owns the type and its refusal mapping (`refusal_table!`, `crates/candler-schemas/src/refusal_impls.rs:61-198`); no surface can fix it.
- **Blast radius:** `candler-schemas` only (`execution.rs`, `evaluation.rs`, `refusal.rs`, `refusal_impls.rs`); a `schemas@` release (the mapping from variant to code is a behaviour change on the wire: for the same invalid file `code` changes from `experiment.document_invalid` to `experiment.factor_without_levels`, while `path`, `side` stay); `codes.toml` rows: none added, none retired, `document_invalid` stays and still describes the type-error case; pages: the two Notes lines (`experiment-file.md:108`, `suite-file.md:66`) and their `[checked]` stamps change; guards: `contract_pages` (the Refusals tables are generated), `schema_drift` untouched, every test that asserts `document_invalid` on a value-invalid file changes its expectation; digests: none (no record written changes).
- **Triad:**
  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | + one code per fault, no sentence to parse | 0 | + a skill names the code and stays true |
  | scalable | 0 | - the typed error must survive serde (a typed side channel or a post-parse validation step) | + every new inner code is reachable by construction |
  | efficient | 0 | 0 | + one read, no second pass over `detail` |
- **Compromise:** the codebase cell is the cost: carrying a typed error through `try_from` needs either a side channel on `Located` or running `new()` after a plain deserialization. The consumption gain wins; the cheaper of the two ways (post-parse validation, where `new()` is already the single validator) adds no second layer of validation, it moves the existing one after the parse.
- **Idioms:** follows "a refusal is a code, a side and a path", "values are derived, never asked for" (the inner code is already known), "facts for agents are generated under drift tests". It must not become a parser of `detail` strings (a second source).
- **Reason or owner:** Candler owns it; the registry's promise is the spec.
- **Design suggestion:** `candler-schemas`: validate after parse so `ExecutionError`/`EvaluationError` keep their typed variants, map to `code` through the existing table; keep `Document` for pure shape errors; update the two Notes lines; size about 25 Sonnet calls. Test, role **confirm**: the question is "does an experiment file with no level on a factor print `experiment.factor_without_levels` and a suite with a repeated id print `suite.id_repeated`?"; no existing test answers it because the current tests assert `document_invalid` for these cases (that is the defect); sustainable because it is one table-driven test over the 14 inner codes of M6, reading expectations from `codes.toml`; scalable because a new inner code adds one row to the table, not one test.

### X9: contract-page rows that no longer apply, and stale stamps
- **Verdict:** accept
- **Covers:** none
- **Live on the tip:** yes. (a) `docs/contract/index.md:70` says `config.no_store` is "Not raised today", but the variant is built at `crates/candler/src/commands/common.rs:103` and its message tells the caller to pass `--store` (`crates/candler/src/commands/refusal.rs:24`); the row text is `crates/candler/src/commands/refusal.rs:235-240`. Row and code disagree: one is wrong. (b) 21 `[checked: v@...]` stamps read 0.6.0 (10) or 0.7.0 (11) against workspace 0.13.0 (`Cargo.toml:6`) and schemas 0.11.3 (`crates/candler-schemas/Cargo.toml:4`). (c) the retired and the lifted row are not on a page as such: `isolation.egress_unsupported` is lifted (`F:1926-1930`) and `config.retired_setting` is live (`index.md:74`); I did not find the lifted code on a page, which is right, so no page row needs removal for it.
- **The ask questioned:** the premise ("rows that no longer apply") is mostly false: of the 265 caller rows, no page lists a code the registry lacks, once field rows are ignored, and the one page row that no longer applies is `config.no_store`. The stamps are a fact, but a stamp is a claim of a check: `[checked: v@0.6.0]` says "I checked this at 0.6.0", it does not say the Note is wrong now. Re-stamping by hand would assert a check nobody made, a stamp that lies is worse than an old one. The real problem is that a stamp is hand-kept beside a generated page and cannot drift visibly.
- **Responsibility:** Candler's, development audience: the pages are generated by `contract_pages` (architecture §Contract pages) but Notes and stamps are hand-written.
- **Blast radius:** `docs/contract/*.md` Notes and the one `no_store` row (generated from the `refusal_table!` text at `crates/candler/src/commands/refusal.rs:235`, so the fix is in the row text, then regenerate with `CANDLER_BLESS_CONTRACT=1`); no `codes.toml` row changes; guards: `contract_pages` blesses; no `schemas@` release; digests none.
- **Triad:**
  | | development | codebase | consumption |
  |:--|:--|:--|:--|
  | sustainable | + a stamp tied to a check | 0 | + a page that tells what happens |
  | scalable | 0 | 0 | 0 |
  | efficient | 0 | 0 | 0 |
- **Compromise:** none.
- **Idioms:** follows "facts for agents are generated under drift tests"; breaks it today for Notes and stamps, which are hand-written.
- **Reason or owner:** Candler owns it.
- **Design suggestion:** (1) decide `config.no_store`: the code is live, so rewrite the row to what the variant does when no store root resolves, or, if the default always resolves, retire the variant and the row text says `lifted`-style history (a `retired`/`lifted` row, not a deletion); one decision in the `execution_preflight`/`cli_contract` leaves (G9 is theirs). (2) re-verify each of the 21 stamped Notes against the tip and restamp only those the build session checked, with the version actually read; size about 15 Sonnet calls. No new test: the stamps are text, and `contract_pages` already guards the generated parts.

## Seams
- `execution_preflight` (G5, G6): any new refusal for pre-spend or `validate` is a new row, appended under its own header (V2), side from the existing six, no `remedy`.
- `cli_contract` (D1): the `refusal/1` shape stays; G8 option (2) changes `code` for the same `path`, not the fields. `config.no_store` and G9 meet in `resolved_root` (`common.rs:103`).
- `knowledge_as_data`: the helper's `[helper]` rows (27) are read from `codes.toml`; any move of the file or the section affects the helper tests and the generator.
- `docs_and_skills`: a skill that says "match on side plus path, not on code" (R06 G8) becomes false after the G8 build; the wording belongs to that leaf.
- `verdicts` should carry the 263 / 85 / 27 split, not "377".
