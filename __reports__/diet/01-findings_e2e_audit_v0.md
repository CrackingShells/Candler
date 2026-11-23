# Findings: the README claims, judged end to end (v0)

Audit of `feat/diet` tip `01f137a1` (S1 merged), 2026-11-24 04:07 to 04:40 JST. Yardstick: the README purpose paragraph at `254ff4a0`, verbatim. Store: `~/candler-diet-audit-store/store` (9.9 MB, not committed). Records beside the commands: [`audit/records/`](audit/records/), commands in [`audit/commands.sh`](audit/commands.sh). `S` below is `~/candler-diet-audit-store/store`.

## Verdicts

| Claim | Verdict | Evidence read |
|:--|:--|:--|
| Experiment matrix | **works**, 3 defects | `S/runs/*/manifest.json` (18 live runs): factor values (`model.id`, `prompt.values.hint`) match `experiment.toml` per cell; 4 cell digests, replicates share one (`3036d0...-r1-a1`, `-r2-a1`, `-r3-a1`); `sources` name `experiment_file` or the deriving step. Defects: D1, D2, D3 below |
| Runners (resume) | **works** | `audit/records/resume_evidence.txt`, `events_*.jsonl`, `ps_*.json`: job `01a209d6` killed with 2 runs in flight, both sealed `integrity invalid, reason killed`, exit 143 (`93cb22...-r3-a1`, `bbc08f...-r3-a1`); resume job `01a209d7` ran exactly 3 runs: the two killed cells as attempt 2 and the one never started as attempt 1. Nothing lost, nothing repeated |
| Isolation | **works** | `audit/records/e2e_guards.txt`: 24 real-Docker guards passed (356.8 s), including `i8_canary_never_reaches_the_agent`, `gateway_e2e::b17_canary_absent`, `egress_e2e`. `canary_scan.txt`: the evaluator-only canary (`S/blobs/3da6f4...`, via oracle input `b97acc...`) is in none of the 324 files under `S/runs/` (trace, raw streams, workspace) |
| Per-run tool-catalog snapshots | **broken** (half) | `S/runs/*/manifest.json` `measured` `census.tools` (source `gateway`) holds `audit_audit_stamp`, the one tool of `inputs/servers/audit_mcp.py`, plus OpenCode's built-ins; `raw/gateway_census.jsonl.gz` agrees. But the manifest's `snapshots` entry is only a digest (`sha256:02083e4f...`); no `tool-catalog-snapshot/1` document exists in `S/blobs/` or `S/assets/` (only `S/jobs/*/spec.json` names the digest). The snapshot cannot be compared with the server's tool list |
| Provenance | **works**, 1 defect | `audit/records/analyze_second.json`: arms per experiment digest, killed attempts counted as `invalid` (`experimenter_ends: {killed: 1}`), the contaminated import run excluded (`input_leak`), no pair across the two experiment digests (`027323` and `9ecba0`). The pair refusal does not name an experiment-digest cause: D4 |
| Evaluation, fresh | **broken** | Every one of 24 `audit-file` outcomes is `fail`, `outputs/answer.txt is not among the artifacts` (`S/outcomes/*/audit-file@0.1.0/*.jsonl`), yet each agent wrote the file: `S/runs/*/workspace/outputs/answer.txt` holds `stamp-7f3a` (11 bytes, 10 for Reasoning-Small) and the trace shows the `audit_audit_stamp` and `shell` calls. `S/runs/*/tree.json` is `entries: []` in all 18 runs and `S/blobs/` holds no answer blob. Cause: `OpenCodeDriver::collect` (`crates/candler-harness-opencode/src/driver.rs`) returns only the session-export command; the Claude Code driver also returns `CollectSpec::file("**/*")` plus exclusions. S1 stores blobs for what the driver collected, and OpenCode collects no file |
| Evaluation, on disk | **works** | `ingest.json` `{runs: 6, records: 136}`; `ingest_vs_oracle.txt`: 136 of the oracle's 138 rows match on (run, id, status), none extra. The two missing rows are `synthetic_contaminated` run-1 A1 and A3: that run's manifest is `integrity invalid, input_leak`, so its rows are excluded by design. The ingested runs' blobs are readable: `audit-second` passed all 6 (`evaluate_second_2.json`: 6 pass) |
| Pluggable evaluators, one record format | **works** | `outcome_schema_check.txt`: 24 `audit-file`, 24 `audit-second`, 136 `htn-creator-eval` records, 0 invalid against `outcome-record.schema.json`. `show_rates_audit-*.json` read both. `show --rates` with no `--evaluator` refuses (`analysis.mixed_evaluator_versions`), by design |
| One operation API; library and lean CLI as thin surfaces | **works**, 1 known defect | `crates/candler/src/commands/*.rs` searched for own output shapes: only `resolve.rs:202 print_failures` builds its own JSON; plus D5 (kill prompt). Not every verb was read line by line (call cap) |
| MCP server | **not built** | M7 |
| Domain plugs in; core names no domain | **works**, partly read | Two non-domain plug-ins (`audit-file`, `audit-second`) ran end to end with no domain code; the existing `guard_layering_i1` renamed-corpus rows were not re-run and the `htn` scan of `crates/*/src` was cut short by the search cap, so the plan's "camel-case hits only" is not re-confirmed |

## What ran

18 live runs on RiVault (4 cells; Reasoning-Small and Instruction-Small; 8 + 3 + 3 + 4), about 1.5 minutes of runner wall time in 5 jobs, no cost recorded (`cost_usd: 0.0`). More than the leaf's 8, for two reasons the leaf could not foresee: the first 8-run job finished in 39 s, before it could be killed (so the kill and resume used `--set replicates=3`, 4 added runs of which 3 started), and the provenance pair needed a second experiment digest (`--set budget.per_run.wall_s=601`, 4 runs). Two plug-in suites ran over all runs, one ingest, the 24 e2e guards once. Server-start check: `job.started` to the first `run.started` took 5.9 s on job `01a209d5` (gateway container 1.5 s after start, first agent container at 5.9 s); `resolve` preflight about 3 s.

## Broken claims and defects, for `audit_fixes`

| # | Claim | Smallest fix | One capped piece? |
|:--|:--|:--|:--|
| B1 | Evaluation, fresh, for OpenCode | `OpenCodeDriver::collect` also returns the file spec the Claude Code driver returns (`**/*` minus the harness's own state and the input mount points), with a guard that evaluates a real OpenCode seal | Yes, one piece (a driver line, one guard seen red) |
| B2 | Tool-catalog snapshot | Store the `tool-catalog-snapshot/1` document as a blob under its digest when `catalog@1` derives it | Yes, with B1 or alone |
| D1 | Matrix | `resolve` prints only the values common to all cells: no cell count, no factors. The leaf's "8 cells" check cannot pass. Print the cells or their count | Yes |
| D2 | Matrix | `resolve` accepts an absolute path in `mcp.servers.*.args`, `run` refuses it (`experiment.absolute_path`). Make `resolve` run the same cell check | Yes |
| D3 | Matrix | `treatment` cannot be a factor (`experiment.reserved_root_misused`, then `entry_repeated`), so skill on/off is a second experiment, not a level. The audit used a prompt-hint factor. Either document it in the experiment-file contract or accept `treatment` levels | Document: yes. Feature: no, Next |
| D4 | Provenance | In the 2 x 2 matrix only the two `model.id` pairs at `hint = Ignore` appear, both `refused` with cause `prompt.values`, although their hints are equal; no `hint` pair and no `model.id` pair at the other hint is listed. The nested factor `prompt.values.hint` is not handled by the pair finder or the diff (`crates/candler-core/src/manifest/compare.rs`, not read to the root). Needs a root-cause read first | Probably; size unknown |
| D5 | Surface | `candler kill <job>` without `--yes` prints `stop them? [y/N]` on stdout and exits 2 when not on a terminal; the first kill attempt of the audit did nothing for that reason | Yes, trivial |

Compat: B1 and B2 add stored bytes and change no digest. D1 and D2 change what `resolve` prints; there are no external users, so no deprecation path is owed.

## Cleanup

Docker images built by the audit and removed by id: `sha256:a28c36d9353b` (env), `sha256:96146bacb2e6` (base), `sha256:5368f9ef1adf` (gateway, tag `candler-gateway:latest`). The e2e run replaced the older `candler-gateway:latest` (`9655d472fe16`), which no longer exists after the rebuild; a later run rebuilds it. No containers left. Kept: the store (evidence), `~/candler-diet-audit-store/{treatment,plugins,project,state}` (staging, small).

## Gates

`cargo fmt --check`, clippy `-D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc`, DIGEST (20 passed) and GOLDEN (10 passed): all green. No product file changed.
