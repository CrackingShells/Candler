# m4-session-runner

## Context
Milestone M4 of `candler-v1`. It runs off `main` at a26fe53 (`v0.6.0`, `schemas@0.6.0`), on `feat/m4-session-runner`. It adds an opt-in session mode, in which the prompt goes through stdin and stdin stays open, so a background job's notification wakes the agent for further turns; headless runs stay the default. It adds OpenCode V2 as a second harness behind the same driver traits. It closes amendment A3 (no harness named in `candler-core`), amendment A4 (imported runs get a stratum verdict) and amendment A5 (a contract page for driver authors).
Depth is ordering:
- **L0, this level,** holds two probes (the OpenCode facts and the Claude Code session facts), the type merges and the imported-strata verdict.
- **L1, `traits/`,** designs the driver traits with both harnesses' facts in hand.
- **L2, `traits/core/`,** moves the harness out of the core and builds the OpenCode driver crate.
- **L3, `traits/core/runs/`,** builds session mode and wires OpenCode's export and stop into the run loop.
- **L4, `…/closeout/`,** adds the session controls, the harness deny-list, the contract page and the R01 record.
- **L5, `…/revalidation/`,** measures the digests and makes the live confirmations.

## Reference Documents
- [R01 Architecture](../../__design__/architecture.md) — §Harness drivers, §OpenCode v2 (second driver, M4), §Session runner (M4), §Run manifest, §Agent environment and secrets (the configuration table), §Process supervision, §Invariants (I1, I3, I7, I9, I12, I20, I22)
- [R02 Meta leaf](../candler-v1/contracts/parity/alignment/surfaces/m4_session_runner.md) — the gates this campaign decides
- [R03 Gap analysis A3](../../__reports__/candler-v1-review/01-gap_analysis_harness_env_v0.md) — the Claude Code content of the core (H1)
- [R04 Gap analysis A4](../../__reports__/m2c-wire-alignment/gap_analysis_imported_strata_v0.md) — why no imported stratum gets a verdict
- [R05 Gap analysis A5](../../__reports__/contract-docs/gap_analysis_contract_docs_v0.md) — the contract pages' frame
- [R06 M2c KT](../../__reports__/m2c-wire-alignment/00-knowledge_transfer_v0.md) — "For M4", the flakes, the lessons
- [R07 Session design](/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion/.claude/skills/htn-creator-workspace/infrastructure/__reports__/waiting_on_processes_eval/01-architecture_eval_sandbox_v0.md) — the waiting-on-processes eval sandbox: end rule, `init_tools`, controls (branch `feat/skill-htn-creator`)
- [R08 CONTRIBUTING](../../CONTRIBUTING.md) — types, scopes, the schemas-release rule
- [R09 Approved plan](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-approved-plan.md) — decisions D1–D12, the guard table, the gate commands, what is deliberately not tested
- [R10 Implementer brief](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-implementer-brief.md) — read order, environment, commit flow, the leaf report
- [R11 M2c campaign](../m2c-wire-alignment/README.md) — the standard gate forms this campaign reuses

## Goal
Agents can run in long-lived sessions whose end Candler records rather than imposes; OpenCode V2 runs through the same driver traits as Claude Code; `candler-core` names no harness; imported strata get a verdict; no cell digest of the golden import, the parity fixtures or the M2c smoke moves.

## Pre-conditions
- [x] [nl] Eliott approved the plan and decisions D1–D12 (2026-11-11)
- [ ] [cmd] The milestone branch starts at `main`: `git merge-base --is-ancestor a26fe53 feat/m4-session-runner`
- [ ] [cmd] The golden corpus is in place: `test -d /Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`
- [x] [cmd] The scope exists before any L0 branch is cut (d6f91f5): `grep -q '"adapter-opencode"' cog.toml && grep -q 'adapter-opencode' CONTRIBUTING.md`
- [x] [nl] The coordination session sequenced the file overlap with M3 and ruled (2026-11-11): D10 approved (`feat(cli)!`), `refactor(runner)!` approved, the contract page written by hand now, the scope landed on this branch; the meta leaf's status is the coordinator's to set, together with M3's
- [x] [cmd] P0 on a26fe53 (2026-11-11): see the Gotchas' P0 line

## Success Gates
- ⬜ [cmd] G1 controls: guards S4 and Q1 pass (S4 in CI's `container-e2e` job, and locally when Docker is up)
- ⬜ [nl] G1 live: one live C1 (the agent waits, a notification wakes it, the session closes) and one live C2 (one answer, the session closes `runner.idle_s` later), on a model Eliott chose, are recorded in `live_confirmation`'s report
- ⬜ [cmd] G2 notification: guards S1 and S4 pass; [nl] the live report records a notification-started second turn
- ⬜ [cmd] G3 `init_tools`: guard H3 and `cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_comparability)'` pass, and S2 asserts `census.init_tools` on session runs
- ⬜ [cmd] G4 imported strata: guards V1 and V2 pass, and `sed -n '/^### Run manifest/,/^### Normalised/p' __design__/architecture.md | grep -q 'import.rules'`
- ⬜ [cmd] G5 contract page: `test -f docs/contract/driver-traits.md`, and its drift guard (or M3's page checks, when they are on `main`) passes
- ⬜ [cmd] G6 OpenCode: guards K1, O1 and W1 pass for `candler-harness-opencode`
- ⬜ [cmd] G7 facts: `s=$(sed -n '/^### OpenCode v2/,/^### Session runner/p' __design__/architecture.md); for k in 'event' 'exit code' 'signal' 'budget' 'binary'; do echo "$s" | grep -qi "$k" || echo "missing $k"; done; ! echo "$s" | grep -qi 'not yet confirmed'` prints nothing
- ⬜ [cmd] G8 A3: guard L1 passes; `! git grep -n -i -E 'claude|anthropic|opencode' -- crates/candler-core/src crates/candler-schemas/src`; `git grep -q -E '\.version_probe\(' -- crates/candler-core/src`; `! git grep -n -E '"subtype"|"init"' -- crates/candler-core/src/runner/evidence`
- ⬜ [cmd] G9 digests: DIGEST passes on the milestone tip, and GOLDEN passes
- ⬜ [nl] G9 measured: `__reports__/m4-session-runner/digest_invariance_v0.md` records 0 moved run keys, cell, context and suite-entry digests on the golden and mini imports, 0 moved M2a fixture digests, 0 moved M2c smoke cell digests, and the recorded Claude Code version string unchanged
- ⬜ [cmd] G10 schemas: exactly one release-triggering commit touches the schemas crate, it is `refactor(schemas)`, and it touches no other crate: `test "$(git log --no-merges --format=%s a26fe53..HEAD -- crates/candler-schemas | grep -cE '^(feat|fix|perf|refactor|style|revert)')" -eq 1 && git log --no-merges --format=%s a26fe53..HEAD -- crates/candler-schemas | grep -qE '^refactor\(schemas\): '`; DRIFT holds
- ⬜ [cmd] STD and LINUX pass on `feat/m4-session-runner`; SUBJ passes over `a26fe53..feat/m4-session-runner`
- ⬜ [cmd] No confirm is left: `! git grep -n 'fn confirm_' -- ':(glob)crates/**'`
- ⬜ [nl] Each leaf report records the bite of every guard it introduced, and every verifier named in a leaf closed with no open finding
- ⬜ [cmd] The PR to `main` is titled `merge(runner): land the session runner and the OpenCode driver` (at most 72 characters with ` (#N)`), and its CI is green: `gh pr checks <N>`

## Gotchas
- **Guard ids and filters.** Guard ids (K1 … L1) are those of R09 §Test design; each is named in the leaf that writes it. Every nextest call carries `--no-tests=fail`. The control tasks are C1 (must wait) and C2 (must stop); verdict rows are C01–C64; neither is a guard id.
- **Condition tags.** Every Pre-condition and Success Gate line starts with `[cmd]` (decided by a command's exit status) or `[nl]` (a judgement).
- **Standard gates**, named in every leaf:
  - **STD**: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`. Implementers run the `-p` forms of their crates; the coordinator runs the workspace form once per merge;
  - **LINUX**: `cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings`;
  - **DRIFT**: `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas -E 'binary(schema_drift)' && test -z "$(git status --porcelain crates/candler-schemas/schema)"`;
  - **SUBJ**: `cog check <base>..HEAD && git log --format=%s <base>..HEAD | awk 'length>72{b=1}END{exit b}'`, `<base>` being the leaf's branch point;
  - **SPAWN**, before any test run that may spawn a process: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_signal)'`;
  - **GOLDEN**: `export CANDLER_GOLDEN_CORPUS="$GOLDEN" && cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'binary(guard_golden)' && cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'binary(guard_oracle_ac)'`, with `$GOLDEN=/Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train`. Read it; never write in it;
  - **DIGEST**: `git diff --numstat a26fe53 -- crates/candler/tests/oracle crates/candler-core/tests/oracle/vectors crates/candler-schemas/tests/vectors ':(exclude)crates/candler-core/tests/oracle/vectors/field_paths.toml' | awk '$2!=0{b=1}END{exit b}' && ! git diff a26fe53 -- crates/candler-core/tests/oracle/vectors/field_paths.toml | grep -q '^-[^-]' && cargo nextest run -p candler-schemas --no-tests=fail -E 'binary(guard_cell_digest)' && cargo nextest run -p candler-core --no-tests=fail -E 'binary(guard_import_mini)|binary(guard_legacy_mapping)|binary(guard_oracle_mini)|binary(guard_run_context)|binary(guard_engine)|binary(guard_seams)'`.
- **P0** (a26fe53, 2026-11-11, target dir `/private/tmp/claude-501/m4-target`): SPAWN 9/9; `cargo nextest run --workspace` 421/421 (19 skipped); DIGEST passes as written (9 + 39); the core's digest, verdict, layering and resolve-surface binaries 52/52; GOLDEN 2/2 + 1/1; LINUX clean; a filter for a guard not yet written (`test(/^imported_stratum_/)`) exits 4 under `--no-tests=fail`; G7, G8 and G10 fail on a26fe53 as they must (G8's grep finds 35 lines).
- **Build footprint (shared machine).**
  - Export `RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/private/tmp/claude-501/m4-target-<leaf>` for every cargo command: one target dir per worktree, outside it, never shared between worktrees (a shared one links a sibling worktree's code). The coordinator's tip runs use `/private/tmp/claude-501/m4-target`, which only it uses.
  - Check `df -h /System/Volumes/Data` before building. At most three leaves build at once.
  - Each leaf deletes its target dir before its last report; a fix round recreates it. The coordinator removes a leaf's worktree, branch and target dir as soon as it is merged.
- **Safety.** M2b's rules hold:
  - signals go through the I22 guard only, and containers are destroyed only through `ContainerGuard`;
  - bite tests of stopping or destroying use the recording fakes of `test-support`;
  - never `kill`, `pkill`, `killall`, `docker rm -f $(…)`, `docker … prune`, or a `docker stop`/`rm` of an id the test did not create;
  - a real-daemon test, probe or e2e removes only the containers, images and networks it created, by exact id; record `docker images -q` before and compare after;
  - if Docker is down, `docker desktop start` and wait for `docker info`; never stop it.
- **Tokens and downloads.** Every RiVault probe or paid run waits for Eliott's go and his model choice, after a RiVault health check: at most 3 runs in `opencode_facts`, 1 optional run in `session_facts`, 5 in `live_confirmation`. Downloading OpenCode V2 waits for his go, with the file, source and size stated. OpenCode is installed only inside a Candler image, never on the host.
- **Digests.**
  - Cell digests come from `comparable`. Nothing M4 adds may enter an existing cell: `runner.mode` has no core default, `comparable.harness.driver` stays `None`, the label key `candler.cc_version` and `assets/docker/base/` stay byte-identical.
  - A digest that moves is a defect to report, never a value to bless. An expected digest is never edited "until green" (DIGEST).
- **Schema commits.** Only `type_merges` touches `crates/candler-schemas/`, in exactly one commit, `refactor(schemas): …`, which touches no other crate and leaves every schema file unchanged (DRIFT clean). A wire defect found later goes to Eliott before any second schemas commit.
- **Behaviour changes.** A newly refused input or a moved stored digest is a behaviour change: the implementer reports it, and the coordinator sends it to the coordination session for a ruling before the leaf merges. Ruled (2026-11-11): D10 (an unknown `request.harness` is refused, `feat(cli)!`, the migration in the body; a silent fallback to Claude Code is a defect) and `refactor(runner)!` (the public `FrameMode` is removed; its body names the migration to `candler_harness::spec::Framing`). Open: D9's census rule for a harness with no tool inventory, decided only if `opencode_facts` finds none, and brought to the coordination session first.
- **Test design** (`designing-tests`; R09 §Test design).
  - Every new guard is written first and seen red. Its doc carries a `Guards:` line and an `Oracle:` line naming an expectation derived without Candler's code.
  - Its bite (the plausible bug that turns it red) is checked against the guard's oracle before it is relied on, and recorded in the leaf report.
  - A guard that reads a sealed run asserts the seal first. A check over a list has at least one non-empty fixture.
  - Confirms are named `confirm_*` and are culled or promoted before the leaf merges.
  - Probe answers go to the leaf report; probe scripts are not committed, but fixtures and the fake model are.
- **Shared names** (one owner each; a leaf that needs to change one reports it instead):
  - the cell values `runner.mode` (`headless` or `session`; absent means headless) and `runner.idle_s` (whole seconds, or `none`; only in session mode; derived as 60 when absent);
  - the measured paths `session.turns`, `session.yielded_at_ms`, `session.pending_at_end`, `session.end` (`closed`, `cap`, `budget`, `harness_exit`), `session.close_forced`, `session.control`;
  - the trait items (owner `driver_traits`): `Capabilities.session`, `GatewayEnv {base_url, auth_token, reserved}`, `StatePath {env, kind: Dir | File, subpath}`, `InitReport {tools, mcp_servers}` in `Observed.init`, `SessionSpec`, `HookSpec`, `PendingState`, `Action::Exec` (a command run inside the agent's isolation) and the placeholder `{session_id}`, the trait `candler_harness::skills::SkillTree`;
  - the registry (owner `harness_out_of_core`): `crates/candler/src/drivers.rs`, `Registry`, `DriverEntry {driver, skills, base_context, reader}`;
  - the fake model (owner `session_facts`): `assets/docker/fakemodel/`, scripts `c1` and `c2`;
  - the control record (owner `session_controls`): `<store>/controls/<base-image-hex>-<control-version>.json`.
- **Merges.** The coordinator lands each leaf with `git merge --no-ff -F <file>` after rebasing it onto the milestone tip, subject `merge(<scope>): land <leaf>`. Scopes: `opencode_facts`, `opencode_driver` → `adapter-opencode`; `imported_strata` → `manifest`; `harness_out_of_core` → `isolation`; `harness_guard` → `repo`; `design_record` → `design`; every other leaf → `runner`. Level pre-conditions key on these subjects, never on the ancestry of the moving milestone branch.
- **Leaf reports.** Each leaf writes `__reports__/m4-session-runner/leaves/<leaf>_v0.md` (`writing-reports`, type findings, no code dump): the guards and their bites, the probe answers, the deviations and the spec defects found.
- **Owned files.**
  - Each leaf lists its OWNED paths. Implementers never edit `__design__/` (except `design_record`), `__roadmap__/`, another leaf's files, `cog.toml`, `.github/`, `.config/` or `Cargo.lock` (except a leaf that adds a crate).
  - Shared hunks, merged by the coordinator: at L3, `crates/candler-core/src/runner/exec/mod.rs` and `crates/candler-core/src/isolation/container/mod.rs` (`session_runner` against `opencode_wiring`).
- **M3 runs in parallel** on `feat/m3-lean-cli`. It owns the contract-docs frame and generator, `analyze` (the verdict's first `src` caller, D8) and the I22 same-tick rule in `supervise/signal.rs`. `guard_job_runtime::b33_setsid_descendant_dies` is a known flake of that rule: never patched here. The coordination session's rulings (2026-11-11):
  - **Rebase point.** This branch sits on `main` at c356a0e (v0.6.1). M3's scaffold landed there as 2126181: `foreground`, `asset_paths`, `request_settings` and `compose_resolver` now live in `crates/candler/src/commands/preflight.rs`, which names `ClaudeCodeDriver` and belongs to `harness_out_of_core`. Every leaf branch is cut from the milestone tip, so it holds c356a0e.
  - **Refusal codes.** M3's `refusal_core` gives every author-facing error variant a stable code (`docs/contract/codes.toml`) and a `Refusal` impl in a `refusal.rs` beside its enum, including in the harness crates. Whichever milestone merges to `main` second codes the other's new variants; the exhaustive matches fail to compile until it does.
  - **Collisions to keep small.** M3 renames `commands/import.rs` to `ingest.rs`: keep edits there inside function bodies. M3's `evaluate_body` may build `JobServices`: a field added there is a semantic clash to report at the merge. M3 owns all of R01 except its line on the same-tick rule; the second milestone to merge rebases its R01 edits. M3's `feat(schemas)` leaves `trace.rs` alone, so the two schemas commits land in either order.
  - **Contract pages.** The driver-traits page is written by hand now, in the frame; the milestone that merges second folds it into M3's generator (`crates/candler/tests/contract_pages.rs`).

## Status
```mermaid
graph TD
    opencode_facts[OpenCode facts]:::planned
    session_facts[Session facts]:::planned
    type_merges[Type merges]:::planned
    imported_strata[Imported strata]:::planned
    traits[Traits]:::planned
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `opencode_facts.md` | 📄 Leaf Task | ⬜ Planned |
| `session_facts.md` | 📄 Leaf Task | ⬜ Planned |
| `type_merges.md` | 📄 Leaf Task | ⬜ Planned |
| `imported_strata.md` | 📄 Leaf Task | ⬜ Planned |
| `traits/` | 📁 Directory | ⬜ Planned |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
