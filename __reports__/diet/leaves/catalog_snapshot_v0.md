# Catalog snapshot leaf, v0

A sealed run stores the canonical bytes of the tool catalog it ran with; `candler inspect` and the
op API's inspect result show it.

## What changed

- `CatalogSnapshot::content_text` (schemas) is the RFC 8785 text without `captured_at`;
  `content_digest` hashes it, so no digest changes.
- `CatalogStep` puts that text in its `StepReport.blobs`, a field outside the cell's values. `resolve`
  writes nothing to the store.
- `cells_of` copies the blobs of the cell's level into `CellRun.blobs`; `drive` stores each with
  `RunWriter::blob` in the seal task, just before `seal_run` stores the outputs.
- `InspectRun.catalog` (wire, regenerated schema) carries the stored catalog as JSON; `inspect` reads
  it through the manifest's `catalog` snapshot digest. A run with no stored blob has no `catalog` and
  its text form says `catalog not stored`.

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| Guard fails on the base | PASS | scratch worktree at `c4a48cc1` plus the guard: `FAIL guard_catalog_snapshot::a_sealed_run_stores_the_catalog_it_ran_with_and_inspect_shows_it`, `the catalog digest resolves to a blob: Os { code: 2, kind: NotFound ...}` |
| Guard passes on the tip | PASS | `cargo nextest run -p candler -E 'test(/a_sealed_run_stores_the_catalog/)'`: 1 passed |
| Step 2 bite | PASS | scratch worktree on the tip with `writer.blob(...)` replaced by `let _ = text;`: guard FAIL with the same blob-not-found panic |
| `resolve` stays a pure read | PASS | the step only returns the text in its report; no store handle reaches `resolve` |
| Snapshot stored by the sealing task; run from `World` | PASS | guard uses `cli_run::world()`, `launch`, `finish` |
| No digest or golden vector edited | PASS | workspace nextest 751 passed, 31 skipped; no golden file in the diff |
| fmt, clippy `-D warnings`, nextest, doc `-D warnings` | PASS | all four clean |
| container-e2e | not run locally | runs on the PR |
| Fresh Sonnet verifier | not mine | the lead runs it |

## Departures from the brief

- The op API's inspect result is `InspectRun`, whose `catalog` is a JSON value, not a `CatalogSnapshot`:
  the stored bytes omit `captured_at`, which the snapshot reader requires.
- `seal_run_observed` already takes seven arguments (clippy limit), so the blob write is in the seal
  task before `seal_run`, not a `SealStep`; the observer does not see it.
- The schemas commit adds a field to `InspectRun`, so the workspace builds only with the next commit;
  `candler-schemas` builds and passes alone.
- The fake docker prints a catalog with no servers, so the guard shows the provider, not tool names.
