# Seam review 2: findings and what was decided

A fresh Sonnet reviewer wrote none of this code. It read plan v2, the README purpose paragraph,
`seam_review_one_v0.md`, `git diff origin/main...origin/feat/integrity` at `cd968c54` with level 2 in focus (#64,
#66, #67, #68), and the level-2 leaf reports. Its report, kept as it returned, is
`integrity-tools/seam_review_two_reviewer.md`, outside the repository. It cost $2.0 against a $2.5 estimate and a $5
cap. Its brief put three questions first: does any run path seal without its snapshot or its execute bit; does each
input have one reader; is any guard fed state that no real producer makes.

## Findings

Nothing had to be fixed before P7. Every run Candler executes is sealed through `drive`, which stores the catalog
blob and writes each output's real execute bit. The blob is written before `seal_run` writes the manifest, so a
failure in between leaves an orphan blob, not a manifest that names a missing one.

| # | Finding | Decision |
|---|---|---|
| S1 | `RunWriter::blob` (`store/run.rs:170`) stores the freshly captured catalog without checking it against a digest. The manifest of a forced or pinned run keeps the stored cell's comparable (`runner/exec/mod.rs:114`), so it can name a catalog other than the one the agent saw | Recorded; follow-up below |
| S2 | `inspect` reads the catalog blob leniently and untyped (`commands/inspect.rs:218`, `:372`): an unreadable or invalid blob reads as "catalog not stored", and no guard covers the `server` and `tool` lines | Recorded: it is the typed-catalog follow-up already named after #64 |
| S3 | `resolve --json` and every stored job spec carry the full catalog text, in `preflight[].blobs` (`resolve/preflight.rs:59`, `jobs/model.rs:20`) | Recorded: the job spec is the only channel from the foreground to the supervisor, so removing the text needs a second channel, which is a design call |
| S4 | The execute bit is read in two places, `runner/exec/gather.rs:342` and `import/read.rs:253` | Recorded: two producers each read the mode of their own file; no input has two readers |
| S5 | `experiment.cell_value_invalid` and `experiment.setting_unsupported` point at the same setting in two shapes | Recorded: already recorded by seam review 1 |
| S6 | No page under `docs/contract/` documents the stored catalog or the op API's `InspectRun.catalog` | Recorded: P7's consumers look for the stored catalog from the documentation alone, and the pass records whether they found it |

S1 follow-up. A forced run whose manifest names a catalog its agent did not see is a provenance claim that can be
false, which is the subject of this milestone. First, settle whether it is reachable: a probe stores a cell, changes
the catalog the fake image prints, and runs again with `--force`. Then decide between refusing at seal (the store
checks the blob against the manifest's digest, as it checks outputs) and stamping the manifest with the digest of
the catalog the run saw. The meaning of `--force` is decided on that evidence.

The reviewer also recorded, with no action needed:

- Interrupted, imported and older runs all show "catalog not stored".
- The test producers `seal_workspace` and `seal_with_inputs` still build state by hand, and `sealed_run.rs:47` writes
  `executable = false`. Their cases cover state no runner path makes (the `.claude` file rule; declared inputs), and
  F3/F4's P7 gate is the producer-side observation for the first.
- Evaluators are not handed the catalog snapshot (`evaluation/context/assemble.rs:110`).
- `resolve` prints level 0 only; `run` and `resolve` still report a level failure differently (the F6 remainder,
  deferred to M7).

## Consistency

No code changed after the review. `cargo nextest run --workspace` at `5b850c56`, the last code change on the tip:
754 run, 754 passed, 31 skipped.
