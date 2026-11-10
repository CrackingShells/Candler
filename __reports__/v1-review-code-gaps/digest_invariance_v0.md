# Digest invariance of the two L0 fixes — Findings (v0)

Date: 2026-11-10

---
type: findings
topic: v1-review-code-gaps
date: 2026-11-10
version: v0
prior-version: none
key-metric: moved digests over 10,320 golden + 136 mini records: 0 (prior: N/A, delta: N/A)
decision-required: none
---

## Headline Result

| Field | Value |
|:---|:---|
| metric | run keys, outcome context digests, manifests minus stream/stderr sources that differ between `main` and the branch |
| value | 0 |
| unit | differences (all four diffs empty, both corpora) |
| prior | N/A (first run) |
| direction | new |

Builds: `main` is `d7a29a1` (`origin/main`; no `crates/`, `python/` or Cargo file differs from `025754d`, the brief's sha, so the code is the same). Branch is `e5d0887` (`fix/v1-review-code-gaps` after L0). Both are `cargo build --release -p candler`. Both imports used the rules file `crates/candler/tests/fixtures/cli/mini-corpus-rules.toml` (sha256 `f26fef6c…d87f`); the golden and mini corpora share the `htn-results@1` format, and the import accepted the golden corpus with it on both builds.

## Results Tables

### Counts compared

| Corpus | Runs | Cells (distinct run-key prefixes) | Outcome records | Import output, `main` | Import output, branch |
|:---|---:|---:|---:|:---|:---|
| golden (`$GOLDEN`) | 240 | 12 | 10,320 | `{"records":10320,"runs":240}` | `{"records":10320,"runs":240}` |
| mini-corpus | 6 | 3 | 136 | `{"records":136,"runs":6}` | `{"records":136,"runs":6}` |

### Comparisons (each is `diff` of two sorted extractions; all print nothing)

| Comparison | Lines per side, golden | Lines per side, mini | `diff` output |
|:---|---:|---:|:---|
| run keys: `ls runs \| sort` | 240 | 6 | empty |
| run keys: `index/runs.jsonl`, sorted | 240 | 6 | empty |
| `(run key, context_digest)` per outcome record: `jq -r '[.run.cell+"-r"+replicate+"-a"+attempt, .context_digest]\|@tsv'` over `outcomes/**/*.jsonl`, sorted | 10,320 | 136 | empty |
| manifests, `jq -S -c` with `import.sources[]` entries ending `agent_stream.jsonl.gz` or `agent_stderr.txt` dropped, one line per run, sorted | 240 | 6 | empty |

Stronger than the gates ask, also empty on both corpora: `diff -r` of the whole `outcomes/` tree, `diff` of the sorted `index/outcomes.jsonl`, and `diff -rq` of every run directory file other than `manifest.json` (so `trace.jsonl`, `tree.json`, `run.done` and both `raw/*.jsonl.gz` are byte-identical). The only run files that differ are the manifests: 240 of 240 and 6 of 6.

### What did change (`import.sources[]` and blobs)

| Corpus | Side | `import.sources` entries | of them stream/stderr | Blob files |
|:---|:---|---:|---:|---:|
| golden | `main` | 18,000 | 0 | 6,288 |
| golden | branch | 18,480 | 480 | 6,529 |
| mini | `main` | 151 | 0 | 124 |
| mini | branch | 163 | 12 | 131 |

Each run gains exactly two entries (stream and stderr): 480 = 2 × 240 and 12 = 2 × 6. The manifest comparison is therefore not vacuous: unfiltered, all 246 manifests differ.

### Raw header `source` digest against the store's blobs

The header is line 1 of `runs/<key>/raw/stdout.jsonl.gz`, field `source`.

| Corpus | Side | stdout headers | `source` digest has a blob file |
|:---|:---|---:|---:|
| golden | `main` | 240 | 0 |
| golden | branch | 240 | 240 |
| mini | `main` | 6 | 0 |
| mini | branch | 6 | 6 |

### Golden guard on the branch

`CANDLER_GOLDEN_CORPUS="$GOLDEN" cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_golden)'`: 2 tests run, 2 passed (`the_golden_corpus_imports_to_the_oracle_rows`, `..._runs`), 57 s. `$GOLDEN` was only read.

## Observations

| Signal | Baseline / Expected | Observed | Interpretation |
|:---|:---|:---|:---|
| Run keys and cell digests | equal | 240/240 and 6/6 equal [source: run dir listing, `index/runs.jsonl`] | No cell digest moved |
| Outcome context digests | equal | 10,320/10,320 and 136/136 equal [source: `outcomes/**`] | No context digest moved |
| Manifest, non-stream fields | equal | equal on 246 runs [source: `manifest.json`] | `import.sources[]` is the only changed field |
| Stream/stderr sources | 0 on `main` | 2 per run on the branch | The L0 fix, as designed |
| Raw header `source` blob | absent on `main`, present on branch | 0/246 vs 246/246 | The gap, measured and closed |
| Traces and raw frame files | equal | byte-identical | The raw header digest did not move |

## Steering Questions

- `[later]` The raw header field is `source`; the trace header's `raw[].digest` (the digest of the decompressed frame file) is a different value and has no blob in either store. The leaf's wording matches the raw file's header, which is what was measured.

## Pointers

- Leaf: `__roadmap__/v1-review-code-gaps/settle/digest_invariance.md`
- Scratch (removed after this report): `$TMPDIR/digest-main`, `$TMPDIR/digest-branch`, `$TMPDIR/digest-compare.sh`
- Golden guard source: `crates/candler-core/tests/guard_golden.rs`
