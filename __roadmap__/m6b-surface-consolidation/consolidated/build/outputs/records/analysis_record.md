# The analysis record

**Goal**: `analyze` prints one versioned record, `analysis-report/1` in `candler-schemas` (arms with `spent`, entries, comparisons, runs, `summary`, and the interval's reading named `interval_reading`), cut once after `analysis_core` (X4, Q8); `--format text` and `csv` render from that record (F5); the comparability sense of "stratum" is renamed in core and in the record, while the block sense keeps the word (Q7); `inspect <run> --transcript` prints a run's stored normalised trace, one event per line (C9).
**Estimate**: about 51 Sonnet calls ($2.1), cap 77; at 77 calls, stop, commit what is green, list what is left `[V]`
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (build README, Gotchas)
- [ ] [cmd] `output_contract` is merged: `git log --format=%s $BUILD_BASE..HEAD | grep -q '^feat(cli)!: print refusals as json by default'`
- [x] [nl] The new word for the comparability sense of "stratum" is "comparability class", key `comparability_class` (Eliott, 2026-11-23, round 06). `docs_chores` uses the same word
- [ ] [nl] OWNED (regex): `^crates/candler-schemas/src/(analysis|lib|generate)\.rs$|^crates/candler-schemas/schema/analysis-report\.schema\.json$|^crates/candler-schemas/tests/.*$|^crates/candler-core/src/manifest/compare\.rs$|^crates/candler-core/src/analysis/[a-z_]+\.rs$|^crates/candler-core/src/trace/[a-z_]+\.rs$|^crates/candler/src/commands/(analyze|inspect)\.rs$|^crates/candler/tests/.*$|^crates/candler-core/tests/.*$|^docs/contract/(codes\.toml|[a-z-]+\.md)$|^__reports__/m6b-surface-consolidation/build/analysis_record_v0\.md$`. `blocking.rs`'s `Stratum` (a block) keeps its name. Refusal codes keep their names (append-only); `codes.toml` gains a row only if step 1 finds no "run without a trace" refusal. `experiment_env` edits `candler-schemas/src/lib.rs`, `generate.rs`, `codes.toml` and the generated pages at the same level: whichever merges second rebases and regenerates, never hand-merges a generated file
**Success Gates**:
- ⬜ [cmd] Targeted tests: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/transcript_lists_every_event_in_order/) | test(/^guard_analysis_report::/) | test(/^guard_analysis_summary::/) | test(/^guard_known_truth::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/every_data_verb_prints_its_record/) | test(/^guard_analyze::/) | test(/^guard_inspect::/) | test(/^guard_golden_design::/) | test(/^contract_pages::/)' && cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/drift/)'`
- ⬜ [cmd] No hand-built analysis document is left: `test -z "$(grep -n 'json!' crates/candler/src/commands/analyze.rs)"`
- ⬜ [cmd] The schemas commit touches one crate: `for h in $(git log --format=%H --grep='^feat(schemas)' $BASE_LEAF..HEAD); do test -z "$(git show --name-only --format= $h | grep -v '^crates/candler-schemas/')" || exit 1; done`; the `!` commit gives the renamed keys: `h=$(git log --format=%H --grep='^feat(cli)!' $BASE_LEAF..HEAD | head -1); git log -1 --format=%b $h | grep -q interval_reading`
- ⬜ [cmd] `codes.toml` only grows: `test -z "$(git diff $BASE_LEAF -- docs/contract/codes.toml | grep -E '^-[^-]')"`; ownership check prints nothing; `cargo clippy -p candler-schemas -p candler-core -p candler --all-targets -- -D warnings`; DIGEST passes
- ⬜ [nl] The report lists every identifier, printed key and doc line that held "stratum" or "strata" with its sense (block or comparability) and what it became; the record's fields equal what `analyze` printed before this leaf, except the renamed keys, each listed with its reason; the verifier closed with no open finding
**References**: [R16 §Verdicts X4, F5, C9, H7, Q7, Q8, §Seams 4 and 6 (the "stratum" row), §D1, §D3 (rows C9, X1), §`schemas@` releases](../../../../../../__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md); R18 `leaves/cli_contract_v0.md` (the D1 record) and `leaves/analysis_outputs_v0.md` §C9, §F5; [R01 §Analysis statistics, §Readings](../../../../../../__design__/architecture.md); [docs/statistics.md](../../../../../../docs/statistics.md) `:80`, `:165-167`

## Step 1: Read the trace, the two senses and the printed shape; write the guards, seen red
**Goal**: The leaf knows the `TraceFile` event shape, every "stratum" by its sense, and today's `analyze` keys; the two guards fail on the tip.
**Implementation Logic**:
1. Through colgrep (index `crates/candler-core`, `crates/candler-schemas` and `crates/candler` of this worktree, one at a time): `TraceFile` and its events (`candler-schemas/src/trace.rs`), how a sealed run's trace is stored and read (`candler-core/src/trace/`), and whether the registry has a refusal for a run without a stored trace; `arm_json`, `entry_json`, `comparison_json` and the `stratum` and `exact_strata` keys in `commands/analyze.rs`; `stratum_verdict` in `manifest/compare.rs`; `UNKNOWN_STRATUM`, `Strata`, `strata_of` in `analysis/report.rs`; `Stratum` in `analysis/blocking.rs`. Classify each identifier and printed key as the block sense or the comparability sense; record the table in the report.
2. Record today's `analyze` JSON keys (with `analysis_core`'s `summary` and `spent`) as the field list the record must keep.
3. Guards, each with a `Guards:` line:
   - `transcript_lists_every_event_in_order` in a new `crates/candler-core/tests/it/guard_transcript.rs` (registered in `main.rs`): on a stored trace fixture, the rendered transcript has one line per event, in the stored order, and none is dropped; it asserts the relation, so a new event kind needs no edit;
   - the `analyze` row of `every_data_verb_prints_its_record` (`guard_cli_surface.rs`, `output_contract`'s guard): `analyze` with no option parses as `analysis-report/1`, and with `--format text` exits 0.
**Deliverables**: `crates/candler-core/tests/it/guard_transcript.rs`: `transcript_lists_every_event_in_order`; `crates/candler-core/tests/it/main.rs`; `crates/candler/tests/it/guard_cli_surface.rs`: the `analyze` row; `__reports__/m6b-surface-consolidation/build/analysis_record_v0.md`: section `## First reads`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/transcript_lists_every_event_in_order/)'; cargo nextest run -p candler --no-tests=fail -E 'test(/every_data_verb_prints_its_record/)'` (expected: FAIL)
**Commit**: `test(analysis): guard the transcript and the analysis record`

## Step 2: Rename the comparability sense in core
**Goal**: In core, "stratum" means one block only; the comparability sense carries the ruled word.
**Implementation Logic**:
1. Rename the comparability-sense identifiers from step 1's table (in `manifest/compare.rs` and `analysis/report.rs`) to the ruled word; `blocking.rs`'s `Stratum` and the exact stratified test keep theirs. Doc comments follow.
2. Refusal codes keep their names (append-only); printed keys do not change in this commit (they change with the record, step 4).
3. No behaviour changes: the targeted tests pass unchanged.
**Deliverables**: `crates/candler-core/src/manifest/compare.rs`, `crates/candler-core/src/analysis/report.rs` (renamed identifiers)
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_analysis_report::/) | test(/^guard_known_truth::/) | test(/^guard_analysis_summary::/) | test(/compare/)'` (expected: PASS)
**Commit**: `refactor(analysis): name the comparability class apart from a block`

## Step 3: Cut `analysis-report/1`
**Goal**: The record exists in `candler-schemas` under `schema_drift`.
**Implementation Logic**:
1. `candler-schemas/src/analysis.rs`: the record with arms (each with `spent`: `cost_usd`, `runs_counted`, `runs_without_total`), entries (tested and omitted), comparisons, runs, `summary` (the decisions and the omitted entries), and the interval's reading as `interval_reading`; the comparability field takes the ruled word. Fields follow step 1.2's list; each difference is in the report.
2. Register it in `generate.rs` and `lib.rs`; regenerate with `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas drift`. A new record is a minor bump. This commit touches `crates/candler-schemas/` only.
**Deliverables**: `crates/candler-schemas/src/analysis.rs`: `AnalysisReport`, `Arm`, `Spent`, `Entry`, `Comparison`, `Summary`, `Decision`; `crates/candler-schemas/src/generate.rs`, `lib.rs`; `crates/candler-schemas/schema/analysis-report.schema.json`
**Consistency Checks**: `cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/drift/)'` (expected: PASS)
**Commit**: `feat(schemas): add the analysis report record`

## Step 4: `analyze` prints the record; `inspect <run> --transcript`
**Goal**: Both guards pass; every rendering of `analyze` comes from the record.
**Implementation Logic**:
1. Core converts `Report` (with `Summary` and `Spend`) into `AnalysisReport`; `analyze` prints it as JSON by default; `--format text` renders it (F5: arms, decisions and omitted entries); `--format csv` with `--table` renders from it. `json!` leaves `analyze.rs`.
2. Update `guard_analyze.rs` and `guard_golden_design.rs` where they read a renamed key (`reading` to `interval_reading`, the comparability key); any other change stops the implementer.
3. `inspect <run> --transcript`: a core function in `trace/` renders a stored trace's events; JSON prints each event as stored, one object per line, and `text` one line per event. A run without a stored trace is refused with the code step 1 found, or a new appended row (regenerate the pages). The flag is "what to show", not a format; its doc comment says why it cannot be derived.
4. Commit `feat(cli)!:` with the migration in the body: each renamed key, old and new (`reading` to `interval_reading`, the comparability key), the new keys, and `--format text` for text.
**Deliverables**: `crates/candler-core/src/analysis/` (the conversion), `crates/candler/src/commands/analyze.rs` (record, text, csv), `crates/candler-core/src/trace/` (`render_transcript`), `crates/candler/src/commands/inspect.rs` (`--transcript`)
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/transcript_lists_every_event_in_order/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/every_data_verb_prints_its_record/) | test(/^guard_analyze::/) | test(/^guard_inspect::/) | test(/^guard_golden_design::/)' && test -z "$(grep -n 'json!' crates/candler/src/commands/analyze.rs)"` (expected: PASS)
**Commit**: `feat(cli)!: print the analysis report record and a run's transcript`

## Step 5: Bites, the report and the verifier
**Goal**: The guards are shown to bite, and the leaf is verified.
**Implementation Logic**:
1. Bites, each made, seen red and reverted: drop one event kind from the renderer, or sort events by kind (red on `transcript_lists_every_event_in_order`); print one `analyze` key with `json!` beside the record (red on the `analyze` row); rename a block-sense identifier too and run the targeted tests (state what turned red, or that nothing could, so the verifier checks it).
2. Write the report (build README, Every leaf), with step 1's tables.
3. Dispatch a fresh Sonnet verifier (model passed explicitly) with this brief verbatim, adding only the branch name. **Verifier brief:** "You verify the leaf `__roadmap__/m6b-surface-consolidation/consolidated/build/outputs/records/analysis_record.md` on branch <branch>. Read the build README (Gotchas), R16 (`__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md`) rows X4, F5, C9, Q7, Q8, seams 4 and 6 and §D1, then the diff from the leaf's branch point and the code through colgrep. Assume something is wrong. Attack: a fact `analyze` printed before that the record lacks, or a field changed with no reason in the report; a text or CSV rendering that reads anything but the record; a block-sense "stratum" renamed, or a comparability-sense one left; a refusal code renamed or a `codes.toml` row changed; `interval_reading` missing or `reject`'s meaning changed; a transcript that drops, reorders or re-shapes stored events, or a new record invented for it; a schemas commit touching another crate; a `!` commit without its renamed keys; a statistic computed in the CLI; an oracle read from the code under test; a comment carrying history or ids. Report each finding with the step, `path:line` and the evidence. Change nothing."
4. Fix the findings; have the same verifier re-check through SendMessage; record each finding and its disposition in the report.
**Deliverables**: `__reports__/m6b-surface-consolidation/build/analysis_record_v0.md`: sections `## First reads`, `## Guards changed`, `## Bites`, `## D3`, `## Verifier`, `## Follow-ups`
**Consistency Checks**: `for s in 'First reads' 'Guards changed' Bites D3 Verifier Follow-ups; do grep -q "^## $s" __reports__/m6b-surface-consolidation/build/analysis_record_v0.md || exit 1; done` (expected: PASS)
**Commit**: `reports(api): record the analysis record build`
