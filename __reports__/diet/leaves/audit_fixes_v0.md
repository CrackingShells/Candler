# audit_fixes: leaf report (v0)

## Plan

| Claim | Fix | Owned files | Estimate |
|:--|:--|:--|:--|
| B1 fresh evaluation, OpenCode: runs store no output files | `OpenCodeDriver::collect` also returns `CollectSpec::file("**/*")` and the `!` exclusions of the harness's own workspace directory (`.opencode`, where its skills are mounted) | `crates/candler-harness-opencode/src/driver.rs`, `crates/candler-harness-opencode/tests/it/guard_opencode_driver.rs`, `crates/candler-harness-opencode/tests/it/conformance.rs`, `crates/candler-core/tests/it/guard_build_context.rs` | 25 calls |
| B2 tool-catalog snapshot not stored as a blob | read the cause; fix only if small and at one owner | none yet; listed in Next if not fixed | 10 calls, only if B1 is ready |

Out of scope: D1 to D5 and the silent_acceptance follow-ups.

## First reads

`__reports__/diet/01-findings_e2e_audit_v0.md` (rows "Evaluation, fresh" and "Per-run tool-catalog snapshots", B1 and B2), `crates/candler-harness-claude-code/src/driver.rs` (`collect`, `discovery::exclusion_patterns`), `crates/candler-harness-opencode/src/driver.rs` (`collect`, `skills`), `crates/candler-core/src/runner/exec/gather.rs` (`collect_files`).

## Bites

**B1 guard**: `candler-core` test `guard_build_context::opencode_collect_stores_the_files_the_agent_wrote` takes what `OpenCodeDriver::collect` returns through the core's own `collect_files` over a workspace holding `outputs/answer.txt`, `notes.md` and `.opencode/skills/s/SKILL.md`.
- Red on the tip: `left: [] right: ["notes.md", "outputs/answer.txt"]`.
- Green after the fix. The harness skills directory `.opencode/` is left out because the core mounts the treatment's skills there (`OpenCodeDriver::skills`), as the Claude Code driver leaves out `.claude/`.
- Two existing OpenCode expectations that pinned "collect is exactly the export" now pin the file specs plus the export (`tests/it/conformance.rs`, `tests/it/guard_opencode_driver.rs`).

**B1 rerun on RiVault** (one cell: `RiVault/Instruction-Small`, hint "Follow the audit-on skill.", 1 replicate; the audit experiment with each factor cut to one level and `replicates = 1`, plug-ins staged as `commands.sh stage` does, into a scratch store, not the audit store):
- `candler run experiment.toml`: job `01a209f9-2be9-7202-9d9b-882d7b79a1b0`, ended (completed), 1/1.
- `tree.json` now: `{"schema":"run-tree/1","entries":[{"kind":"file","path":"outputs/answer.txt","sha256":"sha256:f6ebe50df69d...","executable":false}]}`, and the blob `f6ebe50df69d...` is in `blobs/` (the audit store had `entries: []` and no answer blob).
- `candler evaluate suite_file.toml` with the audit's unchanged plug-in: `not_measured`, evidence `kind raised FileNotFoundError ... workspace/outputs/answer.txt`. The artifact is now listed, so the old "not among the artifacts" fail is gone; the plug-in reads `context.root / path`, but the core copies artifacts under `<root>/outputs/<path>`, so the file is at `<root>/outputs/outputs/answer.txt`. That is a bug of the audit plug-in (`__reports__/diet/audit/plugins/audit_file/audit_file.py`), not of the product; it never read a real artifact before.
- With the plug-in reading `context.root / "outputs" / path` (a scratch copy, version 0.1.1, not committed) the same `evaluate` gives `{"counts":{"pass":1}}` and the outcome record:
  `{"run":{"cell":"sha256:72f47864...","replicate":1,"attempt":1},"expectation_id":"F1","role":"gate","evaluator":"audit-file@0.1.1","status":"pass","evidence":[{"note":"outputs/answer.txt holds 'stamp-7f3a'"}], "context_digest":"sha256:e5b97ca5..."}`
- The scratch store was removed after this record was copied here; image `d4388be5010d` (the env image the run built) was removed by id. Cost: RiVault, no cost.

## Verifier

Not run: the 55-call cap was kept for the fix, the rerun and the gates; the verifier is the caller's step.

## Follow-ups

- **B2 (not fixed; cause read).** `CatalogStep::run` (`crates/candler-core/src/resolve/steps/catalog.rs`) calls `provider.capture`, digests the `Catalog` with `catalog_digest` and records only `DigestRef(catalog, digest)` under `snapshots`; the `Catalog` is then dropped, and no code puts it in the store (the module doc says the catalog "belongs to the run evidence", but the run evidence writes nothing for it). A fix needs a store handle in the step (or the provider, which already has `store_root`) and a decision that is not small: `Store::put_blob` digests the raw bytes, while `catalog_digest` is `Catalog::content_digest`, so the stored bytes must be the exact canonical form that digest covers, or the manifest digest and the blob name will differ. That choice, and whether `resolve` (which also runs the step) should write to the store, belongs to a design note, not to this piece.
- The audit plug-in `audit_file.py` reads `context.root / path`; it should read `context.root / "outputs" / path` (or the artifact's absolute path). The audit store's 24 `fail` outcomes for `audit-file` are the product bug fixed here; a rerun of the full audit would also need this plug-in line.

## Gates

`cargo fmt --check`, clippy `-D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc`: clean. `cargo nextest run --workspace`: 740 passed, 32 skipped. DIGEST: 21 passed. GOLDEN: 10 passed. No vector edited.
