# m3-lean-cli

## Context
Milestone M3 of `candler-v1`. It runs off `main` at a26fe53 (`v0.6.0`, `schemas@0.6.0`), on `feat/m3-lean-cli`. It makes every operation of R01's API a thin `candler` verb that derives what it can and works from any directory. It runs `evaluate` as a supervised job, adds `ingest`, `inspect` and `analyze` with unpaired statistics, and builds amendment A5's refusal codes and contract pages. Its first leaf fixes the stray sweep's same-tick defect, which fails most PR CI runs. M4 runs in parallel on `feat/m4-session-runner`; the coordination session's rulings of 2026-11-11 govern the overlap (R10 §0).
Depth is ordering:
- **Phase 0**, the coordinator's, comes before any dispatch.
  - On `m3/scaffold`, cut from a26fe53, it builds `build(deps)`, the `refactor(cli)` scaffold and the `docs/contract/codes.toml` skeleton. That branch lands on `main` in an early PR beside the sweep fix's PR (R10 D1), and the coordinator also merges it into `feat/m3-lean-cli` locally.
  - It writes the oracles `refusal_scope.tsv`, `verbs.txt` and `quickwins.tsv` beside this README.
  - It answers probes P2 and P6, and builds the `m3-linuxrun` image.
- **L0, this level,** has four leaves that share no file:
  - the sweep fix, on its own branch from a26fe53 and its own PR;
  - the wire types;
  - the statistics;
  - the platform seam.
- **L1, `verbs/`,** holds the core refusal codes, the evaluate job body, `analyze`, `resolve`/`ls`/`watch`/`ps`, `ingest`/`inspect`, the contract-page frame and R01. It starts once `feat/m3-lean-cli` has rebased onto `main` with both early PRs merged.
- **L2, `surfaces/`,** holds the `evaluate` verb and the remaining refusal codes.
- **L3, `closeout/`,** holds the contract pages' refusals and the CLI surface audit.

## Reference Documents
- [R01 Architecture](../../__design__/architecture.md) — §Operation API, §Configuration, §Budget, §CLI surface over the jobs, §Job, §Process supervision (line 408, I22), §Wire schema: outcome record (rates, selection rule), §Invariants (I2, I7, I10, I12, I14, I15, I17, I20, I22)
- [R02 Meta leaf](../candler-v1/contracts/parity/alignment/surfaces/m3_lean_cli.md) — the gates this campaign decides
- [R03 Gap analysis A5](../../__reports__/contract-docs/gap_analysis_contract_docs_v0.md) — the contract pages' frame; refusals without codes
- [R04 CLI defaults audit](/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion/__reports__/candler_core/02-findings_cli_defaults_v0.md) — the ten quick wins
- [R05 M2c KT](../../__reports__/m2c-wire-alignment/00-knowledge_transfer_v0.md) — "Flakes seen" (b33), "Pain Points" (the lessons these gotchas encode), "Follow-ups" (the two CLI items `refusal_core` takes)
- [R06 M2c campaign](../m2c-wire-alignment/README.md) — the gate texts reused here
- [R07 CONTRIBUTING](../../CONTRIBUTING.md) — types, scopes, the schema line, the schemas-release rule
- [R08 Golden corpus oracle](../../crates/candler/tests/oracle/README.md) — `rates_by_cell.toml`, the second engine behind A4
- [R09 M4 rulings](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m3-coordinator-inputs.md) — the coordinator's inputs: imported strata after M4, the early PR, the overlap rule
- [R10 Approved plan](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m3-approved-plan.md) — §0 rulings; §1 the I22 rule and its safety argument; §2 decisions D1–D10 and D8's statistics; §3 behaviour changes; §5 the guard table (ids S1…U5), probes P1–P6 and landing gates; §6 the execution model

## Goal
Every operation of the API is a verb that derives what it can, works from any directory and refuses with a stable code. A fresh `run` flows into a supervised `evaluate` and an `analyze` whose statistics are checked against known truth. Contract pages tell coding agents what each surface expects. No stored digest moves.

## Pre-conditions
- [x] [nl] Eliott approved the plan, the I22 rule (`>=`, kept as defence in depth, with the parent cross-checked against `/proc/<pid>/status`), decisions D1–D10 and the behaviour changes (2026-11-11)
- [x] [nl] The coordination session's rulings on the overlap with M4 are recorded in R10 §0 (2026-11-11)
- [ ] [cmd] The golden corpus is in place: `test -d /Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`
- [ ] [cmd] Phase 0 is in: `test -f crates/candler/src/commands/preflight.rs && test -f docs/contract/codes.toml && test -f __roadmap__/m3-lean-cli/refusal_scope.tsv`
- [ ] [cmd] STD passes on the Phase 0 tip

## Success Gates
- ⬜ [cmd] G-sweep: S1–S4 pass (`cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_signal)'`), and S3 and S4's real-process row pass through LINUXRUN
- ⬜ [cmd] G-digest: no expected digest is edited: `git diff --numstat a26fe53 -- crates/*/tests/oracle crates/*/tests/vectors ':(exclude)crates/candler-core/tests/vectors/stats' ':(exclude)crates/candler-core/tests/vectors/platform_inventory.toml' | awk '$2!=0{b=1}END{exit b}'`. GOLDEN passes, and so do M2c's G5c digest guards: `cargo nextest run -p candler-schemas --no-tests=fail -E 'binary(guard_cell_digest)' && cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_import_mini)|binary(guard_legacy_mapping)|binary(guard_oracle_mini)|binary(guard_run_context)|binary(guard_engine)|binary(guard_seams)'`
- ⬜ [cmd] G-codes: `codes.toml` only grows: `git diff --numstat a26fe53 -- docs/contract/codes.toml | awk '$2!=0{b=1}END{exit b}'`
- ⬜ [cmd] G-text: the text form of existing CLI outputs is unchanged: `git diff --numstat a26fe53 -- 'crates/candler/tests/guard_cli*.rs' | awk '$2!=0{b=1}END{exit b}'`. Rows may be added. `evaluate`'s expected outputs are the stated exception (R10 §3)
- ⬜ [cmd] G-schemas: exactly one release-triggering commit touches the schemas crate, and it is `feat(schemas)` or, with the unknown-field refusal, `feat(schemas)!`: `test "$(git log --no-merges --format=%s a26fe53..HEAD -- crates/candler-schemas | grep -cE '^(feat|fix|perf|refactor|style|revert)')" -eq 1 && git log --no-merges --format=%s a26fe53..HEAD -- crates/candler-schemas | grep -qE '^feat\(schemas\)!?: '`. DRIFT holds, and so do the helper's models: `(cd python/candler-evaluator && uv run --frozen python scripts/gen_models.py && test -z "$(git status --porcelain -- src)")`
- ⬜ [cmd] G-verbs: `diff <(target/debug/candler --help | awk '/^Commands:/{f=1;next} f&&NF{print $1}' | grep -v '^help$' | sort) <(sort __roadmap__/m3-lean-cli/verbs.txt)`, and no scaffold stub is left: `! git grep -n 'm3 scaffold' -- crates`
- ⬜ [cmd] G-I22: `tr '\n' ' ' < __design__/architecture.md | grep -q 'is not earlier than the supervisor'`, and `! git grep -n 'fn started_after' -- crates`
- ⬜ [cmd] G-platform: guard Q1 passes, and the binary keeps one `cfg(not(unix))`, the one on its `compile_error!`: `test "$(git grep -c 'cfg(not(unix))' -- 'crates/candler/src/**' | awk -F: '{s+=$2}END{print s+0}')" -eq 1`
- ⬜ [cmd] G-quickwins: `cargo nextest run --workspace --no-tests=fail -E "$(cut -f2 __roadmap__/m3-lean-cli/quickwins.tsv | sed 's/.*/test(=&)/' | paste -sd'|' -)"`
- ⬜ [cmd] G-guards: every guard of R10 §5 passes through its id prefix with `--no-tests=fail`, each leaf's filter as its leaf names it
- ⬜ [cmd] G-docs: `CANDLER_BLESS_CONTRACT=1 cargo nextest run -p candler --no-tests=fail -E 'binary(contract_pages)' && test -z "$(git status --porcelain docs/contract)"`, and `(cd python/candler-evaluator && uv run --frozen pytest -q)`
- ⬜ [cmd] G-confirm: `! git grep -n 'fn confirm_' -- ':(glob)crates/**'`
- ⬜ [cmd] STD and LINUX pass on `feat/m3-lean-cli`; SUBJ passes over `a26fe53..feat/m3-lean-cli`
- ⬜ [nl] Each leaf report records the bite of every guard it introduced, and every verifier named in a leaf closed with no open finding
- ⬜ [nl] Each behaviour change of R10 §3 has a ruling from the coordination session before its leaf merges
- ⬜ [cmd] The two early PRs and the milestone PR (`merge(cli): land the lean CLI`) are merged with green CI (`gh pr checks <N>`), and the release shows `schemas@0.7.0`: `git tag --list 'schemas@0.7.0' | grep -q .`

## Gotchas
- **Guard ids and filters.**
  - Guard ids (S1 … U5) are R10 §5's. Each guard's name starts with its id in lowercase (`c2_…`), and S1 and S2 keep their existing names.
  - Every nextest call carries `--no-tests=fail`.
  - Every guard doc has `Guards:`, `Oracle:` and `Bite:` lines.
- **Condition tags.** Every Pre-condition and Success Gate line starts with `[cmd]` (decided by a command's exit status) or `[nl]` (a judgement).
  - A pre-condition checks content, never the ancestry of a moving branch. For example: `git grep -q 'fn started_no_earlier' -- crates/candler-core/src/supervise/signal.rs`, `test -f crates/candler-schemas/src/refusal.rs`.
- **Standard gates**, named in every leaf:
  - **STD**: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`. Implementers run the `-p` forms of their crates; the coordinator runs the workspace form once per merge;
  - **LINUX**: `cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings`;
  - **LINUXRUN**, for the guards that exist only on Linux (S3, S4's real-process row, E3, P1). The coordinator runs it, one filter at a time: `docker run --rm --init -v "$PWD":/w -w /w -v m3-linux-target:/target -e CARGO_TARGET_DIR=/target -e CARGO_BUILD_JOBS=2 m3-linuxrun cargo nextest run -p <crate> --test <binary> --no-tests=fail -E '<filter>'`. `--init` keeps nextest off pid 1 (four `guard_signal` rows assert a parent above 1), and `CARGO_BUILD_JOBS=2` with one test binary keeps the link within the Docker VM's 7.7 GiB (found by `stray_sweep_tick`). The image carries Rust, nextest, `uv` and Python 3.12, so E3's fixture plug-ins run in it;
  - **DRIFT**: `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas -E 'binary(schema_drift)' && test -z "$(git status --porcelain crates/candler-schemas/schema)"`;
  - **SUBJ**: `cog check <base>..HEAD && git log --format=%s <base>..HEAD | awk 'length>72{b=1}END{exit b}'`, `<base>` being the leaf's branch point;
  - **SPAWN**, before any test run that may spawn a process: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)'`;
  - **GOLDEN**: `export CANDLER_GOLDEN_CORPUS="$GOLDEN" && cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_golden)' && cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'binary(guard_oracle_ac)'` (about 60 s), with `$GOLDEN=/Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`. Read it; never write in it;
  - **OWNED**: `test -z "$(git diff --name-only <base>..HEAD | grep -vE '<the leaf's owned globs>')"`.
- **Build footprint (shared machine).**
  - Export `RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/private/tmp/claude-501/m3-target-<leaf>` for every cargo command. Use one target dir per worktree, outside the worktree, and never share one between worktrees: cargo names a member's artifacts by its path relative to the workspace root, so a shared dir links a sibling's code (M2c, R05). sccache shares the compile work;
  - each leaf deletes its target dir before its last report, and a fix round recreates it;
  - at most three leaves build at once. Run `df -h /System/Volumes/Data` before building;
  - the coordinator removes a leaf's worktree once it is merged, and at close-out removes `m3-linuxrun`, `m3-linux-target` and its own target dir, by exact name.
- **Safety.** M2b's rules hold:
  - signals go through the I22 guard only, and containers are destroyed only through `ContainerGuard`;
  - bite tests of stopping or destroying use the recording fakes of `test-support`;
  - never `kill`, `pkill`, `killall`, `docker rm -f $(…)`, `docker … prune`, or a `docker stop`/`rm` of an id the test did not create.
- **Digests.**
  - A digest that moves is a defect to report, never a value to bless. An expected digest is never edited "until green" (G-digest).
  - Before merging a leaf that touches a wire type, the coordinator runs the workspace suite and the digest guards on the rebased leaf (R05).
- **Behaviour changes.** A newly refused input, a removed option or a moved stored digest is a behaviour change. The implementer reports it and never marks it alone. The coordinator sends it to the coordination session for a ruling before the leaf merges (R10 §3).
- **Schema commits.**
  - Only `contract_wire` touches `crates/candler-schemas/`. It does so in exactly one release-triggering commit, `feat(schemas): …`, which carries the DRIFT bless and touches no other crate. The helper's regenerated models go in the next commit.
  - `trace.rs` is left alone: it is M4's. A wire defect found after `contract_wire` lands goes to Eliott before any second schemas commit.
- **Shared constraints** (one owner each; a leaf that needs to change one reports it instead):
  - **Refusal identity** (R10 D2):
    - codes are `<surface>.<reason>`;
    - `side` is one of `plugin`, `suite`, `experiment`, `config`, `environment` and `candler`;
    - `path` is an RFC 6901 pointer into the parsed document, or a file;
    - the trait is `candler_schemas::refusal::Refusal` (`code`, `side`, `path`), and the record is `refusal/1` `{schema, code, side, path?, file?, message, causes[]}`;
    - errors that are not an author's carry the fallback `candler.internal` and sit in `[internal]`.
  - **`docs/contract/codes.toml`** is hand-written and append-only:
    - a code once written is never renamed or removed;
    - each leaf appends only inside its own section, and the sections are pre-created by Phase 0, `[analysis]` included. There is no `[ingest]` section: `RawIngestError`'s codes are `import.*`;
    - only `refusal_late` codes the enums of `jobs`, `evaluation`, `import`, `manifest` and `trace`, including the variants `evaluate_body` and `ingest_inspect` add. Those two leaves write no `codes.toml` row; `analyze` codes its own `ReportError` in `[analysis]`;
    - it is the oracle of C2 and R1.
  - **A new error enum** implements `Refusal` in a `refusal.rs` beside it; schemas impls go in `refusal_impls.rs`.
  - **A new file that calls a unix-only API** (`std::os::unix`, `rustix`, `libc`) gets a row in `crates/candler-core/tests/vectors/platform_inventory.toml`, with its Windows counterpart.
  - **Scaffold names** (Phase 0 creates them; later leaves use them as they are): `JobSpecFile` in `jobs/model.rs` with the arm `Run(JobSpec)`, the `Evaluate(EvaluateSpec)` arm added by `evaluate_body`; the pending lists `PENDING_CWD` and `PENDING_STAMPS` in `crates/candler/tests/guard_cli_surface.rs`, which `cli_surface` empties.
  - **`JobSpec` stays a struct.** `JobSpecFile {Run(JobSpec), Evaluate(EvaluateSpec)}` is read and written at the job-directory boundary only. `JobServices` is not edited: the evaluate body builds its own services (M4 owns it).
  - **Field classes** (R10 D4): `required` and `optional` are derived from the committed schema; `candler` and `free` are declared with `x-candler-class`.
  - **`analyze`** reports whatever `stratum_verdict` answers and never hardcodes how an imported stratum is judged (R09 item 1). Its per-run rows are an open map of recorded fields.
- **M4 overlap** (R09, R10 §0). The milestone that merges to `main` second resolves the conflicts and codes the other's new refusal variants in `codes.toml`. M4 edits:
  - `commands/{run,supervise,import}.rs`. `ingest_inspect` renames `import.rs` with `git mv`, in a commit of its own;
  - `jobs/runtime/**` and `JobServices`;
  - `isolation/`, `runner/`, `frames/`, `manifest/compare.rs` and `import/streams.rs`;
  - `trace.rs`;
  - R01 and `CONTRIBUTING.md` (another section);
  - the driver-traits page, hand-written by M4. The second to merge folds it into `contract_pages.rs`.
- **Merges.**
  - The coordinator lands each leaf with `git merge --no-ff -F <file>` after rebasing it onto the milestone tip, subject `merge(<scope>): land <leaf>`.
  - Scopes:
    - `stray_sweep_tick` → `jobs`, `contract_wire` → `schemas`;
    - `stats` and `analyze` → `analysis`;
    - `evaluate_body` and `evaluate_verb` → `evaluation`;
    - `refusal_core`, `refusal_late`, `observe`, `ingest_inspect` and `cli_surface` → `cli`;
    - `docs_frame` and `docs_refusals` → `contributing`;
    - `platform_seam` → `repo`, `design_record` → `design`.
- **Leaf reports.** Each leaf writes `__reports__/m3-lean-cli/leaves/<leaf>_v0.md` (`writing-reports`, type findings, no code dump). It records the guards and their bites, the probe answers, the deviations and the spec defects found.
- **Owned files.** Each leaf lists its OWNED paths. Implementers never edit:
  - `__design__/` (except `design_record` and `stray_sweep_tick`'s line 408);
  - `__roadmap__/`, `cog.toml`, `.github/` or `Cargo.lock`;
  - another leaf's files.
- **Test design** (`designing-tests`; R10 §5).
  - Every new guard is written first and seen red, and its bite is recorded in the leaf report.
  - Confirms are named `confirm_*` and are culled or promoted before the leaf merges.
  - Probe answers go to the leaf report, and probe scripts are not committed.
  - Tests run the real binary from a temp dir outside the project wherever a verb is exercised (I14).

## Status
```mermaid
graph TD
    stray_sweep_tick[Stray sweep tick]:::done
    contract_wire[Contract wire]:::planned
    stats[Stats]:::planned
    platform_seam[Platform seam]:::done
    verbs[Verbs]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `stray_sweep_tick.md` | 📄 Leaf Task | ✅ Done |
| `contract_wire.md` | 📄 Leaf Task | ⬜ Planned |
| `stats.md` | 📄 Leaf Task | ⬜ Planned |
| `platform_seam.md` | 📄 Leaf Task | ✅ Done |
| `verbs/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
