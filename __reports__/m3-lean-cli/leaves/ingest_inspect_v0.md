# ingest_inspect, leaf report v0

The `import` verb becomes `ingest` and gains raw Claude Code trace directories; `inspect` replaces its scaffold stub. A Sonnet implementer did the work in two rounds. The coordinator rebased the branch onto 6d978bf, resolving one dispatch conflict in `main.rs` with `analyze`. Branch `m3/ingest_inspect` has 7 commits, head 056ee3f, merged as `merge(cli): land ingest_inspect`. There was no verifier, as planned: I1 and I4 machine-check the risks.

```
056ee3f test(cli): follow the ingest rename in the guards
a520a7c fix(importer): refuse a results folder given without rules
60ad44d test(cli): give each ingest and inspect guard its doc lines
0ac4829 feat(cli): add inspect to show a run with its evidence
a1eb421 feat(cli): ingest raw trace directories and results folders
460aad3 feat(importer): ingest raw claude code trace directories
9f2c547 refactor(cli): rename the import command file to ingest
```

`9f2c547` is a pure `git mv` (R098), so M4's in-body edits of the file rebase over it.

## What landed
- **`ingest <paths>…`:** each raw trace directory becomes one run per stream, through `import/raw.rs` and `ImportRecord::raw(format)` (P2 branch B). The record has no `base_commit`, no legacy block and no wall-clock time.
  - The cell is the harness name and version under a treatment named `unrecorded`.
  - Stored paths are relative (I17).
  - A second ingest of the same directory creates nothing, and a grown directory adds only the new runs.
  - A killed ingest, run again, leaves the store equal to one fresh ingest (I11).
  - Only `.jsonl` and `.jsonl.gz` files are candidates. Symbolic links are skipped and never followed.
  - The output reports `runs`, `created`, `existing` and `skipped` with reasons.
- **Results folders still need `--rules`.** A path with an `outputs/` directory under it, the legacy layout, is refused without `--rules` as `import.rules_required` (side `config`). Without this check the folder's own streams would have been ingested as ungraded raw runs.
- **`import`** stays a hidden alias of `ingest`, with the same output, so the commit is `feat(cli)` with no `!`.
- **`inspect <run dir | key prefix | job id>`** derives the run or job and shows its outcomes, evidence, provenance and integrity. It writes nothing, and an ambiguous key prefix is refused. A job with no `lock` file shows as `starting`, because probing would create the lock.
- **`RawIngestError`** has 7 variants. Its codes `import.no_stream`, `import.unreadable_path` and `import.rules_required` are `refusal_late`'s to register. The enum is on R1's pending list and in `refusal_scope.tsv`.
- **Recogniser:** core takes the stream recogniser from its caller (`dyn Fn(&[u8]) -> Option<Recognised>`), because core does not depend on the Claude Code harness crate. The binary supplies the Claude Code one.

## Rulings (coordinator, 2026-11-12)
- A results folder given without `--rules` is refused (R10 §3, "a results folder still needs `--rules`").
- The marker is `feat(cli)` with no `!`, because the alias keeps `import` working.
- Two authorized edits outside OWNED, both forced by the rename:
  - `guard_cli_surface.rs`: `PENDING_CWD` changes `import` to `ingest`, and `import --rules` leaves `PENDING_STAMPS`;
  - `guard_refusal_codes.rs`: one pending line for `RawIngestError`.

## Spec defects
- Two pre-condition commands fail as written. `git grep 'pub fn raw'` misses `pub const fn raw`, and the `[ingest]` table does not exist, since the codes are `import.*`.
- The spec's `find_streams(path)` cannot call `detect::init_version` from core, so the signatures take a recogniser.
- The manifest `format` is the reader's own id, `claude-code-stream-json@0.2.0`, not `claude-code-stream@<version>`.
- I2's mini-corpus counts are the hand literals of `guard_cli.rs` (6 runs, 136 records). No oracle file holds them.
- **For `refusal_late`'s verifier:** the results-folder check matches an `outputs/` directory anywhere under the path. Check that a raw trace directory holding an unrelated `outputs/` directory is not refused by mistake.

## Bites (each seen red, then reverted)
- **I1:** an absolute path stored; a second ingest adding runs; the results-folder layout check removed.
- **I2:** the rename dropping the `--rules` routing.
- **I3:** `inspect` needing a flag; a write during `inspect`; evidence dropped; an ambiguous prefix picking the first match.
- **I4:** the seal written before the trace; the replicate number not taken from the store on resume; `Existing::Refuse` in place of replacement.

## Gates
All PASS on 056ee3f, rebased, run by the coordinator:
- fmt, workspace clippy with `-D warnings`, workspace nextest (561 passed, 21 skipped), `cargo doc` with `-D warnings`, and the Linux clippy;
- I1 (9), I2 (2), I3 (5), I4 (3), R1 (2), the A2 stratum guards (10);
- GOLDEN: `guard_golden` (2), `guard_oracle_ac` on the merged tip, and `analyze`'s A4 (2);
- `cog check`.

On the merged tip: G-text, G-digest, and G-codes against 8aad2c9 (1037 added, 0 deleted). `guard_golden_parity` was not run: it needs `CANDLER_HTN_PLUGIN`.

## Production
| Round | Time | Tokens |
|---|---|---|
| Implementer, round 1 | 32 min | 371k |
| Implementer, round 2 (rebase, refusal, guard lists) | ~10 min | 390k |

Token counts are the harness's per-call figures. Peak target dir was 3.8 GiB.

## Follow-up merge: ingest_fix
The same implementer fixed the defects this leaf left, on branch `m3/ingest_fix`, in two rounds (about 20 and 8 minutes). It was merged as `merge(cli): land ingest_fix` (30a64fb) after `refusal_late`.
- **A results folder is recognised only by its root markers.** `PROVENANCE.json` and `expectations.jsonl` must sit at the path given. A raw trace directory that holds an unrelated `outputs/` directory is now ingested, not refused. Bite: the recursive `outputs/` check restored.
- **Codes kept:**
  - `diff` and `ingest` no longer flatten the coded rules and equivalence refusals;
  - a rules file that does not parse is `config.import_rules_invalid` (side `config`), with the file named. TOML errors carry a span, not a pointer, so the record has no `path`;
  - the text form is unchanged.
- **Guards:**
  - `i1_a_rules_file_that_does_not_parse_is_refused_with_its_code`;
  - the `outputs/` rows in `guard_ingest.rs`;
  - `guard_cli_diff_refusal.rs`.

  `refusal_late`'s ingest fixture gained the two markers.
- **Gates from eed99b7:**
  - workspace nextest, 574 passed and 21 skipped;
  - fmt, clippy, doc and LINUX;
  - I1–I4, R1 and R2;
  - G-codes (5 lines added) and G-text;
  - `guard_golden` and `guard_oracle_ac`;
  - `cog check`.
