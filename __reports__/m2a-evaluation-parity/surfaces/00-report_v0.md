# surfaces: leaf report (v0)

This is a condensed version of the implementer's two reports: the first round, and the fix round with the rules-digest addendum. The rulings are in [l1/00-rulings_v0.md](../l1/00-rulings_v0.md) §surfaces. The leaf is merged on report, with no verifier. It lands as stage one and stays in progress until CLI2 stage two (`evaluate` with the synthetic plug-in) runs after `engine`.

## Report
### 1. Commits
Steps 4 and 5 go first on the branch, so each commit passes `cargo check --all-targets`.
- e85ab9c feat(schemas): record the import rules digest in the run manifest (Step 4; touches only `crates/candler-schemas/`)
- 5eabce5 feat(importer): record the rules digest and refuse a changed one (Step 5)
- 9d49949 feat(cli): share one exit mapping and wire import and evaluate (Step 1, with the `--rules` wiring)
- 1fb0d5b feat(cli): wire candler show and diff to the core (Step 2)
- 97491d5 test(cli): guard the json contract and the end-to-end path (Step 3)

### 2. Gates (implementer)
| Gate | Result |
|---|---|
| SPAWN + workspace nextest | PASS. 293 passed and 4 skipped, with the M1 import guards and `guard_layering_i1` included |
| STD | PASS: fmt, clippy `-D warnings`, doc `-D warnings` |
| Linux clippy | PASS |
| DRIFT | PASS. Blessing changes nothing after the commit, and the schema diff adds one optional `ImportRecord.rules` (`Digest`) |
| OWNED | PASS on the regex widened for Steps 4 and 5 |
| SUBJ | PASS, with every subject at 65 characters or fewer |
| SCAFFOLD | PASS: 3 tests and 3 `Guards:` lines |
| NEUT | PASS. `crates/candler/src` names no legacy domain; the rules arrive as a data file |

### 3. Guards
- **CLI1** pins the `diff --format json` key sets. `diff` exits 1 on a planted change, 0 on none, and 2 for a missing version. Bite: renaming `compared`.
- **CLI2, stage one**, test `import_show_and_diff_over_the_mini_corpus_match_the_hand_counts`:
  - `import --rules tests/fixtures/cli/mini-corpus-rules.toml` prints `{"runs":6,"records":136}`.
  - `show` gives pass 105, fail 7, blocked 13, `not_measured` 10 and `not_applicable` 1 under `htn-creator-eval@0.4.2`.
  - `diff` of that version against itself gives `compared` 136 and `changed` 0, with exit 0. Against an absent version it exits 2.
  - Every manifest carries `import.rules = sha256:f26fef6c…cd87e`, a hand literal taken with `shasum -a 256` on the fixture.
  - A re-import with the same file exits 0. A re-import with a one-byte-different file exits 2 and names `import.rules`.
  - The oracle is `jq -r '.expectations[].status'` over the mini-corpus's 42 `eval-N-grading.json` files. Rows: run-6 43, run-12 43, run-17 43, synthetic run-2 5, synthetic run-3 2.
  - Bites: the importer skips writing `rules`; the `Conflict` stops naming `import.rules`; `diff`'s `compared == 0` check is disabled. Each fails the test.
- **CLI3** is one exit mapping shared with M2b. Its bites are unchanged from the first round.

### 4. Choices where the leaf was silent (accepted)
- **`show` JSON shapes.** Counts are `{"counts": {"<evaluator>": {"<status>": n}}}`. Rates are `{"cell": {"<hex>": {"gate": [n,d], "quality": [n,d]}}}`, and the TOML rates are the core's `rates_toml`.
- **`evaluate` defaults and output.** Concurrency defaults to 1. It prints `{request_ids, counts, outcome_files, failed_requests}`. Ctrl-C or a failed request gives exit 2.
- **The error and exit types.** `CliError` (Unavailable, Refused, Failed, Runtime) and `Exit` are both `#[non_exhaustive]`.

### 5. Findings
1. The reader factory falls back to `process::exit(2)`, which is unreachable after `check_reader`. KT follow-up: make `ReaderFactory` fallible.
2. `CliError::Unavailable` is unused; it is reconciled with M2b at the final rebase.
3. `import::digest_of` is re-exported from `import/mod.rs`.
4. `diff` does not report I4 violations. This is accepted and goes in the KT.

## Landing (coordinator, m2a/surfaces on 18ba3e3)

| Gate | Result |
|---|---|
| OWNED | PASS: nothing outside the widened regex |
| SUBJ | `cog check`: no errored commits; all subjects 72 characters or fewer |
| SPAWN | `guard_signal` 4 passed |
| STD | fmt, clippy, doc clean; nextest 293 passed, 4 skipped |
| Linux clippy | clean |
| DRIFT | `schema_drift` 2 passed; no change to the schema files on blessing |
| Schema diff | additive: optional `ImportRecord.rules` |
