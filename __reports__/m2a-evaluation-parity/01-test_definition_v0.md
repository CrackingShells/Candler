# M2a evaluation parity — Test Definition (v0)

Date: 2026-11-06

The roadmap cites this report as R04. A leaf commits only the guards this report assigns it.

## Executive Summary
- **What must be proven.** A live `htn-creator-eval@0.5.0` regrade of the 240-run golden corpus reproduces the imported `0.4.2` rows on the 8,880 A–C rows, field by field, and reproduces itself on a second regrade. It does so through a core that names no domain, under the pinned instrument `git:28be2c15`, with no `deadline` row.
- **Top risks.** R1–R10 below. Each has one to four cornerstone guards. The apex guards are GP1–GP3, which read every expected number from an oracle that imports nothing from Candler.
- **Volume.** 31 cornerstone guards: 23 Rust (4 of them `#[ignore]` corpus tests), 3 in the Python helper and 5 in the HTN repo. There are 2 development tests and 15 one-shot `[cmd]` gates.
  - M1 committed about 90 tests. The heuristic for five features is 20–30.
  - The extra guard is justified because the milestone's one claim is parity, and it spans three languages and two repos.

## Scope

### In scope
- The registry, the context, the protocol client and envelope, selection, rates and diff, the engine, the four CLI commands, the Python helper, and the HTN plug-in's kinds and suite.
- Their parity on Tiers A–C.

### Out of scope
- Tier D, snapshots and suite inputs. See R03 §Scope move.
- The M2b execution path.
- `uuid`, `serde`, `clap`, `jsonschema`, `uv` and the OS behaviour of `fork` and hard links: we test what we own.

### Assumptions
- `$GOLDEN` is the extracted `candler-m2a-golden` tag, with 10,320 rows.
- `$R` holds commit `28be2c15`, with `eval.json` sha `ca7f12af…`.
- The tools are local cargo with nextest, uv and jq. The `forMpyIn` mamba env is used only for the HTN repo's existing suite.

### Trust boundaries

| Boundary | Real or substituted |
|---|---|
| Golden corpus | Real; read-only; local only |
| Pinned grader and checkers | Real, installed `git+file` at `28be2c15`; KP2 extracts `grader.py` by `git show` into tmp |
| Evaluator processes | Real `uv`-run helpers (synthetic, echo, HTN) or hand-written `sh` stubs (PC1) |
| Stopping a process | The recording fake `Signaller` (feature `test-support`) wherever a stop is exercised |
| `uv` in REG2 | A stub on `PATH` that counts invocations |
| Store | A temp store from the M1 mini-corpus or from `$GOLDEN` |

## Risks

| Risk | What could go wrong | At stake |
|---|---|---|
| R1 | The live regrade does not reproduce the 8,880 rows, or the rates drift | parity |
| R2 | `diff` is blind to a field or to a missing or extra row; the equivalence or uncompared set is wrong | diff exactness |
| R3 | The oracle shares a source with the code, or its numbers are wrong | oracle independence |
| R4 | A kind diverges: int vs float, a kept quirk, a param the corpus never measures | kind fidelity |
| R5 | A nondeterministic regrade, or a vacuous second regrade, passes | I4 |
| R6 | A hang or exit costs more than one item, a `deadline` row appears, or a failure is mis-mapped | I14, I22 |
| R7 | The instrument is not the pinned commit | instrument identity |
| R8 | The core learns HTN vocabulary, an unsupported manifest is accepted, or the envelope duplicates its roots | I1, scope |
| R9 | `context_digest` is not computed from what the plug-in received, or a plug-in writes into the store | I11 |
| R10 | The Python models and the Rust schema disagree | the cross-language wire |

## Test Matrix
Every guard carries `Guards: <R#|I#> — <property>. Oracle: <source>.`, as a Rust `///` line or a Python docstring first line. Its leaf report records the bite: the mutation tried, the failure seen, and the restore by `git checkout -- <file>`. `ign` means ignored, run locally against `$GOLDEN` with `--no-tests=fail` or an equivalent that fails when the variable is unset.

| Group | Scenario | Risk it covers | Tier | Setup / Data | Assertion (observable) |
|------:|----------|----------------|------|--------------|-------------------------|
| GP1 golden_parity | Live A–C rows and rates equal the reference | R1 | integration end_to_end, ign | `$GOLDEN` imported as 0.4.2, regraded with the HTN plug-in | 7,790 pass / 209 NA / 415 fail / 466 blocked; row by row on 4 fields; `show --rates` equals `rates_by_cell.toml`. Oracle: `counts.toml`, `rates_by_cell.toml`, `na_pairs.txt`. Bite: C6 `round = 1` |
| GP2 golden_parity | The version diff is exact and complete | R2, R1 | integration end_to_end, ign | Same store | `compared == 8880`, `changed == 0`, `unchanged + excused + changed == compared`, `excused` = line count of `na_pairs.txt`, `equivalence.sha256` = sha256 of the committed file, compared + Σ uncompared = 10,320, uncompared = hand literal D-E1…6 × 240. Bites: one stored 3 → 4; one excused row's live record set to `not_measured {instrument_error}` must be one `equivalence_mismatch` |
| GP3 golden_parity | The second regrade is new and identical | R5, R7, R6 | integration end_to_end, ign | Second regrade | Disjoint request ids, 8,880 rows, empty diff with no I4 violation, every digest `git:28be2c15…`, zero `deadline`. Oracle: the sha literal. Bite: reuse the request ids |
| OR1 reference_oracle | A second engine re-derives the reference | R3 | regression, ign | Raw corpus via `serde_json::Value` only | Counts, `na_pairs.txt`, per-cell sums and the `(N, expectation)` multiset of `equivalence.json`'s rows equal the committed files. Role ids are a literal from `eval.json`, and counted statuses a literal from `_outcome.py`. Bites: wrong role; one equivalence row dropped |
| DF1 selection_rates | Each change class is seen | R2 | regression | Hand-built base that diffs clean | One row per class (status, value 3→4, value 3→3.0, weight, cause kind, cause present→absent, missing, extra): `changed == 1` with that class; evidence-only gives 0. Bite: drop weight |
| DF2 selection_rates | Only the listed rows are excused, only for the exact pair, in one direction | R2 | regression | Hand-written fixture `equivalence.json` naming a few base rows | A listed row showing exactly 0/0 `zero_weight` pass → NA `empty_population` is counted in `excused`, never `unchanged`, and `equivalence.sha256` is the file's. Each is a change with its class: that change on an unlisted row; a listed row shown as a 1/1 pass → NA, as a 0/0 pass with another cause, reversed, or unchanged (`equivalence_mismatch`); a listed row absent (`equivalence_missing`). An unknown key or a duplicate row is refused (exit 2). The uncompared list holds counts. Bite: excuse a listed row whatever pair it shows |
| SEL1 selection_rates | I4 is flagged, recovery allowed | R5 | regression | Hand-built records under one key | A changed field gives `I4Violation`; error→measured is not flagged. Oracle: the I4 text. Bite: remove the exception |
| RT1 selection_rates | The rate formula | R1 | regression | Three hand-computed cells | Gate and quality sums equal literals; NA is out, `blocked` is in with 0. Oracle: R03 §Generic diff prose of `_outcome.py`. Bite: include NA |
| ENG1 engine | Planning | R6 | integration component | Synthetic plug-in, 4 entries | `applies_to` miss gives NA; a dependent of a fail is `blocked {expectation}`; a dependent of a pass got `prior[]` in the same request. Bite: skip the dependency check |
| ENG2 engine | The persisted digest is of what was sent | R9 | integration component | Echo plug-in on the mini-corpus | Persisted `context_digest` equals the RFC 8785 sha256 of the echoed context and equals `live_context_runs.toml`. Bite: digest another projection |
| ENG3 engine | A nondeterministic kind is caught | R5 | integration component | Counter kind, two regrades | Selection reports an I4 violation on the second. Bite: reuse the cached outcome |
| CLI1 surfaces | The JSON contract | R2 | regression | Tiny seeded store | `diff --format json` key sets equal the hand literal at each level, `equivalence` included; exit 1 on a planted change, 0 on none. Bite: rename `compared` |
| CLI2 surfaces | The whole path | R1, R2 | integration end_to_end | M1 mini-corpus with the synthetic plug-in | import→evaluate→show→diff; `compared > 0`; one edited stored value makes `diff` exit 1. Bite: `diff` always exits 0 |
| REG1 registry | Refusals | R8 | regression | Valid manifest and suite, single-fault edits | Positive control passes; unknown kind, bad params, unmeetable need, and `snapshots`/`inputs` with the exact text are refused. Bite: skip the `inputs` check |
| REG2 registry | Environment reuse | R6 | regression | Stub `uv` counter | A second preflight under the same digest runs the stub zero more times, and the env dir inode and mtime are unchanged. Oracle: OS `stat`. Bite: rebuild on rerun |
| RC1 run_context | The live digest | R9 | regression | Mini-corpus store | Digest equals `live_context_runs.toml`; a one-byte twin differs. Oracle: Python `rfc8785` vectors. Bite: omit artifacts |
| RC2 run_context | The file rule | R9 | regression | Synthetic trees | Nested match, `.claude/`, `._*`, `.DS_Store`, empty `outputs/`: materialised set and digest path list equal hand-listed sets. Bite: keep `.claude` |
| RC3 run_context | No write-through | R9, I11 | regression | Materialised workspace | Appending to a materialised file leaves the store blob's sha unchanged. Bite: hard link |
| PC1 protocol_client | Failure paths | R6 | integration component | Six `sh` stubs, fake `Signaller` | Unreported item gives `not_measured {instrument_error}`; request deadline gives `deadline`; cancel is answered `-32800`; late result loses; partial frame gives `instrument_error`; 4 MB stderr completes. Bite: accept the late result |
| PC2 protocol_client | No signals in evaluation | R6, I22 | regression | `assert_clean` scan | No `kill(`, `killpg` or `Signal::` under `src/evaluation/`. Bite: plant `libc::kill` |
| WIRE1 protocol_wire | The envelope | R8 | regression | Shared `fixtures/rpc/{valid,invalid}` | Valid files parse; single-fault invalid files are refused; a `new_v7` id parses. Oracle: JSON-RPC 2.0 §4–5. Bite: accept `"1.0"` |
| WIRE2 protocol_wire | The envelope wraps the roots | R8 | regression | A payload from `evaluator-notification` | Accepted inside the envelope; a single-fault copy refused. Oracle: the existing root's schema. Bite: loosen the payload |
| NEUT skeleton | The core stays neutral | R8, I1 | regression | Existing deny-list over 5 folders | No hit in core, harness, schemas, binary or helper `src`. Oracle: literal deny-list with its planted self-test. Bite: plant `rikyu` |
| HR1 helper_runtime | One fault costs one item | R6 | regression | Synthetic plug-in: hang, exit, print, `os.write(1)`, subprocess echo | Only those items are `not_measured`; `ok` items are right; real stdout holds only valid frames. Bite: drop the fd redirect |
| HR2 helper_runtime | Truthful identity | R7 | regression | Dist-info trees in `tmp_path` | Clean `git+file` gives `git:<sha>`; editable gives `dirty = true`; none is refused. Oracle: PEP 610. Bite: ignore `editable` |
| PYM1 helper_release | Python models match the wire | R10 | regression | The same shared rpc vectors | Valid files parse with the generated models; invalid ones are refused. Bite: make a required field optional |
| KP1 htn_kinds | Kind parity on the corpus | R4 | integration component, ign | 240 runs × 37 from the corpus's own `outputs/` | Rows equal the raw `eval-K-grading.json` (int vs float included); B8, C1 and C6 quirk rows present; 240 and 8,880 asserted first. Bite: flip `on_missing` |
| KP2 htn_kinds | Divergences are exactly the listed ones | R4 | regression | Pinned `grader.py` in a separate process on synthetic fixtures | Differing (fixture, field) pairs equal `deltas.toml`: none unlisted, none stale. Proves glue only. Bite: add an unlisted divergence |
| KP3 htn_kinds | KP1 can fail | R4 | regression | Synthetic fixtures | A mutated reference row, and a flipped regex hit in an `outputs/` copy, each fail KP1's compare. Bite: compare a subset |
| HP1 htn_plugin | The suite equals its source | R4 | regression | `eval.json` at `ca7f12af` | The inverse fold equals the A–C source except the declared moves; 37 entries, no `{…}`. Bite: swap a weight |
| HP2 htn_plugin | The translator refuses other sources | R7 | regression | A pristine and a one-byte-changed copy | Exit 0 on pristine (positive control), non-zero on the changed copy. Oracle: the sha literal. Bite: skip the sha check |

### Development tests (class A)
These are deleted or promoted at the leaf's last commit:
- the skeleton's `NotBuilt` check;
- the client's deadline arithmetic on an injected clock. The `Clock` trait stays as design, and PC1 ships.

### One-shot `[cmd]` gates and report evidence
These are never committed as tests:
- `cargo check`;
- the API review;
- the schema-additive diff and commits touching only schemas (DRIFT and OWNED);
- the `jq` cross-check of 7,999/415/466;
- the oracle's no-`candler`-import grep;
- the sha of `rates_by_cell.toml`;
- byte-identity of `grader.py` and the checkers;
- "kinds do not import `candler_evaluator`";
- KP1 wall-clock;
- the fork probe with 20 repeats;
- `uv lock --check`, no editable or `$RW` source in the lock, and the `direct_url.json` commit;
- the one-shot mutation sweep (survivors that matter become HP1 rows);
- model regeneration with `git diff --exit-code` (also a CI step);
- tag equals version, and the CI job is present;
- the CONTRIBUTING line;
- the bite records.

## Fixtures / Test Data Strategy
- **Budget.** New committed fixtures total 150 KB or less. Generated fixtures ship with their generator.

| Fixture | Where | Limit | How it is made |
|---|---|---|---|
| Synthetic and echo plug-ins | `python/candler-evaluator/examples/` | ≤ 10 KB | Hand-written |
| PC1 stubs | `crates/candler-core/tests/fixtures/client/` | 6 × ≤ 2 KB | Hand-written POSIX `sh` |
| Synthetic run trees | `crates/candler-core/tests/fixtures/…` | ≤ 3 trees, ≤ 20 KB | Committed `gen_run_trees.py` |
| Shared wire vectors | `crates/candler-schemas/tests/fixtures/rpc/` | ≤ 10 KB, one copy for Rust and Python | Hand-written from JSON-RPC 2.0 and R01 |
| Oracle outputs | `crates/candler/tests/oracle/` | ≤ 72 KB (the 209-row files hold a 64-hex key per row) | `gen_oracle.py` |
| Live context vectors | `crates/candler-core/tests/oracle/vectors/live_context_runs.toml` | small | `gen_live_context.py` |
| `deltas.toml` and kind fixtures | `$PLUGIN/` | ≤ 5 KB and ≤ 20 KB | Hand-written from the grader's text |

- **Reused.** The M1 mini-corpus and the M1 vectors are reused.
- **Generated in tmp.** The HR2 dist-info trees.
- **Never committed.** The golden corpus stays local.
- **Real or substituted.** See Trust boundaries. The only substituted process is `uv` in REG2. Stops always use the recording fake `Signaller`.

## Observability Requirements
- Each leaf report holds, for every guard it adds:
  - the gate run log;
  - its bite: the mutation, the failing output, and the restore;
  - the SCAFFOLD gate output.
- GP1–GP3 and KP1 logs carry their wall-clock. GP3's log carries the count of `deadline` rows and the digest prefix.
- The oracle script prints the `[published]` rate comparison to stdout, and the KT quotes it.

## Minimal Must-Run Regression Set
Before every merge:
- GP2 (local, ignored);
- DF1;
- SEL1;
- KP1 (local);
- HR1;
- ENG2;
- REG1;
- NEUT.

## Verification
Verifiers are dispatched by risk, as Sonnet subagents. Each gets the code and a named attack target, and assumes something is wrong.

| Leaf | Kind | Attack target |
|---|---|---|
| skeleton | API review | Seam fit with `engine`, `surfaces`, `selection_rates` and M2b's CLI |
| reference_oracle | plain | Role membership against `eval.json`, the counted set against `_outcome.py`, source independence, the 209 rows |
| htn_kinds | adversarial | The runpy glue (argv, cwd, `sys.path`, `sys.modules`), capture ordering, `SystemExit` forms, the file rule, int vs float, the 0/0 path |
| helper_runtime | plain | Lock state inherited across the fork, pre-fork imports, the digest across items |
| run_context | adversarial | The projection, links vs digest inputs, AppleDouble, empty `outputs/`, I11 write-through |
| protocol_client | plain | Late-result and cancel races, pipe deadlock |
| htn_plugin | adversarial | Params the corpus never measures (from the sweep), weights, `depends_on`, optional flags |
| golden_parity | adversarial | Every constant is read from an oracle file, the bites reproduce, the vacuity counts cannot be gamed |
| others | merged on report | — |

- Every verifier:
  - names any tautological test against the dropped list;
  - re-runs three of the report's bites plus one of its own;
  - runs SCAFFOLD.
- The `htn_kinds` verifier closes before `htn_plugin` Step 3.
- A Grit reviewer covers the Rust of L0+L1, and again of L2+L3.

## Scope Control Notes
- **Consolidations.**
  - The diff classes are one parameterised table (DF1), and so are the stub failures (PC1) and the file rule (RC2).
  - The one-byte digest twin is folded into RC1.
  - The 37-entry and no-template checks are folded into HP1.
- **Promotion rule.** A development test is promoted only if all four hold:
  - it names an invariant that outlives the leaf;
  - its oracle lies outside the code, the generated schema and shared constants;
  - a one-line mutation makes it fail;
  - no committed guard already covers its risk.
- **SCAFFOLD gate.** Per owned test file, `test -z "$(git grep -nE 'mod scaffold|mark\.scaffold' -- <files>)"`, and the count of tests equals the count of `Guards:` lines.
- **Tautology traps and their fixes.**
  - The oracle reads only raw corpus files, never the importer's output, and OR1 is a second engine.
  - KP1's inputs are the corpus's own `outputs/`, and its expected rows are the recorded grading JSON.
  - `rates_by_cell.toml` is produced by Python `_outcome.py` and never blessed by Rust; RT1 is the hand-literal cross-check.
  - `equivalence.json` is generated by `gen_oracle.py` from raw `max_score == 0` passes, with cell digests computed from R01's text, never read from the importer or the store. GP2's `excused` count comes from `na_pairs.txt`, built from the same raw files, and the diff counts excused rows apart from unchanged ones, so the file cannot hide a mismatch: a listed row that does not show the exact pair is a failure.
  - The Python models are tested against the hand-written vectors that Rust shares.
  - API-review findings become behaviour in the owning guard, never signature re-checks.
- **Dropped list (binding).** A leaf that commits one of these has a defect:

| Dropped | Why |
|---|---|
| "Stubs compile" | The compiler is the oracle |
| `--help` exits 0 or lists 4 commands | Re-checks clap |
| CLI JSON validates against its committed schema | Generated from the same types |
| Hard link `nlink > 1` | OS detail; RC3 asserts the observable |
| "Preflight step is registered" | Shared constant |
| v7 version bits | The uuid crate's behaviour |
| The `-32800` literal in the wire test | Shared constant |
| Quirk counts against `counts.toml` | The oracle's own numbers; row parity covers them |
| Model drift check as a test | Self-referential; PYM1 and the regeneration gate replace it |
| Planted rows `blocked→not_measured`, "D-E row added", "A–C row removed" | Duplicate DF1 classes |
| "One byte changes the digest" as its own test | Folded into RC1 |
| "Persisted digest equals a fresh `run_context` digest" | Same code both sides; ENG2 replaces it |
| The 37-entry and no-template `jq` checks | Folded into HP1 |
| Golden constants copied from code | Read from the oracle files |
| A committed mutation-sweep survivor list | Asserts nothing and goes stale |
