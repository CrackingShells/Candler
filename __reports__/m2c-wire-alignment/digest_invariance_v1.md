# Digest invariance of M2c — Findings (v1)

Date: 2026-11-10

---
type: findings
topic: m2c-wire-alignment
date: 2026-11-10
version: v1
prior-version: v0 (R08, `__reports__/v1-review-code-gaps/digest_invariance_v0.md`)
key-metric: moved run keys / context digests / suite-entry digests over 10,320 golden + 136 mini records: 0; manifests differ on 246 of 246 runs in one JSON path, `import.rules`
decision-required: none (the coordinator amended the manifest gate to allow `import.rules` alone; met)
---

## Headline Result

Amended manifest gate (coordinator, milestone commit 7a3eca0): the manifests may differ in `import.rules` alone; the whole `diff` must show only that field; the `diff` with `del(.import.rules)` must be empty. **PASS**: 240 of 240 and 6 of 6 manifest lines differ, and each differs in `import.rules` only (path check below); the masked diff is empty on both corpora. The original wording (manifests equal) was unsatisfiable, as the first run of this leaf found; that run's finding follows unchanged.

| Field | Value |
|:---|:---|
| metric | run keys, `(run key, context_digest)` pairs, manifests: `b017d31` build vs milestone-branch build |
| run keys, context digests | 0 differences (both corpora) |
| manifests, whole, `jq -S -c`, no field filtered | DIFFERENT on every run (240/240, 6/6) |
| the difference | exactly one field, `import.rules`, the sha256 of the rules file; the two rules files differ by one line, as the leaf requires |
| direction | the manifest gate as written is unsatisfiable; the digest claim itself holds |

The manifest `diff` is not empty, which is a finding and is reported here as one. With `import.rules` alone removed, the manifests are identical on all 246 runs (`import.sources[]` included, so the L0 stream/stderr sources are equal on both sides). `import.rules` records the digest of the rules file that drove the import. `b017d31` rejects `entry_params` (`unknown field entry_params`, measured), and the branch requires it, so no single rules file can serve both builds and the field must differ. The gate "manifests equal" needs a coordinator decision (below).

Builds (both `cargo build --release -p candler`, `RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0`, own target dirs):

| Side | Commit | `candler` binary sha256 |
|:---|:---|:---|
| main | `b017d31c5777fdd9ec46cb4da7d0a8ebcafa0c6e` | `f5f09184…eabafd` |
| branch | `bc1567cf0ebd12c000f3f82655c5e98980fcc558` | `e4b32a9a…f6e5b` |

Rules files (copies under `$TMPDIR`; the repo file was not edited):

| Side | File | sha256 |
|:---|:---|:---|
| main | `git show b017d31:crates/candler/tests/fixtures/cli/mini-corpus-rules.toml` (the file R08 used, same sha `f26fef6c…d87e`) | `f26fef6c53ee026c52c08ac9de11cc47263bd289f61b0c2bf9b12000f45cd87e` |
| branch | the same file plus `entry_params` | `b4387a085012bd32701ecd9f968b419d719e9f33ff97fcbd21fa44164cc00ccb` |

`diff rules-main.toml rules-branch.toml` prints `3a4 > entry_params = ["scale", "text", "tier"]`: one added line, the keys of `ENTRY_PARAMS` at `b017d31` (`["scale","text","tier"]`, `legacy.rs:77`). The rules file is one for both corpora (format `htn-results@1`), as in R08.

## Results Tables

### Counts (non-vacuity)

| Corpus | Runs | Outcome records | Import output, main | Import output, branch |
|:---|---:|---:|:---|:---|
| golden (read-only) | 240 | 10,320 | `{"records":10320,"runs":240}` | `{"records":10320,"runs":240}` |
| mini-corpus | 6 | 136 | `{"records":136,"runs":6}` | `{"records":136,"runs":6}` |

Lines per extraction, identical on both sides: keys 240/6, `index/runs.jsonl` 240/6, `(key, context_digest)` 10,320/136, `(key, suite_entry_digest)` 10,320/136, manifests 240/6.

### Comparisons (`diff main branch` of sorted files)

`R=<store>/<corpus>`, `K(r)` = `(.run.cell|sub("^sha256:";""))+"-r"+(.run.replicate|tostring)+"-a"+(.run.attempt|tostring)`.

| Comparison | Extraction command | golden `diff` | mini `diff` |
|:---|:---|:---|:---|
| run keys | `command ls -1 $R/runs \| sort` | empty | empty |
| run keys, index | `sort $R/index/runs.jsonl` | empty | empty |
| `(run key, context_digest)` | `find $R/outcomes -name '*.jsonl' -print0 \| xargs -0 cat \| jq -r '[K, .context_digest]\|@tsv' \| sort` | empty | empty |
| `(run key, suite_entry_digest)` (extra) | same, with `.suite_entry_digest` | empty | empty |
| manifests, whole (amended gate: differ in `import.rules` only) | `for k in $(command ls -1 $R/runs); do jq -S -c . $R/runs/$k/manifest.json; done \| sort` | 240 lines differ, in `import.rules` only | 6 lines differ, in `import.rules` only |
| manifests, `del(.import.rules)` (diagnostic, not the gate) | same with `jq -S -c 'del(.import.rules)' ` | empty (sha256 of sorted output `0bee91a5…` both sides) | empty (`c86e6897…` both sides) |

Where the whole-manifest diff lies, for the first mini run, `jq -c .import.rules`:

```
< "sha256:f26fef6c53ee026c52c08ac9de11cc47263bd289f61b0c2bf9b12000f45cd87e"
> "sha256:b4387a085012bd32701ecd9f968b419d719e9f33ff97fcbd21fa44164cc00ccb"
```

Every one of the 240 and 6 manifests carries the main-side rules digest on one side and the branch-side one on the other. Also equal on both sides: `import.sources[]` has 18,480 entries on golden (480 stream/stderr) and 163 on mini (12), so no L0 source differs.

### Path check: the differing JSON path is exactly `import.rules`

For every run key (240 golden, 6 mini; stronger than one pair) the two manifests were compared path by path: the union of `paths(scalars)` of both sides, keeping the paths whose value differs (or is absent on one side). Command, per run key `k` (`a`, `b` the main and branch `runs/$k/manifest.json`):
`jq -n -r --slurpfile a A --slurpfile b B '([$a[0]|paths(scalars)]+[$b[0]|paths(scalars)]|unique)|map(select(. as $q|($a[0]|getpath($q))!=($b[0]|getpath($q)))|map(tostring)|join("/"))|join(",")'`, then `sort | uniq -c`.

| Corpus | Runs | Differing paths (count of runs) |
|:---|---:|:---|
| golden | 240 | `import/rules` (240) |
| mini | 6 | `import/rules` (6) |

Control: on one mini pair with the branch side's `import.format` altered to `x`, the same command prints `import/format,import/rules`, so the check does fail on another field. These stores were re-imported for this check (same commands, same rules files, same shas; the main binary sha `f5f09184…` is unchanged; the branch binary was rebuilt at the report commit on top of `bc1567c` and has sha `1f0fed55…` instead of `e4b32a9a…` (source differs only by the report file; not investigated further), with identical import counts; the other comparisons were not re-run on it, their results above are from the first build) and removed again.

### The comparison can fail

- Each side is non-empty (counts above).
- One deliberately altered line (last hex digit of line 5 of the branch mini `(key, context_digest)` file changed) makes `diff` print 4 lines, the changed pair among them.
- The whole-manifest comparison itself did fail, which shows it filters nothing.

### Parity fixtures and GOLDEN

| Gate | Result |
|:---|:---|
| Amended manifest gate | PASS: whole diff shows `import.rules` only (path check); masked diff empty on both |
| G5a (`git diff --numstat b017d31` over the oracle and vector dirs, `$2!=0`) | PASS: 6 files listed, all with 0 deleted lines (additions only) |
| G5b (`field_paths.toml`) | PASS: one `-path` row (`request.treatment_source`); gains `isolation.build.prune`, `isolation.record_packages` |
| GOLDEN on the branch | PASS: `guard_golden` 2/2 (57.2 s), `guard_oracle_ac` 1/1 (0.3 s); `$GOLDEN` only read |

## Observations

| Signal | Expected | Observed | Interpretation |
|:---|:---|:---|:---|
| Run keys, cell digests | equal | 240/240, 6/6 equal | no cell digest moved |
| Context digests | equal | 10,320/10,320, 136/136 equal | none moved |
| `suite_entry_digest` | equal (the `entry_params` move) | 10,320/10,320, 136/136 equal | the `entry_params` move changes no suite entry digest |
| Manifest | equal | differs in `import.rules` only | rules-file digest, forced by the one-line rules change |
| Manifest other fields | equal | equal on 246 runs | `import.sources[]`, provenance, requests unchanged |

## Steering Questions

- `[resolved]` (the coordinator took option (b) in 7a3eca0.) The leaf's manifest gate said the manifests are equal with the branch's rules file adding `entry_params`; the manifest records that file's sha256, so the gate is unsatisfiable by construction. Options: (a) accept "equal except `import.rules`" with the masked diagnostic above as the gate's evidence; (b) amend the gate to name `import.rules` as the one allowed difference. I did not mask the field in the gate comparison.
- `[later]` The verifier's list (a manifest field outside the comparison, a live run's provenance leaking into an import) is covered here for imports only: nothing outside `import.rules` differs.

## Pointers

- Leaf: `__roadmap__/m2c-wire-alignment/consumers/closeout/revalidation/digest_invariance.md`; method and baseline: R08 (`__reports__/v1-review-code-gaps/digest_invariance_v0.md`)
- Scratch (removed after this report): `$TMPDIR/m2c-digest-main`, `$TMPDIR/m2c-digest-branch`, `$TMPDIR/ex-*`, `$TMPDIR/ctl-main-mini`, `$TMPDIR/m2c-digest-main-src`, target dirs `/private/tmp/claude-501/m2c-target-digest-{main,branch}`
