# refusal_late, leaf report v0

Codes the refusals of jobs, evaluation, import and ingest, including the enums `evaluate_body` and `ingest_inspect` added, and closes R1: every error type of the workspace is either coded or on the `[internal]` allowlist. A Sonnet implementer did the work in four rounds. The required Sonnet verifier, which also covers `refusal_core`, attacked twice: FAIL, then FAIL on minor findings, which round 4 fixed and the coordinator checked. The coordinator rebased the branch onto f068c7d with no conflict. Branch `m3/refusal_late` has 9 commits, head e93350b, merged as `merge(cli): land refusal_late` (88d0d66).

```
e93350b fix(cli): map job-open failures by kind and align wrapper answers
1271c31 fix(cli): print only a code with the side its registry row gives
f0a1b1b fix(cli): keep the wrapper's side for a value check and name the cause
beea6ea test(cli): guard every wrapper and close the scan holes in R1
953bb61 fix(cli): answer a wrapped refusal with the side of its cause
bab8eec test(cli): require every error type to be coded or internal
9a545bc feat(cli): code the refusals of import and ingest
b09f238 feat(cli): code the refusals of jobs and evaluation
1cd2847 docs(cli): register the codes of the late refusals
```

## What landed
- **Codes** for `jobs`, `evaluation`, `import` (with `RawIngestError`), `trace` and the manifest verdict, in a `refusal.rs` beside each module, with their rows in `codes.toml` `[jobs]`, `[evaluation]` and `[import]`.
- **R1 is closed.** Its pending list is empty. The scan keys types by file and name, reads every derive line (split derives and `cfg_attr` included), finds hand-written `impl … Error`, and skips `tests` and `benches` only at a crate's root.
- **The invariant.** Every printed record's `(code, side)` pair is one row of `codes.toml`, or the `candler.internal` fallback of `[internal]`. It is asserted over a 46-case wrapper table (`r1_every_wrapper_answers_with_a_registered_code_and_side`), for both the walk of causes (`find`) and each wrapper's direct `Refusal` impl, and on every R2 record.
- **The wrapper rule.** A wrapper that only carries another error answers with what the walk of causes finds from it. A `FormatError` under a document wrapper answers with the wrapper's own code and side, unless its cause blames Candler, which then answers with its own row. The document codes are:

  | code | side | section |
  |---|---|---|
  | `experiment.value_invalid` | experiment | `[manifest]` |
  | `outcome.value_invalid` | plugin | `[evaluation]` |
  | `experiment.setting_value_invalid` | experiment | `[experiment]` |
  | `resolve.value_invalid` | config | `[resolve]` |
  | `store.value_invalid` | environment | `[store]` |
  | `import.value_invalid` | environment | `[import]` |

  The first two wrap enums of the schemas crate, whose guard C2 accepts only its own variants under `[schemas]`, so their rows sit in the section of the document they name.
- **Caller mistakes get their own codes:**
  - `jobs.job_not_found` (side config) for a job directory that does not exist;
  - `jobs.job_not_a_directory` and `jobs.io_failed` (side environment) for the other failures to open one;
  - `evaluation.suite_unreadable` (side config).
- **Fixes in `refusal_core`'s modules,** found by the verifier through the binary:
  - the `Coded` path now goes through `find`;
  - `PlanError::Cell` holds its `ExecutionError` as its source, so `experiment.absolute_path` reaches the record. Its `#[error(` text changed from `{detail}` to `{source}`, which renders the same;
  - `MountError::BadPath` is coded `isolation.mount_path_unsupported`;
  - `plugin.environment_not_ready` has side environment.

## Rulings (coordinator, 2026-11-12)
- **The `(code, side) ∈ codes.toml` invariant** replaces my round-2 rule. That rule printed the cause's code with the wrapper's side, a pair that is no registry row. Round 3 corrected it.
- **OWNED was widened before round 1 (f068c7d)** to `refusal_core`'s `refusal.rs` files, `commands/refusal.rs` and `runner/plan.rs`'s `PlanError::Cell`.
- **Two registry rows were edited in place:** `plugin.environment_not_ready` and the `Body` reason. Both are new in this milestone and unreleased, so G-codes against 2e0c015 and 8aad2c9 shows no deletion. After a release this must not happen again.
- **Moved out of this leaf:**
  - **To `cli_surface` (Step 6):** coding the refusals a caller causes in the binary, and clap's usage errors as `cli.usage_invalid` (F4).
  - **Follow-ups:**
    - F6: a suite entry's params pointer is relative to `params`;
    - F7: no `file` on `CliError::Failed` records;
    - F8: the generic `experiment.document_invalid` hides the specific codes. Fixing it needs a schemas release.

## For `docs_refusals`
Each document code collapses the nine author-facing `format.*` reasons into one code. The specific reason stays in `message`.

## Known limits
- A `thiserror` derive produced by a `macro_rules!` evades R1's scan.
- The schemas crate's own wrapper enums keep their direct `Refusal` impls. The core walk answers for them, and R1 checks it. Aligning them needs a schemas release, with F8.

## Verifier
**Round 1, FAIL**, at 5c6aac8 (25 min, 303k). 20 variants were sampled through the binary. Findings:
- F1: a format wrapper printed an author error as `candler`;
- F2: the `Coded` path skipped `find`;
- F3: `PlanError::Cell` flattened `experiment.absolute_path`;
- F5: the `*_failed` names misnamed caller mistakes;
- F9: the wrapper rule was unguarded;
- F10: R1 had three evasions;
- F13: a side was wrong.

**Round 2, FAIL (minor)**, at 9d9682e (14 min, 353k). F1–F3 and F5 were fixed, and no unregistered pair could be printed. Five low findings remained:
- N1: `jobs.job_not_found` on a permission fault;
- N2: Candler-blaming causes swallowed by the document wrappers;
- N3: the direct impl gave a pair that is no row;
- N4: R1 skipped every `tests` directory;
- N5: `watch` on a regular file printed `candler.internal`.

Round 4 fixed all five, each with a guard row and a bite. The coordinator checked the diff instead of a third verifier pass.

## Bites (each seen red, then reverted)
- **The invariant:**
  - the round-2 rule restored;
  - the Candler exception removed;
  - one wrapper's direct `side()` left as it was.
- **R2:**
  - the `Coded` path without `find`;
  - `jobs.job_not_found` given side environment;
  - the operation-text match for job-open failures restored.
- **R1 plants:**
  - a plain new enum;
  - a type named like a coded one;
  - a split derive;
  - a hand-written `impl Error`;
  - a `cfg_attr` derive;
  - a generic `impl<T> Error`;
  - a name shared with an `[internal]` enum;
  - an enum under `src/x/tests/`.

## Gates
Run by the implementer on e93350b's content: fmt, workspace clippy with `-D warnings`, workspace nextest (570 passed, 21 skipped), `cargo doc` with `-D warnings`, the Linux clippy, R1 and R2, G-codes against 2e0c015 and 8aad2c9 (0 deletions), and `cog check`. The coordinator ran the tip gates on 88d0d66.

## Build note
sccache served one verifier run a test binary built for another path, which made `guard_layering_i1` fail with "not a git repository". The gates were re-run after `cargo clean -p candler-core -p candler`. The coordinator's tip scripts now run without sccache.

## Production
| Round | Time | Tokens |
|---|---|---|
| Implementer, round 1 | 55 min | 309k |
| Implementer, wrapper sides | ~15 min | 29k |
| Verifier, FAIL | 25 min | 303k |
| Implementer, round 2 | ~45 min | — |
| Implementer, round 3 | ~11 min | — |
| Verifier, FAIL (minor) | 14 min | 353k |
| Implementer, round 4 | 13 min | 81k |

Rounds 2 and 3 have no separate counts: the harness reports only the resumed agent's running total, 435k after round 3. Token counts are the harness's figures.
