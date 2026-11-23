# One output contract

**Goal**: Every data verb prints JSON by default on stdout and stderr, from a record in `candler-schemas`, and one shared `--format` option asks for the text rendering of the same record (X1, G11, X10, D1, Q1, Q2); the `inspect` and small records land (X4, phase two and the inspect half of phase one); `resolve` prints C1's summary; its printed store root names its real source (N8). `--error-format`, `watch --json` and `resolve --json` retire, with no alias.
**Estimate**: about 68 Sonnet calls ($2.8), cap 102; at 102 calls, stop, commit what is green, list what is left `[V]`
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (build README, Gotchas)
- [ ] [cmd] `analysis_core` is merged: `git log --format=%s $BUILD_BASE..HEAD | grep -q '^feat(cli): print the summary and the spend' && git log --format=%s $BUILD_BASE..HEAD | grep -q '^feat(analysis): derive a decision summary'`, and `preflight_checks` (its `PlanOutcome::summary`): `git log --format=%s $BUILD_BASE..HEAD | grep -q '^feat(resolve): refuse a pinned commit that lacks the skill tree'`
- [ ] [nl] OWNED (regex): `^crates/candler/src/(main|lib)\.rs$|^crates/candler/src/commands/[a-z_]+\.rs$|^crates/candler-schemas/src/(inspect|listing|lib|generate)\.rs$|^crates/candler-schemas/schema/[a-z0-9-]+\.schema\.json$|^crates/candler-schemas/tests/.*$|^crates/candler-core/src/runner/plan\.rs$|^crates/candler/tests/.*$|^crates/candler-core/tests/.*$|^docs/.*\.md$|^scripts/.*$|^python/.*$|^\.claude/skills/.*$|^__design__/architecture\.md$|^__reports__/m6b-surface-consolidation/build/output_contract_v0\.md$`. In `candler-core`, only a conversion into a record; in R01, only §CLI surface and the lines that name `--error-format`, `watch --json` or `resolve --json`; in tests, only the guards that parse the old shapes (R16 §Guards that change) and the new ones; `analyze.rs` and `inspect.rs` keep `analysis_core`'s keys. `knowledge_pages` edits `main.rs`, `commands/mod.rs` and `commands/refusal.rs` at the same level and rebases onto this leaf
**Success Gates**:
- ⬜ [cmd] Targeted tests: `cargo nextest run -p candler --no-tests=fail -E 'test(/^guard_cli_surface::/) | test(/^guard_cli_observe::/) | test(/^guard_cli_refusal::/) | test(/^guard_inspect::/) | test(/^guard_analyze::/) | test(/^guard_cli_diff_refusal::/) | test(/^contract_pages::/)' && cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/drift/)'`
- ⬜ [cmd] The option is gone: `test -z "$(git grep -nE 'ErrorFormat|error_format' -- crates/candler/src)"`
- ⬜ [cmd] Every caller is swept: `test -z "$(git grep -nE -- '--error-format|watch --json|resolve --json' -- . ':!__reports__' ':!__roadmap__' ':!CHANGELOG.md' ':!**/CHANGELOG.md')"`
- ⬜ [cmd] Every `!` commit gives its migration: `git log --format='%H %s' $BASE_LEAF..HEAD | awk '$2 ~ /!:$/{print $1}' | while read h; do git log -1 --format=%b $h | grep -q -- '--format text' || exit 1; done`
- ⬜ [cmd] Each schemas commit touches one crate: `for h in $(git log --format=%H --grep='^feat(schemas)' $BASE_LEAF..HEAD); do test -z "$(git show --name-only --format= $h | grep -v '^crates/candler-schemas/')" || exit 1; done`
- ⬜ [cmd] Ownership check prints nothing; `cargo clippy -p candler -p candler-schemas -p candler-core --all-targets -- -D warnings`; DIGEST passes, or its failure is a CLI output fixture the coordinator rules with this leaf's rows cited
- ⬜ [nl] The supervisor's conditions hold: no deprecation period and no alias (clap's own unknown-flag error, no refusal row, no remedy hint); `!` commits with their migration; every in-repo caller swept in this merge; the migration note for the HTN lane is in the report; a golden or parity test that compares CLI output moved only with this leaf's rows cited
- ⬜ [nl] JSON is never derived from a terminal: no TTY check chooses a format; the verifier closed with no open finding
**References**: [R16 §D1, §Verdicts X1, G11, X10, X4, C1, N8, §Seams 4, 5, 7, §Guards that change, §Design suggestions (the supervisor's conditions)](../../../../../__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md); R18 `leaves/cli_contract_v0.md` (the D1 record, the option and the verb table) and `leaves/round02_candidates_v0.md` §N8; [R01 §CLI surface](../../../../../__design__/architecture.md); [R19 CONTRIBUTING.md](../../../../../CONTRIBUTING.md) `:20`, `:82`

## Step 1: Read the verbs R16 left unknown and write the guards, seen red
**Goal**: The verb matrix is known before any code, and the guards fail on the tip.
**Implementation Logic**:
1. Through colgrep (index `crates/candler` and `crates/candler-schemas` of this worktree): what `run`, `kill`, `ingest` and `evaluate` print on stdout; `show`'s row types (are they `outcome-record`?); whether `watch` prints the committed `event-envelope` type; which errors `--error-format` covers today, including clap usage errors raised before parsing (`main.rs`, the pre-parse detection); how `resolve` prints its resolution and why the store root's source is lost (`preflight.rs` `request_settings`). Record a table in the report: verb, prints today, record (existing, new here, or none), default after this leaf.
2. A verb found to print a data document in a hand shape that R16 did not list goes to the coordinator before it is built (scope); a verb whose output is already a schema type only gets the option and the guard row.
3. Guards and tests, each with a `Guards:` line:
   - `every_data_verb_prints_its_record` in `guard_cli_surface.rs`: one row per verb over the U1 matrix (`ps`, `ls`, `inspect` run and job, `diff`, `show`, `show --rates`, `resolve`, `watch`, and the verbs step 1 adds); each run with no option parses as its record and with `--format text` exits 0; the `analyze` row is `analysis_record`'s;
   - `refusals_print_json_by_default`: a refused call prints a `refusal/1` document on stderr with no option, and text with `--format text`, including a usage error;
   - `resolve_counts_equal_the_plan` (C1, confirm): on an existing plan fixture, the counts `resolve` prints equal `PlanOutcome`'s new and reused runs for the same experiment;
   - `resolve_names_the_store_source` (N8, probe): with `CANDLER_STORE` set, the printed `store.root` names the explicit source. If it passes on the tip, N8 closes as done, the probe is deleted and the report says so; if red, it becomes a guard.
**Deliverables**: `crates/candler/tests/it/guard_cli_surface.rs`: `every_data_verb_prints_its_record`, `refusals_print_json_by_default`, `resolve_counts_equal_the_plan`, `resolve_names_the_store_source`; `__reports__/m6b-surface-consolidation/build/output_contract_v0.md`: section `## First reads`
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/every_data_verb_prints_its_record|refusals_print_json_by_default|resolve_counts_equal_the_plan/)'` (expected: FAIL)
**Commit**: `test(cli): guard one output record per verb and json refusals`

## Step 2: The inspect and small records
**Goal**: `inspect-run/1`, `inspect-job/1` (with `analysis_core`'s `spent`) and the small records (job list, experiment listing with the `store` root used, diff, rates) exist in `candler-schemas` under `schema_drift`.
**Implementation Logic**:
1. New record types in `candler-schemas/src/inspect.rs` and `listing.rs`, registered in `generate.rs`, each `JsonSchema` and versioned; their fields are what the verbs print today plus the `store` root for `ps` and `ls` (G9's `done` relies on it). No field of `analyze` here.
2. Regenerate with `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas drift`. A new record is a minor bump (R19 schema table).
3. This commit touches `crates/candler-schemas/` only.
**Deliverables**: `crates/candler-schemas/src/inspect.rs`: `InspectRun`, `InspectJob`; `crates/candler-schemas/src/listing.rs`: `JobList`, `ExperimentList`, `DiffReport`, `Rates`; `crates/candler-schemas/src/generate.rs`; `crates/candler-schemas/schema/*.schema.json` (new files)
**Consistency Checks**: `cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/drift/)'` (expected: PASS)
**Commit**: `feat(schemas): add the inspect and listing output records`

## Step 3: One option, JSON by default on stdout
**Goal**: Every data verb takes `OutputArgs` and prints its record; `--format text` renders the record; `resolve` prints the summary and the real store source.
**Implementation Logic**:
1. `commands/mod.rs`: `OutputArgs { format }`, flattened into each data verb; `json` is not a value (Q2); values are `text` plus a verb's own renderings (`analyze` keeps `csv` with `--table`, its `text` is `analysis_record`'s; `show` keeps `toml`); clap refuses a wrong value at parse time. Its doc comment says why it cannot be derived ("it is how the output is read"), for U2.
2. Each verb builds its record (the conversion at the core boundary), prints it as JSON, or renders `fn text(&Record) -> String` from it; `ps` and `ls` TSV become their `text`; `show` and `diff` lose their required `--format`; `watch --json` and `resolve --json` retire.
3. `resolve` prints `PlanOutcome::summary()` in both renderings, and its resolution carries the store root with the source it was read from (N8, if step 1's probe was red).
4. Update U1, U2 and the guards that parse TSV or text (R16 §Guards that change) to parse JSON, or to pass `--format text` where text is what they guard.
5. Commit `feat(cli)!:` with the migration in the body: each verb's old and new default, `--format text` to get text back, `watch --json` and `resolve --json` removed.
**Deliverables**: `crates/candler/src/commands/mod.rs`: `OutputArgs`; every data verb's `Args` and printer under `crates/candler/src/commands/`; `crates/candler/src/main.rs`; the updated guards
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/every_data_verb_prints_its_record|resolve_counts_equal_the_plan/) | test(/^guard_cli_surface::/) | test(/^guard_cli_observe::/) | test(/^guard_inspect::/) | test(/^guard_analyze::/)'` (expected: PASS)
**Commit**: `feat(cli)!: print every data verb's record as json by default`

## Step 4: JSON refusals by default and the caller sweep
**Goal**: stderr is JSON by default, `--error-format` is gone with its coverage kept, and no in-repo caller uses a retired flag.
**Implementation Logic**:
1. The refusal prints as `refusal/1` JSON unless `--format text` was given; the pre-parse detection in `main.rs` keeps covering the errors `--error-format` covered (step 1), now looking for `--format text`. `ErrorFormat` and `error_format` are removed. No TTY check.
2. Sweep every caller of `--error-format`, `watch --json`, `resolve --json` and of the old TSV or text output: `scripts/`, `docs/` (CLI pages and examples; regenerate generated pages with `CANDLER_BLESS_CONTRACT=1`), the tests that read stderr as text, any skill text under `.claude/skills/`, and R01 §CLI surface (the sentence that `--error-format` is never derived from a terminal becomes the same rule for `--format`).
3. Commit `feat(cli)!:` with the migration in the body (`--error-format json` is now the default and is dropped; `--error-format text` becomes `--format text`); the sweep goes in `docs(cli): …` and `test(cli): …` commits.
**Deliverables**: `crates/candler/src/main.rs`, `crates/candler/src/commands/refusal.rs`; the swept files; `__design__/architecture.md` §CLI surface
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/refusals_print_json_by_default/) | test(/^guard_cli_refusal::/) | test(/^guard_cli_diff_refusal::/) | test(/^contract_pages::/)' && test -z "$(git grep -nE 'ErrorFormat|error_format' -- crates/candler/src)" && test -z "$(git grep -nE -- '--error-format|watch --json|resolve --json' -- . ':!__reports__' ':!__roadmap__' ':!CHANGELOG.md' ':!**/CHANGELOG.md')"` (expected: PASS)
**Commit**: `feat(cli)!: print refusals as json by default and retire --error-format`

## Step 5: Bites, the migration note and the verifier
**Goal**: The guards are shown to bite, the HTN lane's note is written, and the leaf is verified.
**Implementation Logic**:
1. Bites, each made, seen red and reverted: one verb prints a hand-built `println!` line (red on its row); a key dropped from a record but kept in `text` (red: text must come from the record); stderr in text by default (red on `refusals_print_json_by_default`); the printed new-run count off by the reused ones (red on `resolve_counts_equal_the_plan`).
2. Write the report (build README, Every leaf) with a `## Migration` section for the HTN lane's coordinator: each verb's default before and after, each retired flag and its replacement, the refusal on stderr, and the release it ships in (the next release after the supervisor's PR merges).
3. Dispatch a fresh Sonnet verifier (model passed explicitly) with this brief verbatim, adding only the branch name. **Verifier brief:** "You verify the leaf `__roadmap__/m6b-surface-consolidation/consolidated/build/outputs/output_contract.md` on branch <branch>. Read the build README (Gotchas), R16 (`__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md`) §D1, rows X1, G11, X10, X4, C1, N8, seams 5 and 7, §Guards that change and the supervisor's conditions under §Design suggestions, then the diff from the leaf's branch point and the code through colgrep. Assume something is wrong. Attack: a data verb that still prints a hand-built shape, or a text rendering with a fact its record lacks; `json` offered as a value; a format chosen from a terminal; an alias, a deprecation path, a refusal row or a remedy hint for a retired flag; an error class `--error-format` covered that now prints text by default; a caller of a retired flag left anywhere in the repository outside reports, roadmaps and changelogs; a `!` commit without the old invocation, the new one and `--format text`; a schemas commit touching another crate; a record field that differs from what the verb printed with no reason in the report; a key of `analysis_core` changed; `resolve`'s counts computed by a second planning pass; a guard rewritten to assert less than it did; a comment carrying history or ids. Report each finding with the step, `path:line` and the evidence. Change nothing."
4. Fix the findings; have the same verifier re-check through SendMessage; record each finding and its disposition in the report.
**Deliverables**: `__reports__/m6b-surface-consolidation/build/output_contract_v0.md`: sections `## First reads`, `## Guards changed`, `## Bites`, `## D3`, `## Migration`, `## Verifier`, `## Follow-ups`
**Consistency Checks**: `for s in 'First reads' 'Guards changed' Bites D3 Migration Verifier Follow-ups; do grep -q "^## $s" __reports__/m6b-surface-consolidation/build/output_contract_v0.md || exit 1; done` (expected: PASS)
**Commit**: `reports(api): record the output contract build`
