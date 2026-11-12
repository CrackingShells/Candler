# Digest invariance of M4 — Findings (v0)

Date: 2026-11-13

---
type: findings
topic: m4-session-runner
date: 2026-11-13
version: v0
prior-version: none (method: `__reports__/m2c-wire-alignment/digest_invariance_v1.md`)
key-metric: moved run keys / cell / context / suite-entry digests over 10,320 golden + 136 mini records and the smoke cell inputs: 0 (prior: N/A, delta: N/A)
decision-required: none
---

## Headline Result

metric: moved digests, `cf3de30` build against the milestone tip `44f6c7e` build
value: 0 (golden 240 runs / 10,320 records; mini 6 runs / 136 records; smoke resolution; M2a vectors)
unit: moved digests
prior: N/A (first M4 measurement)
direction: stable

The manifests are byte-identical on all 246 runs, `import.rules` included, because one rules file serves both builds (the branch still accepts the file `cf3de30` accepts). Two documented gaps: `candler resolve` prints no cell digest (see "Smoke resolution"), and the smoke endpoint is a local stub, not RiVault.

## Results Tables

### Builds (`cargo build --release -p candler`, `RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0`, one target dir per side)

| Side | Source | Target dir | `candler` sha256 |
|:---|:---|:---|:---|
| main | `cf3de30`, detached scratch worktree | `target-main` | `f59cd016…0e06004` |
| branch | `44f6c7e`, this worktree | `target-branch` | `0a471360…dbf8` |

Full digests: main `f59cd01615af104aa2af717fd4c711f3fc1ca659ac5e700d981431b2b0e06004`, branch `0a471360210e3a6820fcb9cd28acb412a6cd1a21a1e698c356c27cc089c4dbf8`. The binaries differ, as the sources do.

### Counts (non-vacuity)

| Corpus | Runs | Records | Import output, main | Import output, branch | Distinct cells | Distinct context digests | Distinct suite-entry digests |
|:---|---:|---:|:---|:---|---:|---:|---:|
| golden (read-only, `20260806-cont-train`) | 240 | 10,320 | `{"records":10320,"runs":240}` | `{"records":10320,"runs":240}` | 12 | 239 | 43 |
| mini (`crates/candler-core/tests/fixtures/mini-corpus`) | 6 | 136 | `{"records":136,"runs":6}` | `{"records":136,"runs":6}` | 3 | 5 | 49 |

Four stores, none shared: `store-{main,branch}-{golden,mini}`, each with its own `XDG_STATE_HOME`, `env -i`. Rules file: `crates/candler/tests/fixtures/cli/mini-corpus-rules.toml`, sha256 `b4387a08…00ccb` on both sides (`git show cf3de30:` and the worktree file are identical; it carries `entry_params`, which `cf3de30` already requires).

### Comparisons (`diff` of sorted extractions, main against branch; lines per side in brackets)

`R` = store root, `K` = `(.run.cell|sub("^sha256:";""))+"-r"+(.run.replicate|tostring)+"-a"+(.run.attempt|tostring)`.

| Comparison | Extraction | golden | mini |
|:---|:---|:---|:---|
| run keys | `command ls -1 $R/runs \| sort` | 0 diff [240] | 0 diff [6] |
| run keys, index | `sort $R/index/runs.jsonl` | 0 diff [240] | 0 diff [6] |
| `(run key, cell, context_digest)` | `find $R/outcomes -name '*.jsonl' -print0 \| xargs -0 cat \| jq -r '[K,.run.cell,.context_digest]\|@tsv' \| sort` | 0 diff [10,320] | 0 diff [136] |
| `(run key, suite_entry_digest)` | same with `.suite_entry_digest` | 0 diff [10,320] | 0 diff [136] |
| manifests, whole | `for k in $(ls $R/runs); do jq -S -c . $R/runs/$k/manifest.json; done \| sort` | 0 diff [240] | 0 diff [6] |
| `import.rules` values | `jq -r .import.rules` per manifest, `sort -u` | 1 value, equal | 1 value, equal |

### Smoke resolution (`__reports__/m2b-execution-parity/smoke_v2/`, `candler resolve --json`, no `run`)

| Item | main | branch |
|:---|:---|:---|
| exit | 0 | 0 |
| `values` (sha256 of `jq -S -c .values`) | `58d2c902…` | `58d2c902…` |
| `sources` | `c03f6c6e…` | `c03f6c6e…` |
| `settings`, after masking the store root | `7b1130a9…` | `7b1130a9…` |
| `harness` | `{"name":"claude-code","version":"2.1.311"}` | same |
| `isolation.image` | `sha256:e230e8dc0779e101…` | same |
| `isolation.env_lock` | `sha256:01e3e6b2…` | same |
| `treatment` | `htn-creator@0.2.7`, commit `6af06f8f…` | same |
| catalog snapshot | `sha256:02083e4f…` | same |
| `docker images -q` after the branch resolution against after the main one | unchanged: no rebuild between the two | |

The comparison covers the fields in this table; the whole `resolve --json` document was not diffed. The other difference seen, the `session_mode@1` preflight step, is listed under the differences below.

### Gates on the branch (target dir `target-branch`)

| Gate | Result | Last line |
|:---|:---|:---|
| DIGEST numstat (36 tracked vector/oracle files, `field_paths.toml` excluded) | PASS | `numstat rc 0`: no file with a deleted line |
| DIGEST `field_paths.toml` | PASS | 0 lines start with `-`; the file only gains 10 lines (`git diff --numstat`: `10 0`) |
| DIGEST `guard_cell_digest` | PASS | `9 tests run: 9 passed, 0 skipped` |
| DIGEST core guards (import_mini, legacy_mapping, oracle_mini, run_context, engine, seams) | PASS | `45 tests run: 45 passed, 0 skipped` |
| GOLDEN `guard_golden` | PASS | `2 tests run: 2 passed, 0 skipped` (55.8 s) |
| GOLDEN `guard_oracle_ac` | PASS | `1 test run: 1 passed, 0 skipped` |

## Observations

| Signal | Baseline / Expected | Observed | Interpretation |
|:---|:---|:---|:---|
| Run keys, cell digests | equal | 240/240, 6/6 equal | no cell digest of an import moved [source: `ex/*/keys.txt`, `ctx.txt`] |
| Context digests | equal | 10,320/10,320, 136/136 equal | none moved |
| Suite-entry digests | equal | 10,320/10,320, 136/136 equal | none moved |
| Manifests | equal (same rules file) | byte-identical on 246 runs | `import.sources[]`, provenance and rules digest unchanged |
| Smoke resolution values | equal | equal; `harness.version` 2.1.311 read by the driver's version probe on both | no input to the smoke cell moved; the image is the same id |
| Smoke preflight | equal steps | branch adds `session_mode@1` (empty `derived`, no checks) | the one difference, below |
| Smoke `settings` | equal | differ in `store.root` only | each side has its own `--store`, by design |
| Gateway assets (`gw.js`, `nginx.conf.template`) | no digest depends on them | resolve values equal with the branch's gateway files embedded | no digest reads the gateway content; `assets/docker/base` and `env` have 0 changed files since `cf3de30` |

### Every difference, with its cause

| Difference | Cause on the branch | Cause on main |
|:---|:---|:---|
| `preflight` gains the step `session_mode@1` | `crates/candler-core/src/resolve/steps/mod.rs` registers `SessionModeStep` after the catalog step (`register_m2b_steps`, the added `.register(Box::new(session))`); its `run` (`steps/session.rs`, `impl PreflightStep for SessionModeStep`) calls `settle`, which returns `Settled { derive_idle: false }` at `session.rs:126-135` when `runner.mode` and `runner.idle_s` are both absent, so it reports no derived value and no check | the step does not exist at `cf3de30` |
| `settings[store.root]` | the `--store` argument of this run (`xdg-smoke-branch/...`) | the `--store` argument of that run (`xdg-smoke-main/...`) |

No other difference exists in any of the stores or the resolutions.

### Why the comparison can fail, and why the attacks do not apply

| Attack | Why it does not apply |
|:---|:---|
| Empty extraction | counts above: every extraction has 6 to 10,320 lines per side; the stores report `records`/`runs` equal to the corpora |
| Same file or same store on both sides | four separate store roots and four `XDG_STATE_HOME`s; the two binaries have different sha256; the main side is built from a detached `cf3de30` checkout |
| Comparison that filters nothing | control 1: one hex digit of line 5 of the main mini `(key, context)` file changed makes `diff` print 4 lines against the branch file. Control 2: changing `harness.version` to `2.1.312` in a copy of the branch resolution changes the `values` hash (`448a281b…` against `58d2c902…`) |
| Shared cache | stores are separate; the only shared state is the Docker image store, which is the point of the smoke leg (below); sccache caches compilation only |
| Image rebuilt between the resolutions | `docker images -q` was recorded before (12 ids), after the main resolution (13: `e230e8dc0779` added) and after the branch resolution (unchanged); both resolutions print `isolation.image` `e230e8dc0779…`. `candler-base:src-319fda768a95` (`4ef8094986fe`) pre-existed and served both |
| Digest computed by the same code on both sides | the two builds come from different commits; they agree on 10,320 + 136 imported records. The function `RunManifest::cell_digest` (`crates/candler-schemas/src/execution.rs:2695`) lies in a file with no change since `cf3de30` (`git diff --stat` empty for it; the schemas crate changes only `trace.rs`), so its code is the same by construction; its independent oracle is the Python vectors, which `guard_cell_digest` (9/9) and `guard_oracle_mini` check on the branch, and the vector files are unedited (numstat above) |

## Contradictions & Surprises

- `candler resolve` prints no cell digest. For the smoke leg the compared quantity is the complete resolved input to the cell, not the digest: `values`, `harness`, treatment, snapshots, image id and env lock, equal byte for byte. The planner hashes `Comparable::from_resolved(values)` (`crates/candler-core/src/runner/plan.rs:426`); on the branch it adds one value, `runner.idle_s`, only when `runner.mode` is `session` and `idle_s` is absent (`plan.rs:415-423`, `session.rs:126-154`). The smoke file sets neither, so no value is added. A digest printed by a real `run` was not taken (hard limit).
- `candler resolve` always runs the gateway step, a `GET <model.endpoint>/v1/models` with the secret. To make no RiVault call, both resolutions used `--set model.endpoint=http://127.0.0.1:18765`, a local stub that answers 200, and `RIVAULT_LLM_INFERENCE=placeholder-not-a-key`. `model.endpoint` is a cell value, so these smoke values differ from the recorded M2c smoke in that one value; they are equal across the two sides, which is the comparison made. The treatment `source` path in the experiment file was also re-pointed to the local clone.

## Steering Questions

- `[now]` None open: the milestone moves no stored digest. The gap that `resolve` hides the cell digest is for the coordinator to accept or to close at the next real-model run (the digest of a live smoke cell on the final binary).
- `[later]` A `resolve --json` field that prints the cell digest would turn this leg into a digest-to-digest comparison with no model call.

## Pointers

- Leaf: `__roadmap__/m4-session-runner/traits/core/runs/closeout/revalidation/digest_invariance.md`; method: `__reports__/m2c-wire-alignment/digest_invariance_v1.md`; smoke: `__reports__/m2b-execution-parity/smoke_v2/`
- Scratch (removed after this report): `/private/tmp/claude-501/m4-digest/`, the scratch worktree `main-src`, the env image `e230e8dc0779`
