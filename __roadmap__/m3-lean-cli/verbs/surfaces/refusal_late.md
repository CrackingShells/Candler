# Refusal late

**Goal**: The refusals of `jobs`, `evaluation` and `import`, including the enums `evaluate_body` and `ingest_inspect` added and M4's new ones if M4 merged first, carry a code, side and path, so that every `thiserror` type of the workspace is coded or internal and 20 sampled variants answer correctly through the binary.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The core's codes are in: `git grep -q 'pub fn catalog' -- crates/candler-core/src/refusal.rs && git grep -q 'pub fn find' -- crates/candler-core/src/refusal.rs && git grep -q 'fn render' -- crates/candler/src/commands/refusal.rs && test -f crates/candler-core/tests/guard_refusal_codes.rs`
- [ ] [cmd] The enums this leaf must cover exist: `test -f crates/candler-core/src/jobs/runtime/evaluate.rs && test -f crates/candler-core/src/import/raw.rs && test -f crates/candler/src/commands/ingest.rs`
- [ ] [cmd] The scope oracle names the leaf: `awk -F'\t' '$NF=="refusal_late"' __roadmap__/m3-lean-cli/refusal_scope.tsv | grep -q .`
- [ ] [nl] OWNED: `crates/candler-core/src/{jobs,evaluation,import,manifest,trace}/refusal.rs` (`manifest::compare::VerdictError` and `trace::NormaliseError` included, coordinator ruling 2026-11-11; `compare.rs` itself is M4's and is not edited) and the `mod refusal;` line in each `mod.rs`, the same for any module of M4's new enums that the tsv gives to this leaf, `crates/candler-core/src/refusal.rs` (the catalog's list only), `crates/candler-core/tests/guard_refusal_codes.rs` (the pending list), `crates/candler/tests/guard_cli_refusal.rs`, `docs/contract/codes.toml` (`[jobs]`, `[evaluation]`, `[import]`, `[ingest]`, `[internal]`). Not the enums' own files, not `commands/evaluate.rs`
**Success Gates**:
- ⬜ [cmd] OWNED: `test -z "$(git diff --name-only <base>..HEAD | grep -vE '^(crates/candler-core/src/(refusal\.rs|(jobs|evaluation|import|manifest|trace)/(mod|refusal)\.rs)|crates/candler-core/tests/guard_refusal_codes\.rs|crates/candler/tests/guard_cli_refusal\.rs|docs/contract/codes\.toml$)')"`, `<base>` being the leaf's branch point; a further `<module>/refusal.rs` and its `mod` line are allowed for an enum of M4's that the tsv gives to this leaf
- ⬜ [cmd] R1 with an empty pending list: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^r1_/)'`, and `! git grep -n -i 'pending' -- crates/candler-core/tests/guard_refusal_codes.rs` (no enum is left waiting)
- ⬜ [cmd] R2 with every row live: `cargo nextest run -p candler --no-tests=fail -E 'test(/^r2_/)'`, and no row is on a pending list in `crates/candler/tests/guard_cli_refusal.rs`
- ⬜ [cmd] G-codes: `git diff --numstat a26fe53 -- docs/contract/codes.toml | awk '$2!=0{b=1}END{exit b}'`
- ⬜ [cmd] G-text: `git diff --numstat a26fe53 -- 'crates/candler/tests/guard_cli*.rs' | awk '$2!=0{b=1}END{exit b}'`. The text of every error is unchanged under `--error-format text`
- ⬜ [cmd] No wildcard decides a code: `! git grep -n -E '_ =>' -- 'crates/*/src/**/refusal.rs'`
- ⬜ [cmd] Every enum the oracle assigns to a leaf is in the catalog: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^r1_every_variant_code_equals_its_row/)'`
- ⬜ [cmd] STD (the `-p candler-core -p candler` forms), LINUX and SUBJ pass; the coordinator runs the workspace form
- ⬜ [nl] The leaf report records the bites:
  - R1: the pending list left non-empty with one enum; one enum of `jobs` with no row;
  - R2: the pending suite-params row left off; a code named after the function (`evaluate.failed`) instead of the reason;
  - the sample below: a variant coded with the wrong side
- ⬜ [nl] The verifier closes with no open finding. It covers `refusal_core` too (R10 §6): 20 variants of `refusal::catalog()`, triggered through the real binary under `--error-format json` from a temp directory, each judged on `side`, `path` and the reason in the code. Sample at least 6 from the schemas' rows, 6 from the core's, 8 from this leaf's, at least one per surface and per side, and list the variants no input can reach through the binary, with why
**References**: [R10 §0 (refusal codes cross the milestones), §5 R1–R2, §6](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m3-approved-plan.md); `__roadmap__/m3-lean-cli/verbs/refusal_core.md`; campaign README §Gotchas, "Shared constraints" (Refusal identity, `codes.toml`), "M4 overlap"

## Step 1: Register the codes
**Goal**: `codes.toml` holds a row for every author variant this leaf covers, and `[internal]` the rest, before any impl.
**Implementation Logic**:
1. The list is the tsv rows whose owner is `refusal_late`: today `jobs` (`JobError`, `RecoverError`, `OrchestrateError`, `RuntimeError`, `SlotError`, `JobViewError`), `evaluation` (`ClientError`, `ContextError`, `EvaluateError`, `PlanError` of `engine/plan.rs`, `RegistryError`, `NeedError`, `EquivalenceError`) and `import` (`ImportError`), plus what the two L1 leaves added: the enum of `jobs/runtime/evaluate.rs` and the one of `import/raw.rs`. If M4 merged to `main` first and the rebase carried its new error types, the coordinator has added them to the tsv; code them here, with the same rules. A type the tsv does not list but the scan of R1 finds is reported, not guessed.
2. `evaluate_body` and `ingest_inspect` leave their enums on R1's pending list and write no row of their own. If either wrote rows anyway, keep them and write the impl to match: the codes are append-only.
3. Hand-write each row in its section as in `refusal_core.md` Step 1: `<surface>.<reason>`, side, surface, enum, variant; never `candler` for an author error; wrapping variants delegate. Examples of the cause, not the function: `RegistryError`'s params failure is `suite.params_invalid` (side `suite`, path the pointer it names), a manifest that cannot be read is `manifest.unreadable`, a missing plug-in directory is `config.plugin_dir_missing` (side `config` or `environment`, as the tsv says).
4. `JobError`, `RuntimeError`, `SlotError` and the like are mostly Candler's own: they go to `[internal]` unless an author can cause the variant. Say why in a line each.
**Deliverables**: `docs/contract/codes.toml`: `[jobs]`, `[evaluation]`, `[import]` rows (`RawIngestError` included) and the `[internal]` additions
**Consistency Checks**: `git diff --quiet HEAD -- crates && git diff --numstat HEAD -- docs/contract/codes.toml | awk '$2!=0{b=1}END{exit b}'` (expected: PASS)
**Commit**: `docs(cli): register the codes of the late refusals`

## Step 2: Code the evaluation and job enums
**Goal**: `evaluation` and `jobs` implement `Refusal`, and the two evaluation rows of R2 go live.
**Implementation Logic**:
1. Move R2's suite-params row and missing-plug-in-directory row off the pending list first, and see them red: today the chain holds only `RegistryError` and `EvaluateError`, which have no code. The rows read through a helper in `tests/support/` that runs the evaluation verb: `evaluate_verb` (L2, a sibling) rewrites that verb's options, so only the helper changes when it merges; the coordinator resolves the one hunk.
2. `crates/candler-core/src/evaluation/refusal.rs` and `jobs/refusal.rs`, with `refusal_table!` as in `refusal_core.md` Step 2: exhaustive, no wildcard, with the `when` and `at` sentences of each row, `path()` by hand (`RegistryError`'s params failure returns the pointer it carries; a manifest that is not valid delegates to the schemas' `EvaluationError` path; variants naming a file give `None`). The enum of `jobs/runtime/evaluate.rs` is in; do not edit `evaluate.rs`.
3. Add the rows to `refusal::catalog()` in `crates/candler-core/src/refusal.rs`, and take the enums off R1's pending list.
**Deliverables**: `crates/candler-core/src/evaluation/refusal.rs`, `crates/candler-core/src/jobs/refusal.rs`: impls and `ROWS`; `crates/candler-core/src/{evaluation,jobs}/mod.rs`: `mod refusal;`; `crates/candler-core/src/refusal.rs`: `catalog`; `crates/candler-core/tests/guard_refusal_codes.rs`: pending list; `crates/candler/tests/guard_cli_refusal.rs`: the two rows
**Consistency Checks**: `cargo nextest run -p candler-core -p candler --no-tests=fail -E 'test(/^r1_/)|test(/^r2_/)' && ! git grep -n -E '_ =>' -- 'crates/*/src/**/refusal.rs'` (expected: PASS)
**Commit**: `feat(cli): code the refusals of jobs and evaluation`

## Step 3: Code the import enums
**Goal**: `import`, including the raw-trace enum, implements `Refusal`.
**Implementation Logic**:
1. `crates/candler-core/src/import/refusal.rs`: `ImportError` and the enum of `import/raw.rs`, through `refusal_table!`. The file keeps the module name `import`; the verb is `ingest` after the rename, but the codes are `import.*` or `ingest.*` as the rows already say: never rename a code.
2. If M4's new enums sit in `import/` (A4 changes imported `comparable`), add theirs in the same file, or in a `refusal.rs` of their module, as the tsv says. M4 is the milestone that merged first in that case; this leaf codes its variants (R10 §0).
3. Add to the catalog; take the enums off the pending list.
**Deliverables**: `crates/candler-core/src/import/refusal.rs`: impls and `ROWS`; `crates/candler-core/src/import/mod.rs`: `mod refusal;`; `crates/candler-core/src/refusal.rs`: `catalog`; `crates/candler-core/tests/guard_refusal_codes.rs`: pending list
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^r1_/)' && ! git grep -n -E '_ =>' -- 'crates/*/src/**/refusal.rs'` (expected: PASS)
**Commit**: `feat(cli): code the refusals of import and ingest`

## Step 4: Close the pending list
**Goal**: R1 has no pending list, so a new `thiserror` type fails the workspace until it is coded or internal.
**Implementation Logic**:
1. Delete the pending list and its mention from `guard_refusal_codes.rs`. Run `r1_` and see it fail when an enum is added to any crate's `src` with neither a row nor an `[internal]` entry (try it with a scratch enum, then remove it; keep nothing). Record that bite in the report.
2. Remove any pending list from `guard_cli_refusal.rs`: every row of R2 is live.
3. Run the sample the verifier will use, by hand, for five variants of the leaf's own enums through the binary, and read the records. A wrong `side` or `path` is a defect of the impl: fix it here.
**Deliverables**: `crates/candler-core/tests/guard_refusal_codes.rs`: `r1_…` without a pending list; `crates/candler/tests/guard_cli_refusal.rs`: `r2_…`
**Consistency Checks**: `cargo nextest run -p candler-core -p candler --no-tests=fail -E 'test(/^r1_/)|test(/^r2_/)' && ! git grep -n -i 'pending' -- crates/candler-core/tests/guard_refusal_codes.rs crates/candler/tests/guard_cli_refusal.rs` (expected: PASS)
**Commit**: `test(cli): require every error type to be coded or internal`
