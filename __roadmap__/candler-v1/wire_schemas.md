# Wire schemas: outcome record and catalog snapshot

**Goal**: The cargo workspace and its release tooling exist, and `candler-schemas` defines the outcome record and the tool-catalog snapshot, validated against real examples.
**Pre-conditions**:
- [x] `__design__/architecture.md` committed on `main` (2026-10-28)
- [x] Golden reference frozen: `agenticai4hpc26-artifact` tag `candler-m2a-golden`, `campaign/20260806-cont-train/expectations.jsonl` holds 10,320 rows (read it with `git show candler-m2a-golden:<path>`; the regrade worktree is gone)
- [x] Rikyu snapshot at mcp-python-ingestion tag `htn-creator-eval@0.4.2`: `.claude/skills/htn-creator-workspace/infrastructure/htn_creator_eval/catalogs/rikyu_mcp.json`
- [x] The HTN eval environment runs the snapshot capture: `uv run --directory .claude/skills/htn-creator-workspace/infrastructure …` in mcp-python-ingestion
- [x] Reminder `candler-schemas` (list "Candler") read: its notes are the original brief, written before Rust was chosen; where they differ, this leaf wins
**Success Gates**:
- ✅ `cargo build --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` pass
- ✅ `cargo test -p candler-schemas` passes, including tests that validate against the generated JSON Schema: the rikyu snapshot, a bio-opentrons snapshot, golden outcomes of the four statuses the corpus has (pass, fail, blocked with its artifact cause, not_measured), and synthetic `not_applicable` and text-valued outcomes
- ✅ The committed JSON Schema files match the structs: the check mode fails on a deliberate drift and passes after regeneration
- ✅ `cog check` passes on the branch history, and the `commit-msg` hook rejects a message without a scope
- ✅ HTN Phase 4.5 (`pro-htn-phase-4.5-skill`) can start: the catalog snapshot schema is on `main`
**References**: [R01 §Workspace, §Wire schema: outcome record, §Wire schema: tool-catalog snapshot](../../__design__/architecture.md) — crates, tooling, the two record shapes

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/wire-schemas/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`: leaves with named Deliverables and command-checkable Success Gates, covering:

- **Workspace.** The crates of R01 §Workspace except `candler-harness-opencode` (M4): `candler`, `candler-core`, `candler-schemas`, `candler-harness`, `candler-harness-claude-code`, all empty except `candler-schemas`. Rust 2024 edition, `rust-toolchain.toml`, rustfmt and clippy configuration, Grit `rust-conventions` from the first line (its reviewer agent is `rust-conventions:rust-conventions-reviewer`).
- **Commit and release tooling.** Install `cog`, `release-plz` and `cargo-nextest` (none is installed today) and say how in CONTRIBUTING. `cog.toml` mirrors CONTRIBUTING's types and scopes; a `commit-msg` hook. `release-plz` configured for per-crate versions, with the core line `v*` and the schemas line `schemas@*`, publishing disabled. CI workflow files that will run on the first push.
- **`candler-schemas`.** The outcome record with every field of R01 §Wire schema: outcome record (`role`, `instruments[]` with digest and dirty flag, `suite_entry_digest`, `context_digest`, `request_id`, value variants including text and named counts, `weight`, structured `cause`) and the tool-catalog snapshot in MCP `tools/list` shape. JSON Schema generated with `schemars`, committed beside the structs, with a check mode that fails when they drift. `serde_json` with `float_roundtrip`.
- **Real examples as test fixtures.** The rikyu snapshot (pre-condition 3); a bio-opentrons snapshot captured from `.claude/skills/htn-creator-workspace/servers/bio-opentrons/` with the HTN repo's `infrastructure/scripts/capture_catalog_snapshot.py`; golden outcomes converted by R01 §Golden corpus's mapping, taking `status`, `value`, `score`/`max_score` and `role` from `expectations.jsonl` and `blocked_by` from the run's `eval-N-grading.json` (the JSONL has no cause column); one per status the corpus has. The corpus has no `not_applicable` row and no text value, so those fixtures are synthetic.
- **Repo documents.** README's Status and CONTRIBUTING's Enforcement section updated in the same change.

Decide in the plan, and record in `__design__/architecture.md`: how the `value` union is tagged; the absent-versus-null rule; the serialisation of the run key (the outcome record needs it before M1's run manifest); the string formats of digests, hashes, `request_id` and evidence `ref`; the JSON Schema draft and file layout; the shape of the drift check (test, xtask or binary).

Adversarial verification is required for the schemas: they are the persisted contract every milestone and the HTN repo depend on. Attack golden rows that cannot be expressed, absent-versus-null ambiguity, float round-trip, and enum spellings that differ from today's records.
**Deliverables**: `__roadmap__/wire-schemas/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/wire-schemas/` (expected: PASS)
**Commit**: `rdm(schemas): plan the wire-schemas milestone`

## Step 2: Build and land

**Goal**: The milestone campaign is executed and lands on `main`.
**Implementation Logic**:
Execute the campaign on `feat/wire-schemas`, then land it on `main` with one `--no-ff` merge. The Success Gates above must pass on `main` after the merge. If a design decision changed, `__design__/architecture.md` changes in the same merge.
**Deliverables**: `candler-schemas` crate (`OutcomeRecord`, `CatalogSnapshot` and their member types), generated schema files, workspace manifest, `cog.toml`, `release-plz.toml`, CI workflow files, updated README and CONTRIBUTING
**Consistency Checks**: `cargo test -p candler-schemas` (expected: PASS)
**Commit**: `merge(schemas): land the outcome record and the catalog snapshot`

## Step 3: Close out

**Goal**: The milestone is recorded and the next one can start cold.
**Implementation Logic**:
Write a knowledge-transfer report with `writing-reports` under `__reports__/wire-schemas/`. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott: `candler-schemas` complete, and a log line on `pro-htn-phase-4.5-skill` saying its gate can read the snapshot schema. Apply only on his yes, through the scheduling-work skill's `sw_write.py`.
**Deliverables**: `__reports__/wire-schemas/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/` (expected: PASS)
**Commit**: `reports(schemas): close the wire-schemas milestone`
