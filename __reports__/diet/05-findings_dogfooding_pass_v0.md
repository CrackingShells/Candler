# Findings: the dogfooding pass, runner's record (v0)

Tip: `origin/feat/integrity` at `aaf08f16`. Written by the runner; the Claude reader's check of the classification and the knowledge transfer follow. **Verdict of the pass: it did not produce its signal.** Neither agent ran an experiment, so steps 1 to 5 of the task are not done by either, and F3/F4 has no real OpenCode run to read. Two setup faults of the runner's are named below (cap too tight; rustdoc carries source pages). One attempt each was the rule, so nothing was rerun.

## Setup

| Item | Value |
|---|---|
| Consumer A | `RiVault/Reasoning-Medium` (DeepSeek-V4-Flash, live 2026-11-24), OpenCode 1.18.23 |
| Consumer B | `RiVault/Reasoning-Large` (XiaomiMiMo MiMo-V2.6-Pro-RL), OpenCode 1.18.23 |
| Model under test in the brief | `RiVault/Agentic-Small` (Qwen3.6-35B-A3B) |
| Caps | 150 steps (OpenCode `agent.build.steps`), 5400 s wall (`timeout`). Set by the runner: P1's probe shows a scripted native run costs about 4.5 s, so 12 runs would cost about a minute of Candler time; the step cap was sized for that and not for reading the documentation. |
| VPN and keys | `rv_health.py`: network, llm, embedding, audio, ocr, jev all ok |
| Binary | `cargo install --path crates/candler` from the tip, `candler 0.15.0` |
| Consumer material | `bin/candler`, `README.md`, `docs/`, `rustdoc/` (from `cargo doc --workspace --no-deps`), `TASK.md` (the brief), `opencode-rivault.json` |
| Scratch root | `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7` |
| Brief | `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/task-brief.md` (copied to each consumer as `TASK.md`) |
| Transcripts | A: `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/transcript-a.jsonl`; B: `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/transcript-b.jsonl` (OpenCode `--format json`); launcher `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/run_consumer.sh`; timings `transcript-*.jsonl.meta` |
| Workspaces / stores | `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/consumer-a/`, `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/consumer-b/` (`store/` in B is empty; neither ran `candler run`) |
| Digest and classification files | `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/digest.py`, `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/classify.sh`, `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/classify-prompt.txt`, `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/classify-output.md` |

Setup faults found after the runs (both mine, both visible in the transcripts):
1. **The rendered rustdoc contains source pages** (`rustdoc/src/<crate>/<file>.rs.html`: `cargo doc` writes them by default). Both agents read Candler's source through them (A step 15 on, with a helper that strips tags; B wrote `tools/rustdoc_extract.py` that turns them back into plain Rust and had subagents map the code). This breaks "no source tree". It is not "crate source in cargo's dependency directory": a regex over every tool input for `.cargo/registry|.cargo/git` or any path outside the consumer directory finds none for either agent.
2. **The 150-step cap was too tight**: A used all 150 steps in 209 s and wrote only `NOTES.md` and a helper. B stopped at 102 steps (rc 0, 3926 s) with incoherent final text (e.g. "Resuming the task. I was at: pinned down the catalog/DNS point (MiTM ...") and no experiment.

A rerun needs: `rm -r rustdoc/src` (or `--no-source`-style rustdoc), a step cap above 150 and a wall cap from a measured real OpenCode-in-container run. That is the lead's call.

## Runs

| | A (Reasoning-Medium) | B (Reasoning-Large) |
|---|---|---|
| Wall (launcher) | 209 s | 3926 s |
| Model steps | 150 (cap reached) | 102 |
| Tokens: new input / output / reasoning | 253,757 / 16,552 / 7,046 | 215,020 / 22,848 / 443,140 |
| Tokens: cache read | 19,880,192 | 12,861,072 |
| Reasoning above 0 | yes | yes |
| Per-step latency | about 1.4 s (209 s / 150) | about 38 s (3926 s / 102) |
| `candler` calls | `--version`, `--help` of run, resolve, ls, evaluate, inspect | `--help` of run, evaluate, resolve, inspect, show, ls |
| Candler's own run times | none: no `candler run` | none |
| Opened crate source in cargo's dependency directory | no | no |
| Read Candler source through `rustdoc/src` | yes | yes |
| Files left | `NOTES.md`, `tools/rd.sh` | `NOTES.md`, derived Rust extracts, empty `store/`, several planning directories |
| `RESULT.md` | none | none |

Token counts are the sum of OpenCode `step-finish` records. No USD: RiVault tokens only. Docker: no container, image or network appeared during the pass.

## S6

Neither agent found the stored catalog from the documentation alone. Both reached the format (`tool-catalog-snapshot/1`) only by reading source through `rustdoc/src`; neither reached a stored catalog because neither ran anything. No contract page documents it, as seam review 2 found.

## F3/F4

**BLOCKED.** The gate compares the stored outputs of a real OpenCode run of the pass with the files the agent wrote in its workspace. The pass produced no OpenCode run, so there are no two lists and no difference. The conformance revert is the only evidence for the producer side (see `collect_oracle_v0.md`).

## Golden

Command: `cargo nextest run --workspace --run-ignored all -E 'test(/golden/)'` with `CANDLER_GOLDEN_CORPUS=/Users/hacker/Documents/tmp/candler-m1-golden/campaign/20260806-cont-train` (the path every M1 to M5 roadmap README gives; `guard_analyze_golden.rs` and `candler-core/README.md` name the variable and no path) and `CANDLER_HTN_PLUGIN` (named by `guard_golden_parity.rs`). Log: `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/golden2.log`.

**Verdict: FAIL.** 18 tests run: 15 passed, 3 failed, none skipped. Passed: the five `outcome_golden` guards of candler-schemas, `guard_golden::the_golden_corpus_imports_to_the_oracle_rows` and `..._runs`, the three `guard_analyze_golden` tests, the three `guard_golden_design` tests. Failed, all in `guard_golden_parity`: `the_second_regrade_sends_new_requests_and_changes_nothing`, `the_live_ac_rows_and_rates_equal_the_reference`, `the_version_diff_compares_everything_it_can_and_excuses_only_the_named_rows`. Cause: the fixture build's import is refused, `config.import_rules_invalid`, `candler-plugin/import/htn-results-rules.toml`: "missing field `entry_params`". The plug-in used is the `htn-creator-eval@0.5.2` tree exported with `git archive` from the mcp-python-ingestion repo into a throwaway git repository, because the guard asks git for the tracked files (a first attempt without a repository failed with "git listed ..."). The guard names 0.5.2; `0.5.3` exists (`cdc067fb`, "tool-catalog-snapshot/1") and was not tried. Whether the fault is the plug-in version or the tip's rules schema is not settled here.

## Before tip

| Cause | Observation | Verdict |
|---|---|---|
| Malformed MCP server (D5) | Tip's `guard_cli_observe::o1_resolve_and_run_refuse_a_malformed_mcp_server_with_one_record` copied into a detached `origin/main` tree (`c9572e5b`) with its `support/cli_run.rs`: FAIL at `guard_cli_observe.rs:377`, `resolve` exits 0 where 2 is expected. The same test on the tip: PASS (1.6 s). Logs: `before-tip-mcp-main.log`, `before-tip-mcp-tip.log`. | cause gone: PASS |
| S1 revert | Not re-run. `sealed_run_producer_v0.md` §Bites already shows it on the base and tip (5 guards fail on the tip, output guards green on the base). | cited, not re-observed |
| B1 revert | Not re-run. `collect_oracle_v0.md` §Bites. | cited, not re-observed |
| Plug-in path layout (`context.root/path` meeting `outputs/outputs/...`) | No transcript reaches it: no run, no evaluator. | not judged |
| audit-1 (cell count in `resolve`) | No transcript reaches it. | not judged |

## Frictions

First pass only, by `RiVault/Reasoning-Medium` (prompt 64,723 + completion 13,578 tokens, reasoning 13,230, finish stop), with plan v2's table, from digests of both transcripts. Output kept as it returned (`classify-output.md`). A first call (effort high, 12,000 tokens) used all its tokens in reasoning and returned nothing (`classify-output-1-empty.md`). Not yet checked by the reader.

| # | friction | model A | model B | evidence (step numbers) | class (1-4 or harness) | one-line reason |
|---|---|---|---|---|---|---|
| 1 | Where each run stores its tool-catalog snapshot (and catalog digest) is not documented in any contract page. | needed | needed | A [105–112,116–117]; B [25,67] | 3 | The catalog and `snapshots` digest exist in `catalog.rs` / `store/layout.rs`, but no `docs/contract` page names the storage path; agents could only find it via rustdoc source. |
| 2 | Rustdoc includes rendered source pages (`rustdoc/src/`), even though the brief says “there is no source tree.” | used heavily | used heavily | A [15–65,105–148]; B [16–33] | harness | Not a Candler defect: the supplied surface contradicts the brief’s no-source-tree constraint. |
| 3 | Agent step cap (150 steps) ended model A’s session before any experiment artifacts were written. | hit | no | A [149] | harness | Harness-side limit, not Candler; A stopped after source exploration only. |

Caveats: Both agents stopped before actually completing a `candler run`/`evaluate` flow, so the table records discovery-time frictions, not execution failures. The “catalog not documented” row is based on the absence of a contract page and on the agents reaching the information only through rustdoc source views. The two harness rows are not Candler issues; they are artifacts of the evaluation setup.

Runner's note on that first pass: row 3 says B did not hit the step cap, which is right (B ended at 102 steps with garbled text). Rows 2 and 3 are setup faults (mine), not frictions of Candler. The model's class 3 for row 1 rests on the absence of a contract page for the stored catalog; the reader should check whether `docs/` mentions it anywhere else.

## Cleanup

- Docker images, containers, networks created by the pass: none (before and after listings in `docker-*-before.txt` match). Nothing removed, nothing stopped, nothing pruned.
- Removed at the end: `integrity-target/p7`, `integrity-target/p7-main`, the `main-tree` detached worktree (`scratch/p7/main-tree`), `consumer-a/.target`, `consumer-b/.target` if present.
- Kept in scratch for the reader: transcripts, consumer directories, classification files, `golden*.log`, `htn-0.5.2/`, `main-bin/`.
