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

---

# Rerun (maintainer's one rerun with the setup fixed)

The sections above are the first attempt and are unchanged. The rerun stopped at its cost cap (.5 of the runner's side) before the first classification and before the gates that need a reader; what was not reached is marked.

## Rerun: F3/F4 (done first, before the consumers)

One real run through `candler run`, harness `opencode` 2.2.7, container isolation, `RiVault/Agentic-Small`, a prompt that asks for three files (`a.txt`, `b.txt`, `notes/c.txt`). Experiment, treatment repository and store: `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/f34/` (`exp.toml`, `skills/`, `store/`). The experiment needed a treatment, `permission_mode`, a `.claude/skills/<name>` tree in the treatment repository (the first refusal named it), and a `model.endpoint` without `/v1` (with it the model-list read answered 404).

| List | Files |
|---|---|
| Written by the agent in its workspace (`runs/<run>/workspace`) | `.opencode/skills/demo/SKILL.md`, `a.txt`, `b.txt`, `notes/c.txt` |
| Stored outputs (`runs/<run>/tree.json`, 3 entries, blobs in `blobs/`) | `a.txt`, `b.txt`, `notes/c.txt` |
| Difference | `.opencode/skills/demo/SKILL.md` only |

The collect rule of the OpenCode driver (`crates/candler-harness-opencode/src/driver.rs:281`) excludes `.opencode/**` and `**/.opencode/**`; the one file is the treatment's skill, placed by Candler and not written by the agent. After that exclusion the lists are equal. **F3/F4: PASS** for OpenCode: nothing the agent wrote was dropped, and nothing was stored that it did not write.

**Measured OpenCode-in-container run:** job 09:32:26.97Z to 09:32:38.50Z, **11.5 s**; `candler run` returned its job id in 2 s. The image build (a new `candler-env` image) had happened at the earlier `resolve` and is not in that figure.

## Rerun: setup

| Item | Value |
|---|---|
| Consumers | A: `RiVault/Reasoning-Medium`; B: `RiVault/Reasoning-Large` with OpenCode `--variant low` |
| Aliases read live | `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/alias-rerun.txt`: unchanged since the first attempt (Reasoning-Large = MiMo-V2.6-Pro-RL, levels low, high, max) |
| Reasoning-Large effort probe | one short call each at low, high and max: all three answered coherently ("360 grams") with reasoning tokens 29, 29 and 42. `low` chosen: coherent and cheaper, and the first attempt's high-effort default ended incoherently. |
| Rustdoc sources removed | `rm -r rustdoc/src` and the `src-files*.js` index, in each consumer. Checked with `find`: 0 directories named `src`, 0 files named `*.rs.html`, 0 files with `.rs` in the name, under each rerun consumer directory. Rendered pages still hold dead links to `src/`; the target files do not exist. |
| Step cap | 400 (`agent.build.steps`) |
| Wall cap | 2400 s for both. Derivation: a consumer needs about 12 runs (2 experiments by 6) at 11.5 s = 138 s; 4 x 138 s = 552 s for Candler's share, plus a margin of 1848 s for the agent's own reading and writing. This is shorter than the first attempt's 5400 s because B's first attempt used 3926 s and produced nothing; the margin is the runner's choice. |
| Isolation of the consumers | each has its own `XDG_STATE_HOME` (Candler's default store) and, for B, its own `XDG_DATA_HOME`: the first launch of B failed in 1 s with OpenCode's "database is locked" because A held the shared OpenCode database (`transcript-b-launchfail.*`; no model call was made, so it is not an attempt) |
| Transcripts | A: `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/rerun/transcript-a.jsonl`; B: `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/rerun/transcript-b.jsonl`; workspaces `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/rerun/consumer-a/`, `consumer-b/`; launcher `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/run_rerun.sh` |

## Rerun: runs

| | A (Reasoning-Medium) | B (Reasoning-Large, low) |
|---|---|---|
| End | finished itself, rc 0, 1421 s (under the cap), 390 steps | killed by the runner at 1480 s (rc 143) because of the rerun's cost cap, not the wall cap; 101 steps |
| Tokens: new input / output / reasoning | 642,865 / 83,590 / 74,569 | 123,089 / 23,072 / 172,817 |
| Tokens: cache read | 49,385,984 | 8,281,664 |
| Reasoning above 0 | yes | yes |
| Step latency | about 3.6 s | about 14.6 s |
| Experiment written and run | yes: 2 arms x 3 replicates, one MCP unit-converter server, OpenCode on `RiVault/Agentic-Small`, job 6/6 completed, 6 sealed runs | no run: it was still planning (plug-in, suites, experiment directories; 3 run directories in its workspace are not candler runs) |
| Snapshot and provenance checked | yes, with `inspect`; its RESULT says the stored catalog snapshot is the resolve-time catalog `{"provider":"candler-image@1","servers":[]}` (empty `servers` although a live MCP tool was used) | no |
| Graded fresh and on disk | yes: 3 pass, 3 fail; the second grading made 0 new requests and gave the same split | no |
| `analyze` | arms valid 3/3, decision keep (its own addition) | no |
| Op API step | not done: no Candler crate to depend on from the directory (`cargo fetch`: no matching package `candler-core`); a sketch only in `lib-attempt/src/main.rs` | not reached |
| Crate source in cargo's dependency directory / any source | none seen in the digest (no path outside its own directory); `rustdoc/src` no longer exists | none seen |
| Candler's own run times | 6 runs in one job; per-run times not extracted (cost cap) | none |

Docker objects the rerun left: see Rerun: cleanup.

## Rerun: S6

A did read the stored snapshot of its runs and reports it as the resolve-time catalog with an empty server list; whether it found it **from documentation alone** was not judged (the transcript was not read past its digest, cost cap). B did not reach the question; its notes say it could not find where the MCP tool-catalog snapshot command lives, and noticed that the preflight catalog is empty for an image with no MCP server.

## Rerun: first classification, before tip, golden

- **Frictions, RiVault first pass:** NOT REACHED (cost cap). The material is ready (`/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/classify-rerun.sh`, `/Users/hacker/Documents/tmp/claude-worktrees/Candler/integrity-scratch/p7/digest-rerun.py`); A's own list in its `RESULT.md` is: no `answer.md` until an input was added, the evaluator protocol without the helper package (inline `protocol` table, `describe` without `lock_digest`, the outcome `record` as a full OutcomeRecord with `evaluator` as `name@version`), the stored run replay under `--force`, and the library redo impossible because no crate exists to depend on. These are A's words, not a classification.
- **Before tip:** unchanged from the first attempt (malformed MCP server PASS; S1 and B1 cited; plug-in path layout and audit-1 not judged: A's evaluator ran, so its transcript can settle the plug-in path layout, but it was not read).
- **Golden:** not rerun, as ruled.

## Cross-repo finding: golden parity (not an M6c regression)

The three `guard_golden_parity` failures of the first attempt come from `candler-plugin/import/htn-results-rules.toml` of htn-creator-eval 0.5.2 lacking `entry_params`, which Candler's importer has required since `7872a22d feat(importer)!`, already on `main`. The plug-in's rules file (or the parity corpus) belongs to the mcp-python-ingestion repository. Nothing is fixed here; `htn-creator-eval@0.5.3` was not tried.

## Rerun: cleanup

- Docker, created by the rerun and removed by name: image `candler-env:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855-src-b062c666af0a-env-e7ca5eaba54a` (made by the F3/F4 run's build; the run's container was removed by Candler).
- Not removable: `candler-gateway:latest` existed before the pass (image `6902eef07d3f`); Candler rebuilt and re-tagged it during the pass (now a different id), so the earlier image is untagged. Not restored.
- Consumer A may have created further Docker objects of its own through its six runs; at the end the image and network lists differ from the "before" lists only by the two lines above, and containers are back to the 3 arangodb ones.
- Removed: the consumers' `.target` directories. Kept for the reader: transcripts, consumer directories, `f34/`.

---

# Reader: finding (1)

Read by the Claude reader from A's store (`.../p7/rerun/consumer-a/state/store2/`) and the tip's code at `aaf08f16`. Nothing was fixed.

**Verdict: confirmed. The empty `servers` list is not a fault of A's setup and not a fault of what the container reports. Candler's catalog step never asks the image about MCP servers: the command it runs prints a fixed empty list.**

| Check | Result |
|---|---|
| a. Digest and blob | Run `7673d039...b-r1-a1` (and the other five runs): `manifest.json` `comparable.snapshots` = `[{"kind":"catalog","digest":"sha256:02083e4fd7977a4a9e9aa1f8ec9f4a23624f73f73dd338a27c8de958edad42d8"}]`. The blob `blobs/02083e4f...42d8` reads `{"provider":"candler-image@1","schema":"tool-catalog-snapshot/1","servers":[]}`. All six runs name the same digest (one resolve per job; it is in `jobs/01a20cf4-.../spec.json`). |
| b. Experiment | `experiment/kitchen.toml` declares `[fixed.mcp.servers.converter]` (`python3 converter/unit-converter.py`) and an `[[inputs]]` entry of kind `converter`, `at = "converter"`. Each run manifest lists `mcp.servers.converter.command` and `.args` as experiment-file values. The OpenCode config Candler wrote (`raw/resolved.jsonl.gz`) holds `mcp.servers.converter` with `codemode: true`. |
| b. Use | The three runs of the metric-prompt arm (`7673d039...`) each call `tools.converter.convert(...)` inside an `execute` call (trace: 2 matching events per run; A's transcript shows 24 such calls in all). The three plain-prompt runs (`a16eaf29...`) call it 0 times, which is the arm's design. The gateway census lists only OpenCode's own tools plus `execute`, because the MCP tool is wrapped by code mode; so the census cannot name the server either. |

## Where the gap is

1. **Capture asks the image nothing about the cell.** `CatalogStep::run` (`crates/candler-core/src/resolve/steps/catalog.rs:73-76`) calls `provider.capture(&image, &Cell::new(&ctx.values))`. `CommandCatalogProvider::capture` (`crates/candler-core/src/runner/evidence/catalog.rs:305-311`) names the cell parameter `_cell` and drops it; `capture_one` (line 285) runs only `self.command.argv` in a control container of the image.
2. **That command is a constant.** `crates/candler/src/commands/preflight.rs:42` defines `CATALOG_PROGRAM` as a `printf` of `"servers":[]` plus the date; line 427 builds the argv `sh -c CATALOG_PROGRAM`; line 433 passes it to `CatalogCommand::new`. Its doc comment (lines 39-41) says it is "the catalog of an image that offers no MCP server". So the stored catalog for any experiment is a snapshot of an empty server list. The server is delivered as an input at run time, not in the image, so even a real listing command run in the image would not see it.
3. **The gap is known and unlisted in the contract.** `__reports__/m7-mcp-server/00-inventory_gotchas_v0.md:163`: "Catalog capture of input-delivered servers is still a Future features item."
4. **No test could see it.** The fake docker of the CLI tests prints the same `servers:[]` (`crates/candler/tests/it/support/cli_run.rs:168`). `guard_catalog_snapshot.rs` checks that the manifest digest names a blob whose bytes hash to it and that `inspect` shows what the fake printed; it never has a server in the catalog. Hash-matching passes on an empty catalog.
5. **What the consumer sees.** `inspect --format text` prints `catalog candler-image@1` only (A, step 408), with no server count. The README promises "per-run tool-catalog snapshots, provenance" (`README.md:7`), and the Tier D grading in the architecture note depends on the catalog listing the servers a run targeted. The snapshot is stored and sealed, and it says nothing about the MCP server.

Cause in plan v2's terms: D1 (a fake and the production constant produce the same state by hand, so the guard agrees with the code on the wrong value). The missing capability itself is class 4 below.

# Reader: classification

Classes follow plan v2: **1** the agent's (documented and reachable; one model hit it); **2** both models missed it (documentation or surface); **3** exists but not documented or not reachable (Candler's; D1 to D5 named where one applies); **4** missing capability (a finding for the maintainer); **H** harness or runner setup, not Candler. Each class 3 and 4 row was checked against `docs/` and the code; a sample of the 1 rows was reread. Rows from the first attempt are kept where the rerun settled them.

| # | Friction | Evidence | Class | Owner |
|---|---|---|---|---|
| 1 | The stored catalog cannot be found from the documentation: no contract page, README line or `docs/` page says where the snapshot lives or how the manifest names it (`docs/contract` has only the `catalog.*` and `runner.catalog_*` refusal codes). A reached it by hashing `blobs/` (step 309-310) and `inspect` (408), after rustdoc pointed at the catalog module (209-212). B asked the question repeatedly (about 6 of its notes) and never reached an answer. | A 209-212, 309, 408; B text notes; `grep catalog` over `docs/` and README | 3 (no D cause; a documentation gap) | Candler |
| 2 | The catalog of a run with an MCP server lists no server (finding (1) above). B also noticed the preflight catalog is empty for an image with no MCP server. | `preflight.rs:42,427,433`; `catalog.rs:305-311`; blob `02083e4f...` | 4 (missing capability), with D1 behind the unseen value | Eliott (maintainer) |
| 3 | `treatment` is `optional` in `experiment-file.md:48-52`, yet `resolve` refuses an experiment with no `treatment.source` ("names no source", A 186, 191); the README (line 66) says a real experiment names a treatment repository. A treatment also needs a skill tree in the repository (runner, F3/F4 pass). | A 185-191; B (looked for whether it is required); `experiment-file.md` | 3 (D5: one input, readers of different strictness) | Candler |
| 4 | `model.endpoint` must be the API root without `/v1`; with it the model-list probe answers 404 and `resolve` fails in `gateway.gateway_token`. `configuration-paths.md:21` says only "the endpoint the gateway forwards model requests to". Hit by A (194), by B, and by the runner's F3/F4 run. | A 194; B text; F3/F4 section | 3 (D5 candidate: the probe appends `/v1`, the doc does not say so) | Candler |
| 5 | The evaluator outcome record's `evaluator` field is typed `NameVersion` (`outcome-record.md:28`) and no page in `docs/` states its text form (`name@version`); a wrong form gave `not_measured` instead of a refusal (A 347-374, then fixed). The `not_measured` record does carry a reason, which `evaluator-helper.md:101` documents for the helper. | A 347-374 and A's RESULT | 3 | Candler |
| 6 | The helper package `candler-evaluator@0.1.0` is documented (`evaluator-helper.md`) and shipped nowhere in the consumer material, so A rebuilt the protocol by hand (inline manifest `protocol`, `describe` without `lock_digest: null`, notification then response). Whether an outside consumer can obtain the package is not settled here: the consumer directory had none, and I did not check a package index. | A RESULT, A 347-374 | 3, reachability to be confirmed | Candler |
| 7 | The operation API cannot be reached by an outside consumer: no crate to depend on (`cargo fetch`: no matching package `candler-core`; a git dependency is out of bounds). A could only sketch `lib-attempt/src/main.rs` from rustdoc. This is **finding (2)**, a consumer finding. | A RESULT "Library redo" | 3 (no D cause: distribution, not code) | Candler |
| 8 | A's first-try refusals, each stating its fix in its own message: `--set run.mode` is not a path (183), `request.treatment` shape (185-186), `model.token_secret` not set (192), absolute path in `mcp.servers.converter.args` (197), input path leaves the experiment directory (228), `candler show <file>` takes no file (400). The codes are in `contract/index.md`. B read the same rules in rustdoc and avoided the two path refusals. | A steps as listed | 1 | Agent (A) |
| 9 | No `answer.md` until `inputs.recipe` was added: A's own experiment did not give the agent the recipe. Then `runner.fact_changed` (272) and replay under `--force` with 0 plans; `--force` is documented as accepting the difference (`contract/index.md:204`). A used a fresh store. | A 272, A RESULT | 1 | Agent (A) |
| 10 | B declared "prompt injection" in tool results and refused to continue several times (fake timestamps, a "Kiro" role, a request to rewrite `NOTES.md`). The word `kiro` occurs once in the whole transcript, in B's own text; none of its tool outputs holds such text. Model fault, not a Candler surface. | `grep -ci kiro transcript-b.jsonl` = 1 and no tool output | 1 (one model only) | Agent (B) |
| 11 | A used about half of its 390 steps reading rustdoc through `pandoc` (steps 15-136) to learn what the contract pages do not state. Taken with row 1, the contract pages are not enough for an outside consumer to run an experiment; this is a signal, not a separate defect. | A step count | folded into rows 1, 3, 4 | - |
| 12 | B stopped at 101 steps, killed at 1480 s by the runner's cost cap; no run, so no run time. | B meta | H | Runner |
| 13 | First attempt: rustdoc source pages let both agents read source; the 150-step cap ended A. Both removed in the rerun. | First attempt, setup faults 1 and 2 | H (both) | Runner |
| 14 | Plug-in path layout (`context.root/path` meeting `outputs/outputs/...`) | `outputs/outputs` occurs 0 times in either transcript; A's evaluator ran and read `answer.md` from its own plug-in with no path complaint recorded | not reached | - |
| 15 | audit-1 (the cell count in `resolve`) | No step of either transcript looks at the count. A's job had 6 cells and 6 runs, but nothing tests the count | not reached | - |

## S6

Neither agent found the stored catalog from the documentation alone. A reached the blob by reading the rustdoc catalog module, listing `blobs/`, and hashing candidates against the digest it had seen in `inspect` output; it then read the blob and reported the empty list as the resolve-time catalog. B did not reach a stored catalog. The first attempt's S6 line stands, and the finding is the same: row 1.

## Count per class

| Class | Rows | Count |
|---|---|---|
| 1 the agent's | 8, 9, 10 | 3 |
| 2 both missed it | none | 0 |
| 3 not documented or not reachable | 1, 3, 4, 5, 6, 7 | 6 |
| 4 missing | 2 | 1 |
| H harness or runner | 12, 13 | 2 |
| Not reached | 14, 15 | 2 |
| Folded | 11 | 1 |

The first pass's rows 1 to 3 are replaced by rows 1 and 13; its class 3 for the catalog rests on the checked absence of any storage description in `docs/` and README, and stands. Note that row 2 was not a friction either agent could have worked around: both declared the MCP server correctly.
