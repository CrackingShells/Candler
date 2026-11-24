# Snapshot out (P12): leaf report

Base `7d497545` (`origin/feat/integrity`). Tip: see the commit table. Branch `leaf/l3-snapshot-out`, build slot `l3-2`.

## Commits

| Commit | Subject |
|---|---|
| `e8b07a32` | `test(api): read a run's installed MCP config back through inspect` |
| `4988b245` | `feat(schemas)!: drop the catalog snapshot and show MCP in inspect` |
| `cc7f12c8` | `refactor(runner): remove the constant catalog producer, show MCP config` (the roadmap's line is 88 characters; the commit hook allows 72) |

The schemas commit touches only `crates/candler-schemas/`, as CI's Commit messages job requires. It is a major bump on the schema line (a record valid before is invalid: `catalog-snapshot.schema.json` leaves, `inspect-run` loses `catalog` and requires `mcp`), so it carries `!` and a `BREAKING CHANGE:` footer (CONTRIBUTING §Schema line).

**The workspace does not build at the schemas commit alone.** `candler-core` and `candler` still name `CatalogSnapshot`, `InspectRun.catalog` and `CatalogError` there, and `candler-schemas`'s own `guard_refusal` reads `docs/contract/codes.toml` and the oracle vectors, which live outside the crate and change in the next commit. The step-2 commit restores a green workspace. The step-1 guard test fails by design on its own commit.

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| Guard on `candler run` -> `inspect`, one declared server, JSON and text | PASS | `inspect_shows_the_mcp_server_a_run_was_launched_with` launches `candler run --set mcp.servers.fs.command=mcp-fs --set 'mcp.servers.fs.args=["data"]'` through the fake docker, reads the sealed run back, and finds `mcp.servers.fs.command = "mcp-fs"` and `mcp.servers.fs.args = ["data"]` in `record["mcp"]` and the same two lines in the text form. On the base commit (`e8b07a32`): FAIL, `the record shows the mcp values: {"catalog":{"provider":"candler-image@1","schema":"tool-catalog-snapshot/1","servers":[]},...}`. |
| `CATALOG_PROGRAM`, `CatalogCommand`, `CommandCatalogProvider`, `CatalogStep`, the fake docker's constant, the provider seam | PASS | All deleted: `runner/evidence/catalog.rs`, `resolve/steps/catalog.rs`, the constant in `commands/preflight.rs`, the `printf` branch of the fake `docker run`, `M2bServices.catalog`, `catalog_spec` and its two control-kind rows. `CatalogProvider` had one implementor besides test fakes, so the seam went with it. The plumbing that only carried the catalog text to the seal went too: `StepReport.blobs`/`blob()`, `CellRun.blobs`/`with_blobs()`, `RunWriter::blob`. |
| `comparable` carries no catalog snapshot; new digest version vectors; old vectors verify | DEVIATION | `comparable.snapshots` is still a field of `Comparable`, optional: a cell that lists none leaves the key out of its record and of its hash, so every cell Candler makes now hashes under the new digest. The field stays because it is the one place the digest of a manifest that lists snapshots (the 240 golden runs, imported HTN runs) is read from: dropping it would make those manifests unreadable (`deny_unknown_fields`) and their digests unverifiable. New vectors: `invented-1` (regenerated, no `snapshots`) and `invented-4` (new, no `snapshots`) in `cell_digest.toml`; the three corpus cells and `invented-2`/`-3` keep `snapshots` and their old digests (`digests_equal_the_oracle`, 7 vectors, PASS). The generator and the vector header say why the old vectors stay. The lead decides whether to take the field out later, after the maintainer's stores are re-imported. |
| `inspect` callers gate (htn-creator-eval@0.5.3) | PASS | Supplied by the lead, not re-run: all 279 `.py`/`.sh` files under the HTN and candler paths were searched for `candler ... inspect`, `"inspect"`, `["catalog"]` and `.get("catalog")`. The one hit, `infrastructure/htn_creator_eval/cli.py:1053`, forwards to the HTN's own trace profiler, not to `candler inspect`. `candler-plugin/` has no `inspect` or `catalog` reference. `_catalog_snapshot.py` reads the HTN's own committed `catalogs/*.json`. No reader of `InspectRun.catalog`, so it was dropped. |
| `InspectRun.catalog` and `catalog_of` gone; an old sealed run still reads | PASS | Both deleted. `inspect` reads the manifest only; the stored blob is left in the store and not shown, and no shim reads it. The old manifests read through `RunManifest` (unchanged `snapshots` reader), covered by the golden and import guards in the workspace run. |
| `tool-catalog-snapshot/1` gone | PASS | `candler-schemas/src/catalog.rs`, `schema/catalog-snapshot.schema.json`, `tests/it/catalog_fixtures.rs`, `catalog_properties.rs`, the `tests/fixtures/{spec,upstream}` fixtures, the catalog test in `guard_wire_shapes.rs` and `guard_cell_digest.rs`, and `catalog_content_digest.toml` with its generator section. The plan lists `catalog_content_digest.toml` among vectors that stay; the leaf says vectors only `/1` uses go. Only the deleted digest test read it, so it went. |
| README, AGENTS.md, live candler-v1 M6c Goal | PASS | `per-run tool census` in `README.md:7`, `AGENTS.md:8`, and `__roadmap__/candler-v1/.../integrity/README.md:7`; `AGENTS.md` names "a tool census" among the stored states. No page under `docs/` quoted the phrase. Done leaves and reports keep their words. |
| Refusal codes only the removed code emitted | PASS | `catalog.*` (6 schemas codes), `runner.catalog_command_failed`, `runner.catalog_timeout`, `runner.catalog_unreadable`, and the three `[internal]` rows of the core `CatalogError`; `docs/contract/codes.toml` and `index.md` regenerated (`CANDLER_BLESS_CONTRACT=1`); the `catalog` surface left `SURFACES`; the two `CatalogError` rows left `__roadmap__/m3-lean-cli/refusal_scope.tsv`, which `guard_refusal_codes` reads as its oracle. |
| `cargo fmt --check`, clippy `-D warnings`, `cargo doc -D warnings` | PASS | exit 0 for each on the tip. |
| `cargo nextest run --workspace` | PASS | `Summary [161.761s] 729 tests run: 729 passed (6 slow), 31 skipped`. The 31 skipped are the Docker-gated `container-e2e` tests, which the lead reads from the PR's CI. The first run on the tip failed five (the guard, three pinned-digest tests, and the refusal-scope oracle); each is fixed in the step-2 commit (see Pins retaken). |
| `container-e2e` green on the PR | BLOCKED | Read from the PR's CI by the lead. |
| Step 3 scratch bite | PASS | On a scratch branch from the step-2 tip, `inspect.rs` shows `mcp: BTreeMap::new()`; the guard FAILS: `assertion left == right failed: the declared command is shown: {... "outcomes":[] ...}` at `guard_inspect_mcp_config.rs:57`. Scratch worktree, branch and target dir removed. The fresh verifier is the lead's. |

## Pins retaken

Four tests pinned digests of cells built by the real planner (`guard_egress_setting`: `LISTED_DIGEST`, `BASE_DIGEST`; `guard_run_plan::t3_one_level_plans_the_old_bytes`: both cells, the experiment digest, and the replicate order, which the planner derives from the digests). Those cells hashed an empty `snapshots` list before; they hash without it now, so the pins moved. They were re-taken from the producer's output, which is the new digest version's meaning, not a drift.

## What it leaves

**How it is developed.** The guard was run on the base first and failed for the stated reason after two false starts of the test itself (an absolute path in `--set` that the resolver refuses, and a fake docker with no top-level `inspect`, which the start probe of a declared stdio server calls). No Docker, no live model, one build slot (`l3-2`), no scratch worktree kept. The Python generator reran under `uv run --with rfc8785` and changed only `cell_digest.toml`.

**What it leaves in the code.** One producer of the MCP record: the resolved `mcp.*` values, written by resolve and read by `inspect` from `comparable.values`. The constant catalog, its capture, its resolve step, its seam, the blob plumbing that carried its text, its schema, its fixtures, its refusal codes and its inspect surface are deleted in the same change. The one thing kept without a producer is the optional `comparable.snapshots` read of manifests that list it (gate row above); the importer still writes it for imported legacy runs, which is the field's only live writer.

**How it is consumed.** The experimenter's `inspect` shows `mcp.*` by path, in the JSON record (`mcp`) and as `path<TAB>value` lines in text. A run with no MCP server shows an empty `mcp`. The record no longer shows a tool catalog Candler did not observe. `inspect-run.schema.json` changes (`catalog` out, `mcp` required), which is the compatibility item below.

## Not covered

- **HTN-side follow-up for the lead to hand on:** htn-creator-eval@0.5.3 keeps grading, but its one test that validates against Candler's `catalog-snapshot.schema.json` now skips. The schema file has to move into the HTN repository (the `/1` marker stays the HTN's own). Nothing in the HTN repo was touched.
- Step 3's fresh verifier, and the PR's `container-e2e`: the lead's.
- `__design__/architecture.md` still describes the tool-catalog snapshot and catalog providers (§Wire schema: tool-catalog snapshot and several mentions). The leaf lists no design deliverable, so it was left; it is stale against the code.
- The optional `comparable.snapshots` field (see the deviation) and the importer's write of it.

## Compatibility for consumers

- `inspect-run/1` consumers: `catalog` is gone and `mcp` (object, path to value) is required.
- `run-manifest/1`: a manifest without `comparable.snapshots` is now valid, and its cell digest differs from the digest the same cell had with `"snapshots": []`. Run keys of new runs therefore differ from old ones for the same experiment.
- The schema line takes a major bump (minor while `0.y.z`).
