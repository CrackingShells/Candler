# m2c-wire-alignment

## Context
Milestone M2c of `candler-v1`. It runs off `main` at b017d31 (`v0.5.0`, `schemas@0.5.0`), on `feat/m2c-wire-alignment`. It closes amendment A1 (the `experiment/1` wire type gets inputs, whole table values and a treatment source), amendment A2 as widened by the conformance review (no HTN-domain concept in the core, the binary or `assets/`), and review item E4 (what a build outside a git checkout records).
Depth is ordering:
- **L0, this level,** holds five leaves that share no file but one test file: the wire types, the agent environment, the import rules, the build identity, and the verdict.
- **L1, `consumers/`,** wires the new types into the core and writes R01.
- **L2, `closeout/`,** proves the input seam end to end, rebuilds the I1 guard and builds the real env image.
- **L3, `revalidation/`,** measures the digests and re-runs M2b's smoke.

## Reference Documents
- [R01 Architecture](../../__design__/architecture.md) — §Experiment file, §Run manifest, §Plug-ins (the sections this milestone edits), §Evaluator protocol (run context, `needs`), §Agent environment and secrets (the configuration surface), §Invariants (I1, I7, I8, I12, I14, I17)
- [R02 Meta leaf](../candler-v1/contracts/parity/alignment/m2c_wire_alignment.md) — the gates this campaign decides
- [R03 Gap analysis A1](../../__reports__/m2c-wire-alignment/gap_analysis_v0.md) — F5, F7, the treatment source
- [R04 Gap analysis A2](../../__reports__/m2c-wire-alignment/gap_analysis_planner_v0.md) — the planner axis
- [R05 Conformance review](../../__reports__/candler-v1-review/00-findings_v0.md) — table A (D0–D5, G), E4
- [R06 M2b records](../../__reports__/m2b-execution-parity/04-knowledge_transfer_v0.md) — the KT, the two smoke reports beside it, and the feature map with its checks under `../m2b-execution-parity/feature_map/`
- [R07 M2a KT](../../__reports__/m2a-evaluation-parity/02-knowledge_transfer_v0.md) — "What M2c needs from M2a"
- [R08 Digest baseline](../../__reports__/v1-review-code-gaps/digest_invariance_v0.md) — 240 runs and 10,320 records (golden), 6 runs and 136 records (mini)
- [R09 CONTRIBUTING](../../CONTRIBUTING.md) — commit types and scopes, the schema line, the schemas-release rule
- [R10 Approved plan](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-approved-plan.md) — decisions D1–D9, the guard table, the probes, what is deliberately not tested
- [R11 Implementer brief](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m2c-implementer-brief.md) — read order, environment, commit flow, the leaf report

## Goal
An experiment file declares its inputs, its whole table values and its treatment's source, and runs need no `--set`. The core, the binary and `assets/` hold no HTN-domain concept, and the I1 guard can see one. No cell digest of the golden import or of the parity fixtures moves.

## Pre-conditions
- [x] [nl] Eliott approved the plan and decisions D1–D9 (2026-11-10)
- [ ] [cmd] The milestone branch starts at `main`: `git merge-base --is-ancestor b017d31 feat/m2c-wire-alignment`
- [ ] [cmd] The golden corpus is in place: `test -d /Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`
- [x] [cmd] The P0 baseline holds on `main` (b017d31, 2026-11-10): every gate binary that exists there passes (`guard_layering_i1` 4/4 despite the 82 segment hits, `guard_experiment_file` 3/3 pinning the flattening, `guard_comparability` 2/2, `guard_cell_digest` 9/9, the core digest set 36/36, DRIFT 2/2, GOLDEN 2/2 + 1/1); the new guards' filters exit 4 (`--no-tests=fail`); G6a/b/c and the G4 grep fail; G6d lists 9 undescribed paths

## Success Gates
- ⬜ [cmd] G1 inputs: guards I1, I4 and W3 pass
- ⬜ [cmd] G2 I8: guards I2, EV1, EV2 and S1 pass
- ⬜ [cmd] G3 tables: guards W1 and W2 pass, and the smoke's committed command lines hold no `--set`: `! grep -n -- '--set' __reports__/m2b-execution-parity/smoke_v2/commands.sh`
- ⬜ [cmd] G4 source: guards T1 and T2 pass, and `! git grep -nF 'request.treatment_source' -- ':(glob)crates/*/src/**'`
- ⬜ [cmd] G5a no expected digest is edited: `git diff --numstat b017d31 -- crates/candler/tests/oracle crates/candler-core/tests/oracle/vectors crates/candler-schemas/tests/vectors ':(exclude)crates/candler-core/tests/oracle/vectors/field_paths.toml' | awk '$2!=0{b=1}END{exit b}'`
- ⬜ [cmd] G5b `field_paths.toml` loses exactly its `request.treatment_source` row and gains the rows of the new settings: `git diff b017d31 -- crates/candler-core/tests/oracle/vectors/field_paths.toml | grep -c '^-path' | grep -qx 1`
- ⬜ [cmd] G5c the digest guards pass: `cargo nextest run -p candler-schemas --no-tests=fail -E 'binary(guard_cell_digest)' && cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_import_mini)|binary(guard_legacy_mapping)|binary(guard_oracle_mini)|binary(guard_run_context)|binary(guard_engine)|binary(guard_seams)'`, and GOLDEN passes
- ⬜ [nl] G5 measured: `__reports__/m2c-wire-alignment/digest_invariance_v1.md` records 0 moved run keys, context digests and manifests on the golden and mini imports, `main` against the branch
- ⬜ [cmd] G6a planner: `! git grep -n -i -E 'gtpyhop|planner' -- ':(glob)crates/*/src/**' assets`
- ⬜ [cmd] G6b assets: `! git grep -n -E 'gtpyhop|libcst|python-arango|python-dotenv|jsonschema|domain\.py|problems\.py' -- assets`
- ⬜ [cmd] G6c importer: `! git grep -n -w -i tier -- ':(glob)crates/*/src/**'` and `! git grep -n ENTRY_PARAMS -- ':(glob)crates/*/src/**'`
- ⬜ [cmd] G6d provenance: every `provenance.*` path Candler writes is described in R01 §Run manifest: `for p in $(git grep -ohE '"provenance\.[a-z_.]+' -- ':(glob)crates/*/src/**' | tr -d '"' | sort -u); do sed -n '/^### Run manifest/,/^### Normalised/p' __design__/architecture.md | grep -qF "$p" || echo "$p"; done` prints nothing. And `! git grep -n skill_version -- ':(glob)crates/*/src/**'`
- ⬜ [cmd] G6e guard: guards L1–L3 pass, and the layering verifier's mutation runs each turn `guard_layering_i1` red
- ⬜ [cmd] G6g environment: guards A1, A2 and X1 pass (X1 in CI's `container-e2e` job, and locally when Docker is up)
- ⬜ [nl] G6f smoke: `planner_pin` and `provenance_fields` pass through the committed projection, and every moved live HTN cell digest is listed in the smoke report
- ⬜ [cmd] G7a exactly one release-triggering commit touches the schemas crate, and it is `feat(schemas)`: `test "$(git log --no-merges --format=%s b017d31..HEAD -- crates/candler-schemas | grep -cE '^(feat|fix|perf|refactor|style|revert)')" -eq 1 && git log --no-merges --format=%s b017d31..HEAD -- crates/candler-schemas | grep -qE '^feat\(schemas\): '`
- ⬜ [cmd] G7b DRIFT holds and `Cargo.lock` is unchanged: `git diff --quiet b017d31 -- Cargo.lock`
- ⬜ [cmd] G7c the evaluator helper's generated models are unchanged: `(cd python/candler-evaluator && uv run --frozen python scripts/gen_models.py && test -z "$(git status --porcelain -- src)")`
- ⬜ [cmd] G8 build identity and verdict: guards E1 and V1 pass
- ⬜ [cmd] STD and LINUX pass on `feat/m2c-wire-alignment`; SUBJ passes over `b017d31..feat/m2c-wire-alignment`
- ⬜ [cmd] No confirm is left: `! git grep -n 'fn confirm_' -- ':(glob)crates/**'`
- ⬜ [nl] Each leaf report records the bite of every guard it introduced, and every verifier named in the leaf closed with no open finding
- ⬜ [cmd] The PR to `main` is titled `merge(experiment): declare inputs and sources, keep the domain out` (72 characters with ` (#14)`), and its CI is green: `gh pr checks <N>`

## Gotchas
- **Guard ids and filters.** Guard ids (W1 … X1) are those of R10 §Test design, each named in the leaf that writes it. Every nextest call carries `--no-tests=fail`.
- **Condition tags.** Every Pre-condition and Success Gate line starts with `[cmd]` (decided by a command's exit status) or `[nl]` (a judgement).
- **Standard gates**, named in every leaf:
  - **STD**: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`. Implementers run the `-p` forms of their crates; the coordinator runs the workspace form once per merge;
  - **LINUX**: `cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings`;
  - **DRIFT**: `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas -E 'binary(schema_drift)' && test -z "$(git status --porcelain crates/candler-schemas/schema)"`;
  - **SUBJ**: `cog check <base>..HEAD && git log --format=%s <base>..HEAD | awk 'length>72{b=1}END{exit b}'`, `<base>` being the leaf's branch point;
  - **SPAWN**, before any test run that may spawn a process: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)'`;
  - **GOLDEN**: `export CANDLER_GOLDEN_CORPUS="$GOLDEN" && cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_golden)' && cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'binary(guard_oracle_ac)'` (about 60 s), with `$GOLDEN=/Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`. Read it; never write in it.
- **Build footprint (shared machine).**
  - Export `RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/private/tmp/claude-501/m2c-target-<leaf>` for every cargo command, one target dir per worktree, outside it. Never share one between worktrees: cargo names a workspace member's artifacts by its path relative to the workspace root, so two worktrees write the same names and a build can link the other worktree's code (seen in `agent_env`: "BuildSetting has no field named prune"). sccache shares the compile work. The coordinator's tip runs use `/private/tmp/claude-501/m2c-target`, which only it uses;
  - Each leaf deletes its target dir before its last report; a fix round recreates it.
  - At most three leaves are open and building at once.
  - The coordinator removes a leaf's worktree as soon as it is merged, and the target dir at close-out.
- **Safety.** The rules of M2b's Gotchas hold:
  - signals go through the I22 guard only, and containers are destroyed only through `ContainerGuard`;
  - bite tests of stopping or destroying use the recording fakes of `test-support`;
  - never `kill`, `pkill`, `killall`, `docker rm -f $(…)`, `docker … prune`, or a `docker stop`/`rm` of an id the test did not create;
  - a real-daemon test removes only the containers, images and networks it created, by exact id.
- **Digests.**
  - Cell digests come from `comparable`; context digests come from the context projection; run keys come from the cell digest and the replicate.
  - A digest that moves is a defect to report, never a value to bless. An expected digest is never edited "until green" (G5).
  - Live HTN cell digests move when `stable.in` leaves the lock. That is expected, and P3 and the smoke list them.
- **Schema commits.**
  - Only `wire.md` touches `crates/candler-schemas/`. It does so in exactly one release-triggering commit, `feat(schemas): …`, which carries the DRIFT bless and touches no other crate. Its unit-test literals go in a separate `test(schemas)` commit.
  - CI's schemas-release check refuses anything else.
  - A wire defect found after `wire.md` lands goes to Eliott before any second schemas commit.
- **Breaking changes.** The body of a `!` commit names the migration path:
  - `feat(isolation)!`: `stable.in` is gone, so an experiment declares its packages in `isolation.build.requirements`;
  - `feat(importer)!`: rules files must name `entry_params`;
  - `feat(experiment)!`: `request.treatment_source` is replaced by `[treatment] source`.
- **Test design** (`designing-tests`; R10 §Test design).
  - Every new guard is written first and seen red. Its doc carries a `Guards:` line and an `Oracle:` line naming an expectation derived without Candler's code.
  - Its bite (the plausible bug that turns it red) is recorded in the leaf report.
  - Confirms are named `confirm_*` and are culled or promoted before the leaf merges.
  - Probe answers go to the leaf report, and probe scripts are not committed.
  - Guards carry the fixed name prefixes the gates filter on: `declared_input_`, `evaluator_only_input_`, `table_value_`, `source_only_`.
- **Shared constraints** (one owner each; a leaf that needs to change one reports it instead):
  - **Names.** Every leaf uses these names and no other:
    - the schemas constant `candler_schemas::execution::TABLE_VALUED_PATHS` (`["prompt.values"]`);
    - the experiment's `[[inputs]]` entries `{kind, path, at?, audience}`, with `audience` one of `agent` (the default) or `evaluators`;
    - the type `TreatmentSpec {name, tag?, commit?, source?}`;
    - the manifest field `evaluation_inputs[] {kind, digest}`;
    - the optional `from` of `ValueSource::Derived`;
    - the settings `isolation.build.prune` and `isolation.record_packages`;
    - the measured paths `provenance.packages.<import>.{distribution,version}`.
  - **Input blobs.**
    - Every file of an input is a store blob under its sha256.
    - The input's list `[{path, sha256}]`, sorted by `path`, is a blob whose bytes are its RFC 8785 form.
    - The input's digest is that blob's key.
    - `path` is relative to the input's root. A file input is a list of one entry, named by its file name.
  - **One deriver of `inputs`.** The image step (`resolve/steps/image.rs`) is the only step that derives the `inputs` root. It merges `build_inputs` with the agent inputs that the setting `request.inputs` carries, sorted.
    - `run.rs` hashes and stores the inputs before the job, and puts the agent list in `request.inputs`.
    - It puts the per-file mounts in `JobSpec.inputs` and the evaluator list in `JobSpec.evaluation_inputs`.
    - An evaluator-only input's content never leaves the store.
  - **`request.treatment`** holds a `TreatmentSpec`. The identity step resolves its `source` against the experiment file's directory and records the source as written in `from`.
- **Merges.** The coordinator lands each leaf with `git merge --no-ff` after rebasing it onto the milestone tip, subject `merge(<scope>): land <leaf>`. Scopes: `wire` → `schemas`; `agent_env` and `env_image_e2e` → `isolation`; `import_rules` → `importer`; `eval_infra_origins` and `verdict_gating` → `manifest`; `treatment_source`, `declared_inputs` and `input_seam` → `experiment`; `evaluator_inputs` → `evaluation`; `design_record` → `design`; `layering_guard` → `repo`; `digest_invariance` → `experiment`; `equivalence_smoke` → `orchestrator`. The level pre-conditions key on these subjects.
- **Leaf reports.** Each leaf writes `__reports__/m2c-wire-alignment/leaves/<leaf>_v0.md` (`writing-reports`, type findings, no code dump): the guards and their bites, the probe answers, the deviations and the spec defects found.
- **Owned files.**
  - Each leaf lists its OWNED paths. Implementers never edit `__design__/` (except `design_record.md`), `__roadmap__/`, another leaf's files, `cog.toml`, `.github/` or `Cargo.lock`.
  - Two edits touch the same file and are merged by the coordinator: at L0, `guard_layering_i1.rs` (the allowance hunk and the builder hunk); at L1, adjacent lines in `orchestrate/session.rs::cells_of`, `runner/exec/cell.rs` and `resolve/layers.rs`.

## Status
```mermaid
graph TD
    wire[Wire]:::done
    agent_env[Agent env]:::done
    import_rules[Import rules]:::done
    eval_infra_origins[Eval infra origins]:::done
    verdict_gating[Verdict gating]:::done
    consumers[Consumers]:::inprogress
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `wire.md` | 📄 Leaf Task | ✅ Done |
| `agent_env.md` | 📄 Leaf Task | ✅ Done |
| `import_rules.md` | 📄 Leaf Task | ✅ Done |
| `eval_infra_origins.md` | 📄 Leaf Task | ✅ Done |
| `verdict_gating.md` | 📄 Leaf Task | ✅ Done |
| `consumers/` | 📁 Directory | 🔄 In Progress |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| `import_rules.md` | `m2c/import_rules` → aa1edf8 | 2 | `entry_params` from the rules; open outside the repo: the plug-in's `htn-results-rules.toml` needs the key and `guard_golden_parity`'s rules pin moves with it |
| `wire.md` | `m2c/wire` → 2aa7afd | 4 | verifier PASS after one fix round (UTF-16 order vector); one `feat(schemas)`; the leaf's `guard_cell_digest` bite could not hold (fields outside `comparable`); JSON empty `prompt.values` table now refused |
| `eval_infra_origins.md` | `m2c/eval_infra_origins` → 7ff91f1 | 3 | P1 used `candler-schemas`' real package (`candler` is `publish = false`); a vcs file wins over a surrounding checkout; recorder hardening (ambient variable, packager commit) queued |
| `agent_env.md` | `m2c/agent_env` → ea4905e | 4 | P3: the lock differs only in `# via` lines, live HTN digests move; found the shared target dir linking siblings' code (6ba21c7) |
| `verdict_gating.md` | `m2c/verdict_gating` → 3307ca8 | 3 | first commit fixes the core nuisance set `wire` left red; verifier: sort guarded, every gated field refused when unrecorded, empty commit refused |
