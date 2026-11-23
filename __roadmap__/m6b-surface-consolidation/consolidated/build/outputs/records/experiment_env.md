# Experiment environment table

**Goal**: The experiment file gains an `env` table that sets a name and a value for the harness process; its values are cell values, so they are recorded and enter the cell digest, and an experiment without `env` hashes as today; names Candler sets (`PATH`, `HOME`, `VIRTUAL_ENV`, `LANG`, the gateway's and the driver's variables) and secret names are refused at `resolve`; the `secrets` description says that secret values reach the gateway only (N9b).
**Estimate**: about 20 Sonnet calls ($0.8), cap 30; at 30 calls, stop, commit what is green, list what is left `[V]`
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (build README, Gotchas)
- [ ] [cmd] `knowledge_pages` is merged (it moves the path lists out of `resolve/layers.rs`): `git log --format=%s $BUILD_BASE..HEAD | grep -q '^feat(resolve): read the configuration paths from one table'`
- [ ] [nl] OWNED (regex): `^crates/candler-schemas/src/(execution|lib|generate)\.rs$|^crates/candler-schemas/src/execution/[a-z_/]+\.rs$|^crates/candler-schemas/schema/experiment\.schema\.json$|^crates/candler-schemas/tests/.*$|^crates/candler-core/src/resolve/(layers|paths|refusal|mod)\.rs$|^crates/candler-core/src/resolve/steps/[a-z_]+\.rs$|^crates/candler-core/src/isolation/container/(run|exec)\.rs$|^crates/candler-core/tests/.*$|^docs/contract/(codes\.toml|[a-z-]+\.md)$|^__reports__/m6b-surface-consolidation/build/experiment_env_v0\.md$`. The drivers are read, not edited. `codes.toml` gains the refusal rows only (appended, no `remedy`). `analysis_record` edits `candler-schemas/src/lib.rs`, `generate.rs`, `codes.toml` and the generated pages at the same level: whichever merges second rebases and regenerates, never hand-merges a generated file
**Success Gates**:
- ⬜ [cmd] Targeted tests: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/env_enters_the_cell_digest|env_reaches_the_harness_process|reserved_env_names_are_refused/) | test(/^guard_resolve::/)' && cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/drift/) | test(/guard_cell_digest/) | test(/^guard_experiment_file::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^contract_pages::/)'`
- ⬜ [cmd] DIGEST passes: every existing cell digest vector is unchanged, which is the guard that an experiment without `env` hashes as today
- ⬜ [cmd] The schemas commit touches one crate: `for h in $(git log --format=%H --grep='^feat(schemas)' $BASE_LEAF..HEAD); do test -z "$(git show --name-only --format= $h | grep -v '^crates/candler-schemas/')" || exit 1; done`; `codes.toml` only grows: `test -z "$(git diff $BASE_LEAF -- docs/contract/codes.toml | grep -E '^-[^-]')"`; `! grep -Eq '^remedy *=' docs/contract/codes.toml`
- ⬜ [cmd] Ownership check prints nothing; `cargo clippy -p candler-schemas -p candler-core --all-targets -- -D warnings`
- ⬜ [nl] The report answers, with `path:line` evidence: whether `env` is additive within `experiment/1`; which code sets each refused name (the refused list is read from those places, never a hand copy); how the value reaches the harness process; the verifier closed with no open finding
**References**: [R16 §Verdicts N9b (both rows), §`schemas@` releases (release 4), §`codes.toml`, §Guards that change (digests)](../../../../../../__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md); R18 `leaves/n9b_env_scout_v0.md` (the scout: the overlay, secrets, `mcp.servers.<key>.env`); R17 round 05 (the N9b ruling); [R01 §Contracts & Invariants (cell values, I12)](../../../../../../__design__/architecture.md)

## Step 1: Read the reach of a value and write the guards, seen red
**Goal**: The leaf knows whether the table is additive, where each reserved name is set and how a value would reach the harness; the guards fail on the tip.
**Implementation Logic**:
1. Through colgrep (index `crates/candler-schemas`, then `crates/candler-core`, of this worktree): the experiment type and whether `experiment/1` refuses unknown fields (is an optional `env` table a minor addition, or does it need `/2`: if `/2`, stop and send it to the coordinator); how a cell value enters the cell digest (`resolve/layers.rs`, the cell identity); where the run container's environment is built (`isolation/container/run.rs:78-106`, secrets to the gateway only); where `PATH`, `HOME`, `VIRTUAL_ENV` and `LANG` are set for the harness, the gateway's variables, and the drivers' state variables (`candler-harness-claude-code/src/driver.rs:227-237` writes the overlay empty); whether TOML already refuses a duplicate name (a repeated key is a parse error) so that no row is needed for it.
2. Check that the existing cell digest vectors (`guard_cell_digest`, under DIGEST) cover an experiment resolved through the layers; if they do, "an experiment without `env` hashes as today" needs no new test (state it in D3); if they do not, add that case as a guard.
3. Guards, each with a `Guards:` line and the expected names written in the test:
   - `env_enters_the_cell_digest`: two experiments differing only in one `env` value give two cell digests, and the value is recorded with the cell;
   - `env_reaches_the_harness_process`: with the isolation double, the harness process of a run gets the name and value; the gateway does not;
   - `reserved_env_names_are_refused`: a table over `PATH`, `HOME`, `VIRTUAL_ENV`, `LANG`, one gateway variable, one driver variable and one name declared in `secrets`, each refused at `resolve` naming the key's path.
**Deliverables**: `crates/candler-core/tests/it/guard_resolve.rs`: `env_enters_the_cell_digest`, `reserved_env_names_are_refused`; `crates/candler-core/tests/it/guard_container_e2e.rs` or the isolation double's guard file: `env_reaches_the_harness_process`; `__reports__/m6b-surface-consolidation/build/experiment_env_v0.md`: section `## First reads`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/env_enters_the_cell_digest|env_reaches_the_harness_process|reserved_env_names_are_refused/)'` (expected: FAIL)
**Commit**: `test(resolve): guard the experiment env table`

## Step 2: The `env` table in the experiment file
**Goal**: The experiment schema has the table; the `secrets` description is true.
**Implementation Logic**:
1. In the experiment type, an optional `env` table of name to string value, absent by default and skipped when absent, so an experiment without it serialises as today; its doc comment says the values are cell values and reach the harness process.
2. The `secrets` field's doc comment: the names of the secrets the model gateway needs; their values reach the gateway only and are never written.
3. Regenerate with `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas drift`. A new optional field is a minor bump. This commit touches `crates/candler-schemas/` only.
**Deliverables**: `crates/candler-schemas/src/execution.rs` (or `execution/`): the `env` field; the `secrets` doc comment; `crates/candler-schemas/schema/experiment.schema.json` (regenerated)
**Consistency Checks**: `cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/drift/) | test(/guard_cell_digest/) | test(/^guard_experiment_file::/)'` (expected: PASS)
**Commit**: `feat(schemas): add an env table to the experiment file`

## Step 3: Wire the values and refuse the reserved names
**Goal**: The three guards pass; DIGEST is unchanged.
**Implementation Logic**:
1. `resolve`: `env` values enter the layered values as cell values with their source (I12), so they are recorded and enter the cell digest; an absent table adds nothing to the digest input.
2. The refusal: a name that Candler sets, or a name in `secrets`, is refused at `resolve` naming `env.<NAME>`. The reserved list is built from the constants the setting code already uses (step 1), never a new hand list; where a name is a literal in its setter, make it a constant there and read that.
3. `isolation/container/run.rs`: the harness process gets the `env` names and values; the gateway does not.
4. Append the refusal rows to `codes.toml` in the section of the code that raises them (`side` from the existing six, no `remedy`); regenerate the pages with `CANDLER_BLESS_CONTRACT=1`. Commit `feat(resolve): …` for the refusal and `feat(isolation): …` for the wiring.
**Deliverables**: `crates/candler-core/src/resolve/` (the layer and the refusal), `crates/candler-core/src/isolation/container/run.rs` (the harness environment), `docs/contract/codes.toml` (appended rows), the regenerated pages
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/env_enters_the_cell_digest|env_reaches_the_harness_process|reserved_env_names_are_refused/) | test(/^guard_resolve::/)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^contract_pages::/)'` (expected: PASS)
**Commit**: `feat(isolation): pass the experiment env table to the harness`

## Step 4: Bites, the report and the verifier
**Goal**: The guards are shown to bite, and the leaf is verified.
**Implementation Logic**:
1. Bites, each made, seen red and reverted: treat `env` as a setting, out of the digest (red on `env_enters_the_cell_digest`); write an empty `env` into the digest input when absent (red on DIGEST); drop `HOME` from the reserved list (red on its row); pass the table to the gateway instead of the harness (red on `env_reaches_the_harness_process`).
2. Write the report (build README, Every leaf), with step 1's answers.
3. Dispatch a fresh Sonnet verifier (model passed explicitly) with this brief verbatim, adding only the branch name. **Verifier brief:** "You verify the leaf `__roadmap__/m6b-surface-consolidation/consolidated/build/outputs/records/experiment_env.md` on branch <branch>. Read the build README (Gotchas), R16 (`__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md`) both N9b rows and §`schemas@` releases, `__reports__/m6b-surface-consolidation/leaves/n9b_env_scout_v0.md`, then the diff from the leaf's branch point and the code through colgrep. Assume something is wrong. Attack: an experiment without `env` whose digest input changed; an `env` value kept out of the digest or not recorded; a value reaching the gateway, or a secret value reaching the harness; a reserved name copied by hand rather than read from its setter, or a name Candler sets that the list misses; a refusal that does not name the key's path; a `codes.toml` row with a `remedy`, outside its raiser's section, or an existing row changed; `/2` cut silently; a schemas commit touching another crate; a driver edited; an oracle read from the code under test; a comment carrying history or ids. Report each finding with the step, `path:line` and the evidence. Change nothing."
4. Fix the findings; have the same verifier re-check through SendMessage; record each finding and its disposition in the report.
**Deliverables**: `__reports__/m6b-surface-consolidation/build/experiment_env_v0.md`: sections `## First reads`, `## Bites`, `## D3`, `## Verifier`, `## Follow-ups`
**Consistency Checks**: `for s in 'First reads' Bites D3 Verifier Follow-ups; do grep -q "^## $s" __reports__/m6b-surface-consolidation/build/experiment_env_v0.md || exit 1; done` (expected: PASS)
**Commit**: `reports(api): record the experiment env table build`
