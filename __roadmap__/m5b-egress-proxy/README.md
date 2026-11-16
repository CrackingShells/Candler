# m5b-egress-proxy

## Context
Milestone M5b of `candler-v1`, as cut by amendment A8. It runs off `origin/main` at e05c508 (M5a landed in PR #36), on `feat/m5b-egress-proxy`. M5a refuses `isolation.egress` with `isolation.egress_unsupported`. M5b enforces it, refuses ground truth in agent-bound bytes, shows I8 in a real run, reuses the destroy guard's verification (A1), and closes the leaks of a container that was created but never ran. It keeps the decisions D1, D2, D7 and D9 of the M5 plan; it is not a new design. Depth is ordering:
- **L0, this level:** `guard_reuse` (A1/D7), `egress_proxy` (D1, D2) and `ground_truth_refusal` (D9).
- **L1, `proven/`:** `unstarted_destroy` (B2, B6), `egress_attack` (B3, adversarial verifier), `ground_truth_e2e` (I8, B7, adversarial verifier) and `design_record` (the rest of R01).
Where the work fits, verified against the code. The verb is `candler run`: no `candler create` exists, and R01 §CLI surface lists `run`, `ls`, `ps`, `kill`, `watch`, `ingest`, `resolve`, `inspect`, `analyze`, `evaluate`, `show`, `diff`. M5a's "create" is `docker create`: the agent argv's `run` became `create`, then the record, then `docker start -a` (D8/A8, `container/mod.rs`). The product path M5b touches, from the verb down:
- `commands/run.rs` (`candler run`: resolve, `plan_job`, start the supervisor);
- `resolve/steps/egress.rs` (today's refusal, which M5b removes) and `resolve/steps/ground_truth.rs` (new, D9);
- `jobs/orchestrate/session.rs` (builds `DockerIsolation` once per job after the gateway starts; M5b starts the egress proxies here);
- `DockerIsolation::plan` and `run_request` (`docker create`, record, `docker start -a`; M5b destroys the unstarted container here);
- `container/guard.rs` (A1's one inspect per destroy);
- `runner/exec/mod.rs` (the session-start error path).
No new CLI verb, flag or wire field: `isolation.egress` is already a parsed cell value. Only `egress_attack` merges the removal of the refusal, as its last commit, once the attack rows pass. The coordinator runs the live smoke (B5) once; no leaf does. DIGEST and GOLDEN must not move.
## Reference Documents
- [R01 Architecture](../../__design__/architecture.md) — §Process supervision, §Agent environment and secrets, §Contracts & Invariants (I8), §Future features
- [R02 M5b node](../candler-v1/contracts/parity/alignment/surfaces/isolation/m5b/README.md) — the seven gates this campaign decides (read only)
- [R03 Approved plan](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5b-approved-plan.md) — decisions B1-B7, owned files, guards and bites, probes, gates, budget
- [R04 M5 approved plan](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md) — D1, D2, D7, D9 and the attack lists
- [R05 M5a campaign](../m5-container-isolation/README.md) — the standard gate forms; its moved leaves are the source text: [guard_reuse](../m5-container-isolation/client/seams/guard_reuse.md), [egress_proxy](../m5-container-isolation/client/seams/egress/egress_proxy.md), [ground_truth_refusal](../m5-container-isolation/client/seams/egress/ground_truth_refusal.md), [ground_truth_e2e](../m5-container-isolation/client/seams/egress/proof/ground_truth_e2e.md), [design_record](../m5-container-isolation/client/seams/egress/proof/design_record.md)
- [R06 CONTRIBUTING](../../CONTRIBUTING.md) — types, scopes, the schemas-release rule

## Goal
An agent's container reaches only its listed hosts and ports and the model gateway. A real container is shown never to hold ground truth. The destroy guard verifies a decision with one inspect. A container that was created but never ran is destroyed by its creator. No golden or parity cell digest moves.

## Pre-conditions
- [x] [nl] Eliott approved the plan (2026-11-16; all subagents on Sonnet; cap $52)
- [x] [cmd] The milestone branch starts at e05c508 (cut from 54b83b3, 2026-11-16): `git merge-base --is-ancestor e05c508 feat/m5b-egress-proxy`

## Success Gates
- ⬜ [cmd] G1 egress: `plan_` and `egress_*` pass locally, and in CI's `container-e2e` job; the refusal is gone: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_egress_proxy::plan_/)' && cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*egress_/)' && ! git grep -n egress_unsupported -- ':(glob)crates/*/src/**'`; CI's result is read with `gh pr checks <N>`
- ⬜ [cmd] G2 I8: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_ground_truth_refusal::refuses_/)' && cargo nextest run -p candler --no-tests=fail --run-ignored only -E 'test(/^guard_cli_e2e::.*i8_/)'`
- ⬜ [cmd] G3 A1: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_container_guard::spawn_budget_/)' && cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*destroy_verifies_by_one_inspect/)'`; a stopped 9-container job costs at most 8 spawns
- ⬜ [cmd] G4 no leak: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_unstarted_destroy::/) | test(/^guard_container_stop::/)'`
- ⬜ [cmd] G5 digests: DIGEST and GOLDEN pass, and a cell without egress keeps its argv: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_harness_facts::/)'`
- ⬜ [cmd] G6 R01: `grep -q isolation.ground_truth_exposed docs/contract/codes.toml && grep -q isolation.ground_truth_exposed __design__/architecture.md && grep -A5 'isolation.egress_unsupported' docs/contract/codes.toml | grep -q '^lifted = ' && ! sed -n '/^## Future features/,/^### Execution and isolation/p' __design__/architecture.md | grep -q 'M5b builds'`
- ⬜ [nl] G7 live smoke (B5), run by the coordinator through `candler run` with a RiVault model and a real driver, on a plug-in environment with an MCP server and `isolation.egress = ["pypi.org:443"]`: the agent lists the server's tools, fetches pypi.org and fails on example.com. The run id is recorded here, checkable with `candler show`. The pypi.org index page (a few KB) is the only download
- ⬜ [cmd] G8 hygiene: STD, LINUX and SUBJ pass; every new e2e is in CI's list: `for n in destroy_verifies_by_one_inspect i8_canary_never_reaches_the_agent egress_allowlist_holds_on_a_real_daemon; do grep -q "$n" .github/workflows/container-e2e.yml || exit 1; done`; no schemas release commit: `test -z "$(git log --no-merges --format=%s e05c508..HEAD -- crates/candler-schemas)"`
- ⬜ [nl] G9 reports: every verifier closed with no open finding; every bite is recorded in the leaf reports; confirms are culled (`! git grep -n 'fn confirm_' -- ':(glob)crates/**'`); every probe (P1-CI, P4, P6) is answered in its leaf report

## Gotchas
- **Decisions B1 to B7** (the approved plan's, one line each):
  - **B1.** D1, D2, D7, D9 stand as approved for M5. Why: Nothing was found against them.
  - **B2.** A container created but never run is destroyed by its creator, by the id `docker create` returned, through the guard in `SweepScope::Run`, using A1's single verification. Why: One owner of destroy; the rule already admits it (`guard.rs`).
  - **B3.** The egress attack is one rig: one set of targets, one agent container running every row, each row judged by its target's own connection log. The refusal is removed in the same leaf, as its last commit, once the rows pass. Why: 11 rows for one container set; the refusal stays until enforcement is proven.
  - **B4.** One lock serialises real-daemon test runs; builds use bounded target dirs, one per worktree, with sccache, deleted after each merge. Why: Sweep scopes overlap (R01, "Parallel container e2e"); a target dir shared across worktrees links a sibling worktree's code (`scripts/worktree.py`), so the shared dir of the approved plan is replaced by bounded ones.
  - **B5.** The coordinator runs the live smoke once (G7). No new rig, no kept test. Why: Meets the node's gates 4 and 5 with one run.
  - **B6.** The deadline-races-cancel case is one row in the existing stop-race table. Why: A row, not a test per incident.
  - **B7.** The I8 e2e goes through `candler run` with a plug-in environment of values and agent inputs only, so the cached testagent base serves and no env layer is built. Why: Runs the product path for inputs and builds no image.
- **Budget.** Cap $52; the coordinator stops and reports to Eliott at $42 (80 %). Measured from transcripts at list rates; spend is measured at each level merge. Leaves on Sonnet, verifiers included: $33.5 (guard_reuse 5.0, egress_proxy 6.0, ground_truth_refusal 2.5, unstarted_destroy 3.0, egress_attack 8.5, ground_truth_e2e 7.0, design_record 1.5). A fresh Opus coordinator for each of 2 levels: $7.0. Close (live smoke, final read-only Sonnet verifier, PR, report): $4.5. Contingency (one fix round): $7.0.
- **Shared machine.** Each leaf builds in `CARGO_TARGET_DIR=/private/tmp/claude-501/m5b-target-<leaf>` with `RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0`, exported per command, never in a config file; the coordinator deletes it after the leaf merges, and at most three leaves build at once. Before a build that writes gigabytes: `df -h /System/Volumes/Data`.
- **B4 lock.** macOS has no `flock`: every real-daemon run (`--run-ignored only`, E2E, probes that start containers) is wrapped in `lockf -k /private/tmp/claude-501/m5b-docker.lock <command>`, which waits for the lock. "The B4 flock" in the leaves means this lock.
- **Docker.** Never prune. Remove only what the leaf created, by exact id; record `docker images -q` and `docker ps -aq` before and after. No `docker rm -f $(…)`, no removal by name, label or prefix. If Docker is down, `docker desktop start`; never stop it.
- **CI.** Never cancel a CI run. Push the milestone branch after each level so the cold daemon runs the container e2e early. A probe branch is deleted after its run is read.
- **colgrep.** Index the leaf's worktree and scope every `search` call's `paths` to that worktree path. The search policy denies `grep -r`/`rg`; use `git grep`.
- **Schemas.** No `feat(schemas)` or other schemas release commit in M5b; no leaf touches `crates/candler-schemas/`.
- **Digests.** DIGEST and GOLDEN must not move. A digest that moves is a defect to report, never a value to bless. `crates/candler-core/tests/fixtures/harness_baseline/argv.json` is byte-identical.
- **Behaviour changes stop the implementer**, who reports them to the coordinator for a ruling before the leaf merges: a newly refused input (`ground_truth_refusal`, D9). Already ruled (coordination, 2026-11-16): `egress_attack` lifts `isolation.egress_unsupported`; its row stays, since `codes.toml` is append-only, and gains `lifted = "…"`.
- **Owned files and races.** Each leaf lists its OWNED files; a leaf edits no other leaf's file, nor `__roadmap__/`, `cog.toml`, `.config/` or `Cargo.lock`. The coordinator resolves the append-only hunks: `docs/contract/codes.toml`, the `mod` lines of `tests/it/main.rs` and `guard_container_e2e.rs`, the `resolve/steps/mod.rs` registration. `container/mod.rs` is owned by `egress_proxy` (the network match, L0) and by `unstarted_destroy` (`run_request`, L1): different levels, so no race.
- **Contract pages.** `docs/contract/index.md` is generated from `codes.toml` and `refusal.rs`; `CANDLER_BLESS_CONTRACT=1 cargo nextest run -p candler --no-tests=fail -E 'test(/contract_pages/)'` writes it, and the same run without the variable checks it. A code added or removed changes all three; the leaf that owns the code owns the three.
- **Confirms** are written to drive the work and are culled before merge. Only guards stay.
- **Standard gates**, named in every leaf (the base is e05c508):
  - **STD**: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`; implementers run the `-p` forms; the coordinator runs the workspace form once per merge;
  - **LINUX**: `cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings`;
  - **SUBJ**: `cog check <base>..HEAD && git log --format=%s <base>..HEAD | awk 'length>72{b=1}END{exit b}'`, `<base>` the leaf's branch point;
  - **SPAWN**: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core --no-tests=fail -E 'test(/guard_signal/)'`;
  - **GOLDEN**: `export CANDLER_GOLDEN_CORPUS=/Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train && cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'test(/guard_golden::/)' && cargo nextest run -p candler --run-ignored only --no-tests=fail -E 'test(/guard_oracle_ac/)'` (read it, never write in it);
  - **DIGEST**: `git diff --numstat e05c508 -- crates/candler/tests/oracle crates/candler-core/tests/oracle/vectors crates/candler-schemas/tests/vectors ':(exclude)crates/candler-core/tests/oracle/vectors/field_paths.toml' | awk '$2!=0{b=1}END{exit b}' && cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/guard_cell_digest/)' && cargo nextest run -p candler-core --no-tests=fail -E 'test(/guard_import_mini|guard_legacy_mapping|guard_oracle_mini|guard_run_context|guard_engine|guard_seams/)'`;
  - **E2E**: `cargo nextest run -p candler-core -p candler --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::/) | test(/^guard_cli_e2e::/)'`, with Docker up.
- **Test design.** Each guard is written first and seen red; its doc carries a `Guards:` line and an `Oracle:` line, and its Bite is recorded in the leaf report `__reports__/m5b-egress-proxy/leaves/<leaf>_v0.md` (`writing-reports`, findings type, no code dump). A name filter passes whatever the test asserts; it gates only beside those lines and the recorded bite.
- **Defects.** Verify each claim against the code before changing it. Report a spec defect as a finding; do not build on a false premise.

## Status
```mermaid
graph TD
    guard_reuse[Guard reuse]:::done
    egress_proxy[Egress proxy]:::done
    ground_truth_refusal[Ground-truth refusal]:::done
    proven[Proven]:::done
    classDef done       fill:#166534,color:#bbf7d0
    classDef inprogress fill:#854d0e,color:#fef08a
    classDef planned    fill:#374151,color:#e5e7eb
    classDef amendment  fill:#1e3a5f,color:#bfdbfe
    classDef blocked    fill:#7f1d1d,color:#fecaca
```

## Nodes
| Node | Type | Status |
|:-----|:-----|:-------|
| `guard_reuse.md` | 📄 Leaf Task | ✅ Done |
| `egress_proxy.md` | 📄 Leaf Task | ✅ Done |
| `ground_truth_refusal.md` | 📄 Leaf Task | ✅ Done |
| `proven/` | 📁 Directory | ✅ Done |

## Amendment Log
| ID | Date | Source | Nodes Added | Rationale |
|:---|:-----|:-------|:------------|:----------|
| A1 | 2026-11-16 | L0 coordinator; G3 ruled by the coordination session, placement by the milestone coordinator | none; two gates added to `proven/unstarted_destroy.md` | `guard_reuse` merged with its ladder stop at 12 spawns, not 8: the pre-stop and destroy hooks each take their own list. G3 stays at 8; the destroy hook taking a fresh list and reusing only the pre-stop's verification facts moves to `unstarted_destroy`, which already owns the destroy path at L1. The L0 teardown fix (cac84ff0) makes the guard take a job's allow networks; the lost sweep's allow networks need a new driver call and move with it |

## Progress
| Node | Branch | Commits | Notes |
|:-----|:-------|:--------|:------|
| `ground_truth_refusal.md` | `m5b/ground_truth_refusal` | merge ba55856e | D9 refusal `isolation.ground_truth_exposed` in six places, zero-length inputs skipped; newly refused input ruled by the coordination session before merge; report `__reports__/m5b-egress-proxy/leaves/ground_truth_refusal_v0.md` |
| `guard_reuse.md` | `m5b/guard_reuse` | merge ff927d6a | one inspect per destroy decision; sweep 5, normal exit 8, ladder stop 12 (G3's 8 moved to `unstarted_destroy` by A1); report `__reports__/m5b-egress-proxy/leaves/guard_reuse_v0.md` |
| `egress_proxy.md` | `m5b/egress_proxy` | merge 7897cb7c; fix cac84ff0 | one proxy per allowlist on its allow network, njs filter refusing loopback, link-local and empty upstreams; round 2 dropped the proxy's listed-name aliases (docker DNS resolved them to the proxy itself) for `--add-host` on each agent; cac84ff0 lets the guard tear down allow networks; report `__reports__/m5b-egress-proxy/leaves/egress_proxy_v0.md` |
