# Level 1 of M6.5: what was found outside its scope (v0)

Date: 2026-11-25

Eight findings from M6.5's level 1 leaves (the probe, `declaration`, `trace_context`) and from their verifiers. Each one lies outside the leaf that found it, so no leaf fixed it. Each section below is one observation, written so that a fresh agent can pick it up cold. The front matter is shared, and the urgency differs per item.

---
type: observation
topic: agent_config
spotted-during: implementing and verifying the M6.5 level 1 leaves (probe_harness_checks, declaration, trace_context)
date: 2026-11-25
domain: code
confidence: confirmed (O1–O5, O7); plausible (O6, O8)
urgency: per item, below
deferred-because: each lies outside the topic of the leaf that met it; the leaf rules forbid fixing it there
---

## Summary

| # | Observation | Urgency | Where |
|---|---|---|---|
| O1 | Every gateway run's log read is capped at 4 MiB | medium | `read_since`, `OUTPUT_LIMIT` |
| O2 | Images are built after an inspect, with no lock, in the fake-model path and in the test helper | medium | `fake_image`, `test_image_as` |
| O3 | Commit `514c7db1` does not build alone | low | history of `feat/m6.5-agent-config` |
| O4 | OpenCode's own check does not see unknown config keys | medium | the probe, E-table |
| O5 | `mcp.servers` names must match the key rule | low (notice) | `CONFIGURATION_PATHS`, `McpServer.name` |
| O6 | A timing guard fails under load | low | `guard_container_stop` |
| O7 | `candler-gateway:latest` is left on the machine, and no code uses it | low | local Docker images |
| O8 | A run's child is sometimes not identified for signalling | medium | `supervise/spawn.rs` |

## O1. The gateway log read is capped at 4 MiB for every run

**What was noticed.** The core reads the sidecar's `docker logs` through `read_since`, capped at `OUTPUT_LIMIT` (4 MiB). The census used to read that log on OpenCode only. Since `trace_context`, every gateway run on both harnesses reads it for `context_start`. One context line measured 15,500 B on Claude Code and 8,943 B on OpenCode, so a long session with many requests nears the cap. Past it, the earliest lines may be the ones dropped, and those are the ones `context_start` needs.

**Location map.**
- `crates/candler-core/src/isolation/gateway.rs` (`census_of`, the reader)
- the `read_since` / `OUTPUT_LIMIT` definition, reached from `census_of`
- `docs/contract/trace-file.md` (the measured line sizes)

**Hand-off questions.**
- Does `read_since` keep the head or the tail when it truncates?
- What log size does a 200-turn session reach?
- Working theory, if any: record the context once in its own stream, or stream the log instead of buffering it.

**Scope boundary.** Do not change the `context_start` event or its schema.

## O2. Images are built after an inspect, with no lock

**What was noticed.** `fake_image` (`crates/candler-core/src/jobs/orchestrate/controls.rs`) and the test helper `test_image_as` (`crates/candler-core/tests/it/support/test_image.rs`) inspect a tag and then build it, with no lock between the two. When two tests in one nextest invocation build the same tag, one of them fails. `ensure_base` (`crates/candler-core/src/isolation/image.rs`) takes `lock_tag(&tag)` first and does not race. The race predates M6.5: `git log -S"Built::Found"` gives 1c009bee, and `git log -S"async fn fake_image"` gives dd4f3308.

**Re-observation.** Run `controls_e2e` and the `trace_context` guards in one `cargo nextest run --run-ignored all` invocation.

**Hand-off questions.**
- Can both sites reuse `lock_tag`?
- Is the production `fake_image` reached concurrently in real use, or only in tests?

**Scope boundary.** Do not change image tagging (`sidecar_tag`, `base_tag`).

## O3. Commit `514c7db1` does not build alone

**What was noticed.** `514c7db1 feat(schemas): show the instructions and hooks of an inspected run` fails `cargo check --workspace --all-targets` with E0063: the fields `hooks` and `instructions` are missing in `InspectRun` at `crates/candler/src/commands/inspect.rs:278`. The next commit, `ea23a16e`, builds. History is never rewritten here, so the hazard stays, and `git bisect` must skip this commit.

**Hand-off questions.**
- Should the commit rules (AGENTS.md) require a schema commit's consumers to compile at that commit? Such a commit may touch only `candler-schemas`.

**Scope boundary.** Do not rewrite history.

## O4. OpenCode's own check does not see unknown config keys

**What was noticed.** The probe ran `opencode run --standalone --print-logs -m probe/none x`. It warns on bad values, but it says nothing about a key it does not know. So the harness check that L2 adds for OpenCode refuses errors and warnings, but lets a misspelled key through silently. Claude Code's `claude doctor` reports `Invalid settings`.

**Location map.** `__reports__/agent_config/03-findings_probe_harness_checks_v0.md`, the evidence table.

**Hand-off questions.**
- Does OpenCode publish its config schema, so Candler could validate against it?
- Is that worth doing, or is it OpenCode's defect to report upstream?

**Scope boundary.** Do not change `preflight_check`'s refusal rule (errors and warnings).

## O5. `mcp.servers` names must match the key rule (notice)

**What was noticed.** `hooks` and `instructions.global` now take free keys on the experiment surface. `mcp.servers` still does not, so a server cannot be named `my-server`; this is why `McpServer` carries a `name` leaf. The free-key flag on `TABLE_VALUED_PATHS` (`crates/candler-schemas/src/execution.rs`) would lift the limit.

**Hand-off questions.**
- Does any consumer want hyphenated server names?
- Would the change alter the digest of existing experiments? (There are no external users.)

**Scope boundary.** Do not touch `hooks` or `instructions`.

## O6. A timing guard fails under load

**What was noticed.** `guard_container_stop::stop_asks_the_daemon_and_falls_back_to_the_client` failed once in a full workspace run on a loaded machine (257 s) and passed alone. A c1 timing assertion in `controls_e2e` also failed once under the same load.

**Hand-off questions.**
- Which deadline does each test assert?
- Is that deadline wall-clock where it should be relative?

**Scope boundary.** Observation only; no fix is scoped.

## O7. `candler-gateway:latest` is left on the machine

**What was noticed.** The gateway sidecar is now tagged `candler-gateway:src-<12 digits>`, from its build context. The local image `candler-gateway:latest` (`17f734223887`, created 2026-11-17, by an earlier milestone) is used by no code. M6.5 did not create it, so it did not delete it.

**Hand-off questions.**
- Does Eliott want it removed?

**Scope boundary.** Never prune Docker. Remove this one image by name only on Eliott's word.

## O8. A run's child is sometimes not identified for signalling

**What was noticed.** On the merged L1 tip, in a full workspace run, `guard_known_truth::kt_two_levels_end_to_end` failed once. Of its 48 runs, 47 ran; run `246ad5d5…-r2-a1` failed with "cannot read the start time of the child: the child could not be identified for signalling". The test passed three times alone, and the next full run passed 743 of 743. No M6.5 leaf touches the file that raises the error.

**Location map.**
- `crates/candler-core/src/supervise/spawn.rs` (the error text)
- `crates/candler-core/tests/it/guard_known_truth.rs:594` (the assertion)

**Hand-off questions.**
- Does the supervisor read the child's start time after the child may already have exited?
- Is the failure a lost run in real use, or only in tests?
- Working theory, if any: a race between a fast child exit and the start-time read, under load.

**Scope boundary.** Do not change the known-truth guards' statistics.
