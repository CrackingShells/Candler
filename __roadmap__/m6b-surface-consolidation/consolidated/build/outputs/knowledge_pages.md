# Knowledge pages

**Goal**: The configuration path vocabulary has one owner, a const table in `candler-core::resolve` that the code reads, rendered as `docs/contract/configuration-paths.md` (C3, G20, H3's web page); the two configuration files' keys have one owner, rendered as `docs/contract/config-file.md` (X7); exit codes and value domains come from one `EXIT_CODES` constant feeding `--help` and the page (X3, narrow form, no examples); the `config.no_store` row says when it is raised, and stamps move only after a real check (X9).
**Estimate**: about 37 Sonnet calls ($1.5), cap 56; at 56 calls, stop, commit what is green, list what is left `[V]`
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (build README, Gotchas)
- [ ] [cmd] `codes_inner` is merged (it rewrites the Notes of `experiment-file.md` and `suite-file.md`): `git log --format=%s $BUILD_BASE..HEAD | grep -q '^fix(schemas)!: print the inner code'`
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/resolve/(layers|config|paths|mod)\.rs$|^crates/candler/src/(main|commands/mod|commands/refusal)\.rs$|^crates/candler/tests/it/(main|contract_pages|guard_cli_surface)\.rs$|^crates/candler/tests/it/support/contract_pages/[a-z_]+\.rs$|^crates/candler-core/tests/it/guard_resolve\.rs$|^docs/contract/[a-z-]+\.md$|^__reports__/m6b-surface-consolidation/build/knowledge_pages_v0\.md$`. `codes.toml` is not edited (the `config.no_store` text lives in `commands/refusal.rs` and the page is generated). `output_contract` edits `main.rs`, `commands/mod.rs` and `commands/refusal.rs` at the same level: this leaf merges second and rebases onto it; in those files it touches only `EXIT_CODES`, `after_help` and the `config.no_store` row
**Success Gates**:
- ⬜ [cmd] Targeted tests: `cargo nextest run -p candler --no-tests=fail -E 'test(/^contract_pages::/) | test(/^guard_cli_surface::/)' && cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_resolve::/)'`
- ⬜ [cmd] The two pages exist and are generated: `test -f docs/contract/configuration-paths.md && test -f docs/contract/config-file.md && CANDLER_BLESS_CONTRACT=1 cargo nextest run -p candler -E 'test(/^contract_pages::/)' >/dev/null && git diff --quiet -- docs/contract`
- ⬜ [cmd] No hand restamp: every `[checked: …]` stamp changed in `docs/contract/` is on a line the leaf also reworded, listed in the report; ownership check prints nothing; `cargo clippy -p candler-core -p candler --all-targets -- -D warnings`; DIGEST passes
- ⬜ [nl] Each key and path is named once: the strings stay the `candler_schemas::execution::paths` constants (no new literal), and which paths are settings, which are request keys and which file a key belongs to is said in one table, which `is_setting`, `is_known_path`, `REQUEST_KEYS` and the typed config tables read (or, in the fallback, which the typed tables own alone), with `path:line` evidence in the report
- ⬜ [nl] The report says which seam 3 shape was built (one table with a `file` column read by the typed tables, or the fallback: C3's table without the four config keys, pointing to the X7 page generated from the typed tables), with `path:line` evidence; each page keeps the five sections; the verifier closed with no open finding
**References**: [R16 §Verdicts C3, G20, H3, X7, X3, X9, §Seams 1 and 3, §D3 (rows C3, X7, X3), §Guards that change (`d1_regenerating_the_pages_changes_nothing`, `e2_slots_evaluators_is_a_known_path`)](../../../../../__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md); R18 `leaves/knowledge_as_data_v0.md` §C3 and `leaves/docs_and_skills_v0.md` §X7, `leaves/cli_contract_v0.md` §X3, `leaves/codes_review_v0.md` §X9; [R01 §Contracts & Invariants (generated facts)](../../../../../__design__/architecture.md); [CONTRIBUTING.md](../../../../../CONTRIBUTING.md) (the five-section rule)

## Step 1: Read the owners and decide the table's shape
**Goal**: The leaf knows, before code, whether one table can feed the typed config tables, and how the test crate can read `Exit`.
**Implementation Logic**:
1. Through colgrep (index `crates/candler-core/src/resolve` and `crates/candler` of this worktree, one at a time): `is_known_path` and `is_setting` (`resolve/layers.rs`; the key strings are already the `candler_schemas::execution::paths` constants such as `STORE_ROOT`, so the hand-written facts are the lists of which constant is what, not the strings), `REQUEST_KEYS` (`commands/refusal.rs`), `PROJECT_TABLES` and `USER_TABLES` and their typed parsing (`resolve/config.rs`), `Exit` (`commands/mod.rs`), the page table and the section-order check of the generator (`tests/it/support/contract_pages/`, `contract_pages.rs`), and how `crates/candler/tests/it/main.rs` includes source files by `#[path]`.
2. Decide seam 3's shape and record it in the report: (a) one const table with a `file` column (project, user, none) and a class (experiment path, config key), from which the typed tables take their key names; or (b) if the typed parsing cannot read it, the fallback: C3's table holds the experiment paths and points to the X7 page for the four config keys, and X7's page is generated from `PROJECT_TABLES` and `USER_TABLES`. In both, `is_setting` reads a table instead of its literals; if it cannot, stop and send the item to the coordinator.
3. Read the `config.no_store` row (`commands/refusal.rs:233-241`) against `resolved_root` (`commands/common.rs:89-104`) and the core default (`resolve/layers.rs:284-344`), to word when it is raised (seam 1).
4. Write the report section `## First reads`. No probe test is needed: the d1 guard already fails when a page differs from its source (R16 §D3).
**Deliverables**: `__reports__/m6b-surface-consolidation/build/knowledge_pages_v0.md`: section `## First reads` (the shape chosen, the `#[path]` include for `Exit`, the `config.no_store` wording)
**Consistency Checks**: `grep -q '^## First reads' __reports__/m6b-surface-consolidation/build/knowledge_pages_v0.md` (expected: PASS)
**Commit**: `reports(api): record the first reads of the knowledge pages build`

## Step 2: One table of configuration paths and its page
**Goal**: `is_known_path`, `is_setting` and `REQUEST_KEYS` read one const table, and `configuration-paths.md` renders it under the d1 guard.
**Implementation Logic**:
1. A const table in a new `candler-core/src/resolve/paths.rs` (the lists leave `layers.rs`): one row per path or path form, naming the existing `execution::paths` constant where one exists (`model.id`, `runner.mode`, `treatment:<path>`, the settings classes), with its class and, in shape (a), its file. Each row carries the one-line meaning the page prints.
2. `is_known_path` and `is_setting` read the table; `REQUEST_KEYS` is derived from it or reads it. `e2_slots_evaluators_is_a_known_path` stays unchanged and green.
3. Add `configuration-paths.md` to the generator's page table, in the existing five sections; Surface and Notes are hand sections. Regenerate with `CANDLER_BLESS_CONTRACT=1`.
4. Two commits: the table (`feat(resolve): …`), then the page (`docs(cli): …`).
**Deliverables**: `crates/candler-core/src/resolve/paths.rs`: `CONFIGURATION_PATHS`; `crates/candler-core/src/resolve/layers.rs`: `is_known_path`, `is_setting` reading it; `crates/candler/src/commands/refusal.rs`: `REQUEST_KEYS`; `crates/candler/tests/it/support/contract_pages/pages.rs` (the page entry); `docs/contract/configuration-paths.md`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_resolve::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^contract_pages::/)'` (expected: PASS)
**Commit**: `feat(resolve): read the configuration paths from one table`

## Step 3: The configuration file page
**Goal**: `config-file.md` lists the keys of `candler.toml` and of the user configuration from the same owner as the reader.
**Implementation Logic**:
1. Generate the page from step 1's owner: shape (a) the table's config-key rows by file; shape (b) `PROJECT_TABLES` and `USER_TABLES`, made readable to the generator (a `pub` accessor, no copy). `slots.*` and `gateway.*` are user configuration; a project file that sets them is refused, as the reader does.
2. No Example section (X6 is rejected); the five sections stay. Regenerate.
**Deliverables**: `crates/candler-core/src/resolve/config.rs` (the accessor, shape b only); `crates/candler/tests/it/support/contract_pages/pages.rs`; `docs/contract/config-file.md`
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/^contract_pages::/)'` (expected: PASS)
**Commit**: `docs(cli): generate the configuration file page from the reader's keys`

## Step 4: Exit codes from one constant, and the `config.no_store` row
**Goal**: `--help` and the CLI page print exit codes from `EXIT_CODES`; the `config.no_store` row says when it is raised.
**Implementation Logic**:
1. `EXIT_CODES` beside `Exit` in `commands/mod.rs`: each code, its meaning; `Exit`'s values are checked against it at compile time or by construction, never typed twice. `main.rs` sets `after_help` from it. The generator reads it through the `#[path]` include chosen in step 1 and renders the exit-code row on the CLI page (`index.md` or the page step 1 names). Value domains already come from clap; no examples (X3's narrow form). Commit `feat(cli): print exit codes in help from one constant`.
2. Reword the `config.no_store` row text in `commands/refusal.rs`: it is raised when no store root resolves at all (no `--store`, no `CANDLER_STORE`, no project file and no state home), per step 1.3; regenerate `docs/contract/index.md`. Stamps: a `[checked: …]` stamp changes only on a line this leaf re-read against the code and reworded; list each in the report. Commit `fix(cli): say when the no-store refusal is raised`.
**Deliverables**: `crates/candler/src/commands/mod.rs`: `EXIT_CODES`; `crates/candler/src/main.rs`: `after_help`; `crates/candler/tests/it/main.rs` and `support/contract_pages/`: the exit-code row; `crates/candler/src/commands/refusal.rs`: the `config.no_store` row; `docs/contract/index.md` (regenerated)
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/^contract_pages::/) | test(/^guard_cli_surface::/)'` (expected: PASS)
**Commit**: `fix(cli): say when the no-store refusal is raised`

## Step 5: Bites, the report and the verifier
**Goal**: The page guards are shown to bite, and the leaf is verified.
**Implementation Logic**:
1. Bites, each made, seen red and reverted: add a row to the paths table without regenerating (red on the d1 guard); make `is_known_path` accept a path by a hand `matches!` arm beside the table (no test can see this, so the verifier checks it, and if the implementer keeps any hand arm it adds R16 §D3's guard that every accepted path is a row); add a key to the user tables without regenerating (red on the d1 guard); add an `Exit` value without an `EXIT_CODES` row (red at compile time or on the page check); edit a generated section by hand (red on the d1 guard).
2. Write the report (build README, Every leaf), with the shape chosen and each restamped line.
3. Dispatch a fresh Sonnet verifier (model passed explicitly) with this brief verbatim, adding only the branch name. **Verifier brief:** "You verify the leaf `__roadmap__/m6b-surface-consolidation/consolidated/build/outputs/knowledge_pages.md` on branch <branch>. Read the build README (Gotchas), R16 (`__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md`) rows C3, G20, H3, X7, X3, X9, seams 1 and 3, and §D3, then the diff from the leaf's branch point and the code through colgrep. Assume something is wrong. Attack: a configuration key or path named by hand in two places (a literal left in `is_setting`, `REQUEST_KEYS` or the typed tables beside the table); a page that copies a fact instead of reading its owner; a generated section edited by hand; a page without the five sections, or with an Example section; examples added to `--help`; an exit code typed twice; a `config.no_store` text that does not match `resolved_root` and the core default; a `[checked: …]` stamp changed on a line nobody re-read; `codes.toml` edited; a key of `output_contract`'s records or options changed; a comment carrying history or ids. Report each finding with the step, `path:line` and the evidence. Change nothing."
4. Fix the findings; have the same verifier re-check through SendMessage; record each finding and its disposition in the report.
**Deliverables**: `__reports__/m6b-surface-consolidation/build/knowledge_pages_v0.md`: sections `## First reads`, `## Guards changed`, `## Bites`, `## D3`, `## Verifier`, `## Follow-ups`
**Consistency Checks**: `for s in 'First reads' 'Guards changed' Bites D3 Verifier Follow-ups; do grep -q "^## $s" __reports__/m6b-surface-consolidation/build/knowledge_pages_v0.md || exit 1; done` (expected: PASS)
**Commit**: `reports(api): record the knowledge pages build`
