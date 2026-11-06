# M2a Evaluation parity: registry, engine, helper and the HTN plug-in

## Context
The campaign of milestone M2a (`../candler-v1/contracts/parity/m2a_evaluation_parity.md`). On M1's contracts it builds the following:
- a plug-in registry;
- the evaluation engine;
- the Python evaluator helper `candler-evaluator`;
- the HTN plug-in, which grades Tiers A–C;
- `candler import | evaluate | show | diff`.
Its proof is a live regrade of the 240-run golden corpus that reproduces the imported reference on the 8,880 A–C rows. Depth is ordering:
- L0: the frozen seams and the wire envelope;
- L1 `components/`: seven independent components;
- L2 `assembly/`: the engine, the surfaces, the helper release and the plug-in;
- L3 `parity/`: the golden guard.
M2b runs in parallel on `main`'s M1 contracts. It shares only `crates/candler/src/main.rs` (append-only) and the supervisor, which M2a uses but does not change.

## Reference Documents
- [R01 Architecture](../../__design__/architecture.md) — §Plug-ins, §Evaluator protocol, §Evaluation of one run by one evaluator, §Invariants (I1, I4, I5, I6, I11, I14, I15, I21, I22), §Golden corpus
- [R02 Milestone leaf](../candler-v1/contracts/parity/m2a_evaluation_parity.md) — scope and success gates, as amended for Tiers A–C (Amendment Log A1)
- [R03 Architecture report](../../__reports__/m2a-evaluation-parity/00-architecture_v0.md) — decisions 1–13, the quirk table, the kind signature, the skeleton seams
- [R04 Test definition](../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md) — risks R1–R10, the 31 cornerstone guards with their oracle and bite, the dropped list, the promotion rule, the fixture budget
- [R05 M1 conventions](../m1-contracts/README.md) — Gotchas: condition tags, STD/DRIFT/SUBJ/DEP, the schema publish rule, test classes, oracle rules 1–7, shell safety. Every M1 Gotcha applies here unless this file overrides it.
- [R06 Golden corpus](/Users/hacker/Documents/src/External/PCfVW/agenticai4hpc26-artifact) — tag `candler-m2a-golden`, extracted at `$GOLDEN`; never written
- [R07 HTN repo](/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion) — `$R`. The instrument and grader are at commit `28be2c15` (`htn-creator-eval@0.4.2`). Work happens in the worktree `$RW` on `feat/candler-htn-plugin`. The main checkout holds Eliott's uncommitted work and is never touched.
- [R08 Grit](https://github.com/PCfVW/Amphigraphic-Strict/tree/master/Grit) — the `rust-conventions` skill and its reviewer agent
- [R09 M1 KT](../../__reports__/m1-contracts/01-knowledge_transfer_v0.md) — I22 first, the recording fake `Signaller`, `context_digest_runs.toml`

## Goal
A live `htn-creator-eval@0.5.0` regrade of the golden corpus reproduces the imported `0.4.2` rows on Tiers A–C. It reproduces itself on a second regrade, through a core that names no domain.

## Pre-conditions
- [ ] [nl] Eliott approved the plan (2026-11-06), and the M2 coordinator OK'd the A–C gate wording. Proxy: both replies are quoted in Amendment Log A1.
- [ ] [cmd] ENV: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the checkout's path
- [ ] [cmd] `git merge-base --is-ancestor origin/main feat/m2a-evaluation-parity`
- [ ] [cmd] `git log --format=%s origin/main..feat/m2a-evaluation-parity | grep -q '^build(deps): '`, and `grep -q '"evaluator-helper"' cog.toml`
- [ ] [cmd] `git -C "$RW" rev-parse --abbrev-ref HEAD | grep -qx feat/candler-htn-plugin && git -C "$RW" merge-base --is-ancestor 28be2c15 HEAD`
- [ ] [cmd] `git -C "$R" show 28be2c15:.claude/skills/htn-creator-workspace/infrastructure/evals/eval.json | shasum -a 256 | grep -q '^ca7f12afee55152e'`
- [ ] [cmd] `CANDLER_GOLDEN_CORPUS="$GOLDEN" cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'test(oracle_aggregates)'` (10,320 rows)
- [ ] [cmd] Before the parity gates only: `git -C "$R" rev-parse -q --verify 'refs/tags/htn-creator-eval@0.5.0^{commit}'`. The tag is cut through that repo's own release flow on Eliott's go; no gate points at a branch, and the golden reference stays `0.4.2`.

## Success Gates
- ⬜ [cmd] STD, DRIFT, SUBJ, PYSTD, HTNSTD, SCAFFOLD and NOPUSH pass on `feat/m2a-evaluation-parity` and `feat/candler-htn-plugin`
- ⬜ [cmd] `CANDLER_GOLDEN_CORPUS="$GOLDEN" cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'binary(guard_golden_parity)'` passes (GP1–GP3)
- ⬜ [cmd] `cd "$RW/$PLUGIN" && CANDLER_GOLDEN_CORPUS="$GOLDEN" uv run --frozen pytest -q -m golden tests/test_guard_kind_parity.py` passes (KP1), and fails when `CANDLER_GOLDEN_CORPUS` is unset
- ⬜ [cmd] `candler diff --from htn-creator-eval@0.4.2 --to htn-creator-eval@0.5.0 --equivalence "$EQ" --format json | jq -e --arg eqsha "$(shasum -a 256 "$EQ" | cut -d' ' -f1)" '.compared == 8880 and .changed == 0 and .excused == 209 and .unchanged + .excused + .changed == .compared and .equivalence.sha256 == $eqsha and .compared + (.uncompared | map(.count) | add) == 10320'`, and `diff <(candler show --rates --evaluator htn-creator-eval@0.5.0 --format toml) $ORACLE/rates_by_cell.toml` prints nothing
- ⬜ [cmd] `dirtree-rdm validate` passes on every node, and every leaf's `merge(<scope>): land <leaf>` is on `git log --first-parent feat/m2a-evaluation-parity`
- ⬜ [nl] Every leaf is `done`. No verifier or Grit review holds an open BLOCKER or MAJOR. The KT carries the per-cell gate-rate table against the published rates.

## Gotchas
- **Every M1 Gotcha holds (R05).** That covers condition tags, the schema publish rule, test classes A/B, oracle rules 1–7, the bite record, shell safety, Grit lessons, the wire rules, and reporting spec defects. Only what differs or is new is listed below.
- **Variables.**
  - `$GOLDEN` is `/Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`.
  - `$R` is `/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion`.
  - `$RW` is `/Users/hacker/Documents/tmp/claude-worktrees/mcp-python-ingestion/candler-htn-plugin`.
  - `$PLUGIN` is `.claude/skills/htn-creator-workspace/candler-plugin`.
  - `$HELPER` is `python/candler-evaluator`.
  - `$ORACLE` is `crates/candler/tests/oracle` (from `reference_oracle`) and `$EQ` is `$ORACLE/equivalence.json`, generated by `$ORACLE/gen_oracle.py` with the 209 excused rows named by run key.
  - **Shared gates** (M1's STD, DRIFT, SUBJ and DEP(x) unchanged, with base `feat/m2a-evaluation-parity`):
  - **PYSTD**: `cd $HELPER && uv run --frozen pytest -q && uv run --frozen ruff check && uv run --frozen ruff format --check`.
  - **HTNSTD**: `cd "$RW/$PLUGIN" && uv run --frozen pytest -q && uv run --frozen ruff check`, plus, in `$RW`, `mamba run -n forMpyIn pytest -q -p no:cacheprovider` with no new failure against the Phase 0 baseline recorded in R03 §Environment (`__reports__/m2a-evaluation-parity/htnstd_baseline.txt`).
  - **OWNED**: `test -z "$(git diff --name-only <base>..HEAD | grep -vE '<leaf owned regex>')"`.
  - **SCAFFOLD**: for every test file the leaf owns, `test -z "$(git grep -nE 'mod scaffold|mark\.scaffold' -- <files>)"`, and the count of `#[test]`/`#[tokio::test]`/`def test_` equals the count of `Guards:` lines.
  - **NOPUSH**: `test -z "$(git ls-remote origin feat/m2a-evaluation-parity)" && test -z "$(git -C "$R" ls-remote origin feat/candler-htn-plugin)"`. The `htn-creator-eval@0.5.0` tag is outside NOPUSH: it is pushed by that repo's release flow, on Eliott's go, and nowhere else.
  - **SPAWN** runs before any test that spawns a process: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)'`.
- **Scope: Tiers A–C.**
  - D-E1…D-E6, snapshot attach, suite `inputs[]`, `SuiteFile`, the `rikyu_mcp` rule and the mcp-python-ingestion install are out of scope.
  - The registry refuses a `snapshots` or `inputs` need with `unsupported in this version: <need>`.
  - The parity diff lists the 1,440 D-E rows as uncompared.
- **Signals (I22).**
  - No evaluation code signals anything. The protocol client stops an evaluator through `Supervisor`/`StopHandle` only.
  - Mutation and bite tests that touch stopping use only the recording fake `Signaller` of feature `test-support`.
  - A bitten file is restored with `git checkout -- <file>`, never `mv`.
  - Tests run as non-root. Nobody runs `kill`, `pkill` or `killall`.
- **Test classes (R04).**
  - A leaf commits only the guard IDs R04 assigns it.
  - Each guard carries `Guards: <R#|I#> — <property>. Oracle: <source>.` (Rust `///`, Python docstring first line).
  - Development tests live in `#[cfg(test)] mod scaffold` or under `@pytest.mark.scaffold` and are deleted by the leaf's last commit, unless promoted by R04's rule.
  - R04's **dropped list** is binding: a leaf that commits one of those tests has a defect.
  - One-shot checks are `[cmd]` gates or report evidence, never tests.
- **Oracle independence.**
  - Expected rows and rates come from raw corpus files read by a Python script that imports nothing from Candler.
  - `equivalence.json` is generated from the raw `max_score == 0` passes, with each run key's cell digest computed from R01's text (M1's `gen_vectors.py` rule). Its `from`/`to` pair is a literal in the script. A row is excused only when it shows exactly that pair; any other listed or unlisted row is a change.
  - No golden constant is copied from code. Guards read the oracle files.
- **The instrument.**
  - The plug-in's lock installs `htn-creator-scripts` and `htn-creator-eval` from `git+file://$R@28be2c15#subdirectory=…`, never editable and never from `$RW`.
  - An editable or `$RW` source in `uv.lock` is a defect.
  - The live digest reads `git:28be2c15…`.
- **The grader is adapted, not wrapped.**
  - `grader.py` and the checkers stay byte-identical at `28be2c15`. They are installed, never copied.
  - Kinds are pure functions `(params, context, prior) -> outcome` in the plug-in, with the signature R03 pins.
  - Quirks that change A–C rows are params. Redesigned behaviours are listed in `deltas.toml`.
- **Cross-repo pin.**
  - The plug-in's lock takes `candler-evaluator` from `git+file://<Candler main checkout>@<sha>#subdirectory=python/candler-evaluator`, with `<sha>` a commit of `feat/m2a-evaluation-parity`.
  - After the final `rebase --rebase-merges origin/main`, the coordinator re-pins with `uv lock --upgrade-package candler-evaluator`. The coordinator then checks `git merge-base --is-ancestor <pinned sha> feat/m2a-evaluation-parity`.
- **Owned files.**
  - Phase 0 holds the manifests: workspace and crate `Cargo.toml`, `Cargo.lock`, `cog.toml`, CONTRIBUTING, `.gitignore`, `ci.yml`.
  - `skeleton` holds `candler-core/src/lib.rs`, `candler-core/src/evaluation/mod.rs`, `crates/candler/src/main.rs`, `crates/candler/src/commands/mod.rs`, `public_surface.rs` anchors and `guard_layering_i1.rs`.
  - A leaf edits only its own module and its own `mod <leaf>_surface` block.
  - A new dependency is a request to the coordinator.
- **Environment.**
  - In a Candler worktree, check ENV first.
  - In `$RW`, check that `uv run --frozen python -c 'import htn_creator_candler,sys;print(htn_creator_candler.__file__)'` prints a path under `$RW`, and that `mamba run -n forMpyIn python -c 'import mcp_python_ingestion as m;print(m.__file__)'` resolves as R03 §Environment records.
  - Local tools: cargo 1.99 with nextest, uv, ruff via `uv run`, jq.
- **Verification policy (R04 §Verification).**
  - Verifiers are dispatched the moment a leaf's gates pass: plain on `reference_oracle`, `helper_runtime` and `protocol_client`; adversarial on `htn_kinds`, `run_context`, `htn_plugin` and `golden_parity`; an API review on `skeleton`.
  - The `htn_kinds` verifier closes before `htn_plugin` step 3.
  - Every verifier names tautological tests against the dropped list, re-runs three bites from the report plus one of its own, and runs SCAFFOLD.
  - A Grit reviewer covers the Rust of L0+L1 and of L2+L3.
- **Critical conditions, ranked:**
  1. diff exactness (DF1, DF2);
  2. oracle independence and numbers (OR1);
  3. equivalence and uncompared scope (DF2, GP2);
  4. kind fidelity (KP1–KP3, HP1);
  5. instrument identity (HR2, GP3);
  6. the cross-repo pin survives the rebase;
  7. the digest seam and a non-vacuous I4 (ENG2, ENG3, GP3);
  8. one item's failure costs one item, with zero `deadline` rows (HR1, PC1, GP3);
  9. Phase 0 and skeleton prerequisites;
  10. the schema ratchet is additive and `rpc` wraps the existing roots (DRIFT, WIRE2).
- **Nothing is pushed or published** in any repo until Eliott says so. That covers the integration branch, the HTN topic branch, the `htn-creator-eval@0.5.0` and `candler-evaluator@*` tags, crates.io and PyPI.

## Status
```mermaid
graph TD
    skeleton[Skeleton: frozen seams and stubs]:::done
    protocol_wire[Protocol wire: JSON-RPC envelope]:::done
    components[Components]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `skeleton.md` | 📄 Leaf Task | ✅ Done |
| `protocol_wire.md` | 📄 Leaf Task | ✅ Done |
| `components/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A1 | 2026-11-06 | __reports__/m2a-evaluation-parity/00-architecture_v0.md | [] | Eliott approved plan rev 4 (A–C only, live 0.5.0, 0/0 pass → not_applicable {empty_population}); coordinator OK'd the A–C gates with Tier D dropped, the 0.5.0 tag cut by the HTN release flow before the parity gates, and an exact-row equivalence file that is generated, digest-recorded, counted as `excused` and attacked by the verifier |
| A2 | 2026-11-06 | __reports__/m2a-evaluation-parity/skeleton/00-report_v0.md | [] | The skeleton API review widened the frozen seams (env root, suite file, supervisor and cancel, deadline default, `show --evaluator`, `commands/common.rs`, multi-thread runtime, cfg(unix) registry and engine), moved the consuming leaves' findings into their steps, and replaced the `du -sk` fixture gates with byte counts; the M2 coordinator was told of the shared CLI changes |
| A3 | 2026-11-06 | __reports__/m2a-evaluation-parity/registry/00-findings_v0.md | [] | The registry found `KindSpec` only in `describe` and no supervisor for the environment step: `check_suite` now takes the `DescribeResult`s the engine gets after the preflight (R01: `describe` is a preflight step), and `register_evaluation_steps` takes the shared `Arc<Supervisor>`; engine, surfaces and R03 follow; the M2 coordinator added the Ctrl-C handler condition to `surfaces` |
| A4 | 2026-11-06 | __reports__/m2a-evaluation-parity/l1/00-rulings_v0.md | [] | L1 reports: cells are keyed by cell digest hex in the store, `show --rates` and the oracle (with `cells.tsv` naming directories), so the rates byte gate stands; the oracle limit is 72 KB; stdin stays open for `cancel` (R01 §Lifetime); the engine writes unreported items and sets role and digests itself; the HTN manifest sits above its uv project; two OWNED regexes fixed |
| A5 | 2026-11-06 | __roadmap__/m2a-evaluation-parity/components/assembly/surfaces.md | [] | M2 coordinator ruling on the exit-code clash with M2b: every verb returns `Result<Exit, CliError>`, `Exit::{Success, Changed}` maps to 0 and 1, and any `CliError` maps to 2. `CliError` takes M2b's variants, and `common.rs` stays the shared helper home. The new guard CLI3 brings the count to 32. surfaces' OWNED regex is unanchored for `fixtures/cli/` and admits `main.rs` and `mod.rs` |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
