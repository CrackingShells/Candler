# Level 3 of M6.5: what was found outside its scope (v0)

Date: 2026-11-25

Five findings from M6.5's level 3 leaf (`e2e_guard`) and its fix round. Each one lies outside the leaf, so the leaf did not fix it. Numbering continues from [round 05](05-observation_level2_out_of_scope_v0.md).

---
type: observation
topic: agent_config
spotted-during: implementing and merging `e2e_guard` (merge bfdfa010)
date: 2026-11-25
domain: code
confidence: confirmed
urgency: per item, below
deferred-because: each lies outside the guard leaf's topic
---

## Summary

| # | Observation | Urgency | Where |
|---|---|---|---|
| O11 | A job-start refusal's code reaches no place a consumer reads | medium | `job.ended` payload, `jobs/events.rs` |
| O12 | The parity guard's leftover-container check compares the whole daemon | low | `parity_e2e.rs`, final assertion |
| O13 | The OpenCode plugin API that the hooks rely on is not on the capability page | low | `docs/contract/harness-capabilities.md` |
| O14 | `resolve`'s preflight cannot reach the fake model from a container on macOS | low | resolve preflight, `assets/docker/fakemodel` |
| O15 | Two more timing guards fail under load | low | `guard_supervise`, `guard_wiring` |

## O11. A job-start refusal's code reaches no place a consumer reads

**What was noticed.** Since level 2 moved the harness check to job start (amendment A1), a rejected cell is refused inside the detached supervisor. The refusal is `DriverError::HarnessRejectedInput`, whose code is `experiment.harness_rejected_input`, but the job's `events.jsonl` ends with `job.ended {"reason": "failed"}` and `supervisor.log` carries the message only. `refusal.json` is documented for evaluate jobs only. A consumer, or the guard, can therefore not tell a rejected input from any other failure by its code. The same holds for `jobs.config_check_failed` and `jobs.server_check_failed`. G6 is met on the message; `guard_harness_check_refuses` asserts on it.

**Location map.** `crates/candler-core/src/jobs/events.rs` (`job.ended`); `crates/candler-core/src/jobs/orchestrate/{config,servers}.rs`; `docs/contract/` error-model pages.

**Hand-off questions.** Should `job.ended` carry the refusal's code and pointer, or should run jobs write `refusal.json` as evaluate jobs do? Working theory: one refusal record for every job kind.

## O12. The parity guard's leftover-container check compares the whole daemon

**What was noticed.** `parity_one_installed_server_one_traced_call` ends by comparing `docker ps -aq` before and after. On a daemon shared with other sessions it failed 4 of 5 times in the fix round on containers it did not make; the committed file passed alone. `agent_config_e2e.rs` has `assert_nothing_left`, which compares the guard's own containers only.

**Hand-off questions.** Working theory: the parity guard uses `assert_nothing_left` (or filters by the job's label).

## O13. The OpenCode plugin API is not on the capability page

**What was noticed.** The guard's plugin uses `export default { id, setup(api) }` with `api.tool.hook("execute.before", …)` and `api.session.hook("context", …)`, read from the OpenCode 2.2.7 binary. No repository page names this shape, so a consumer writing `hooks.opencode.plugins` must find it the same way.

## O14. `resolve`'s preflight cannot reach the fake model from a container on macOS

**What was noticed.** The image's `curl` has no CA bundle for the fake model's TLS, and `host.docker.internal` did not resolve in the preflight's container on this Mac. The guard resolves against a one-shot local listener instead. The run itself reaches the fake model through the gateway.

## O15. Two more timing guards fail under load

**What was noticed.** In one full `cargo nextest run --workspace`, `guard_supervise::a_pre_stop_command_leaves_nothing_in_its_group` and `guard_wiring::wiring_stop_execs_in_the_container_with_the_observed_id` failed and passed alone; the next full run passed 767 of 767. Same family as round 05's O10.
