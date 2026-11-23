# Knowledge pages build

Branch `claude/m6b-build-knowledge_pages`, branch point e016b320.

## First reads
- Shape built: seam 3 (a), one table with a `file` column. `CONFIGURATION_PATHS` (crates/candler-core/src/resolve/paths.rs:126) has one row per path or path form with a kind, a class (experiment value, setting, request key, configuration key), a file (none, project, user) and the meaning the page prints. `is_setting` (crates/candler-core/src/resolve/layers.rs:34) reads it through `names_a_setting` (crates/candler-core/src/resolve/paths.rs:221); `is_known_path` reads it through `reaches` (crates/candler-core/src/resolve/paths.rs:241); `REQUEST_KEYS` (crates/candler/src/commands/refusal.rs:201) is `request_keys()` (crates/candler-core/src/resolve/paths.rs:186), a const fn over the table that fails to compile when the count is not 3; `check_keys` (crates/candler-core/src/resolve/config.rs:396) takes its tables and keys from `config_keys` (crates/candler-core/src/resolve/paths.rs:255). `PROJECT_TABLES` and `USER_TABLES` are gone.
- Strings stay the `execution::paths` constants. Five texts had no constant and the schemas crate is not this leaf's: `budget`, `budget.per_run`, `replicates`, `isolation.build` (they were literals in `is_setting`) and `gateway.upstream_ca` (a literal in the user reader). They are private consts at the top of paths.rs.
- Class and kind rows reproduce the old semantics: the row with the most segments that names a path decides `is_setting`; `budget.per_run` is a carve-out so its leaves stay values; `gateway.upstream_ca` and `treatment:` are `NotAPath` rows, so `--set gateway.upstream_ca` is still an unknown path.
- `Exit` cannot be included by `#[path]`: `commands/mod.rs` declares every verb. The generator parses the `EXIT_CODES` array of that file with `syn` (crates/candler/tests/it/support/contract_pages/config_pages.rs:136). `exit_help` in `commands/mod.rs` prints `--help` from the same array (43); `main.rs` only sets `after_help`.
- `config.no_store` is raised by `resolved_root` (commands/common.rs:89-104) when the settings hold no `store.root`; the core default (resolve/layers.rs, `Layered::new`) is `<project root>/.candler/store` with a project and `<state home>/candler/store` without one, none with neither. The row now says so.

## Guards changed
- `d1_regenerating_the_pages_changes_nothing` also settles `configuration-paths.md`, `config-file.md` and the index (which gains the generated Exit codes section). `HARNESS_PAGES` has four entries; d2, d3, d4 therefore cover both new pages unchanged.
- `e2_slots_evaluators_is_a_known_path` is unchanged and green.
- New: `exit_codes_tests::the_status_mapping_reads_the_exit_code_table` in commands/mod.rs.

## Bites
- Row meaning changed in the table, pages not regenerated: red on d1 (configuration-paths.md line 20).
- `slots.count` moved to the project file without regenerating: red on d1 (config-file.md line 24).
- A generated cell edited by hand: red on d1 (configuration-paths.md line 52).
- An `Exit` variant added with no row: red at compile time (non-exhaustive match in `status`).
- A hand `matches!` arm beside the table in `is_known_path`: not seen by any test; left to the verifier. The implementer kept no hand arm.

## D3
- d1 (existing, extended): question, do the pages equal their owners; role guard; bite above; oracle the committed pages.
- exit-code unit test: question, does the process status equal the table row; role guard; bite the new variant; oracle the table read by position. The table is read by `syn` for the page, so a row's text cannot differ from the source.

## Restamps
- No `[checked: …]` stamp changed in `docs/contract/`; the two new pages have empty Notes.

## Deviations
- `main.rs` `status` still types 0, 1 and 2. The leaf allowed only `EXIT_CODES` and `after_help` in `main.rs` because of the rebase; the unit test ties `status` to the table instead. After `output_contract` merges, `status` can read `EXIT_CODES`.
- Steps 2 to 4 were committed as four commits with the leaf's subjects, in the order table, exit codes, pages, no-store row; the paths page and the file page share one commit, since one generator and one index feed both. The first-reads report is one commit with this report.

## Verifier
One verifier (fresh, Sonnet). Findings and dispositions:
- F1 (`is_known_path` accepted `budget.per_run.bogus` through the `budget` family): fixed in `reaches`; guard `a_misspelt_per_run_cap_is_not_known` in paths.rs. It was a behaviour change the first pass of the leaf introduced; no existing guard saw it.
- F2 (typed readers and the machine-wide check in config.rs still name `slots`, `store`, `plugins`, `gateway` by literal; `config.rs` holds `gateway.upstream_ca` twice; `common.rs` has a hand `STORE_ROOT`; `is_location` is a hand list): not fixed. Only the key-name check reads the table; the value readers need typed access per key. Follow-up, with a second leaf if wanted.
- F3 (`usage_failure` returns 2 and clap's status outside the table; `Exit` doc comments say the codes): the length of the table is now asserted by the unit test; the rest is follow-up after `output_contract` merges.
- F4 (uncommitted fmt changes): committed with the fix.
- Minor: `names()` prints "other" for a new kind; the code lists per page are hand picks. Not changed.
Re-check by the same verifier, final verdicts:
- F1 closed (guard not run by the verifier; run by the implementer, green).
- F2 not a blocker, partly met: the key lists have one owner (`check_keys` reads `config_keys` over `CONFIGURATION_PATHS`; `PROJECT_TABLES` and `USER_TABLES` are gone). "Named once" does not hold for the value readers' field names and the machine-wide `slots` check, which are all in `config.rs` at e016b320 and are not new. `const KEY = "gateway.upstream_ca"` predates e016b320 (line 279); the new duplicate is the private `GATEWAY_UPSTREAM_CA` in paths.rs, the disclosed schemas-crate limit. Read the gate as "key lists named once".
- F3 closed for the table length; `ExitCode::from(2)` in `usage_failure` and clap's own status stay outside `EXIT_CODES` (low, follow-up).
- Clippy `cargo clippy -p candler-core -p candler --all-targets -- -D warnings` on the tip before this commit: clean.

## Follow-ups
- F2 and F3 above.
- `treatment:` row meaning was written from the constant's name; not checked against the treatment reader.
