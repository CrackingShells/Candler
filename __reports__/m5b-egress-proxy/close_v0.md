# M5b close: the per-cell egress proxy (v0)

Date: 2026-11-17. Branch `feat/m5b-egress-proxy`, tip `3ba9ca28` before this report. Findings report for
Eliott and the PR reviewer. Base `54b83b37`.

## Verdict

All nine gates are met. The final verifier's table (tip b8d601a2) was clean; since then G7 has run live,
and the fixes that followed it (F1, F2, F3) are green on the final tip.

| Check on `3ba9ca28` | Result |
|:--|:--|
| CI `container-e2e`, run 44008540106 | passed |
| `cargo fmt --check` | OK |
| Linux clippy | rc=0 |
| nextest | 707 passed, 31 skipped |
| B5 live smoke re-run | all three outcomes passed |

## Gates

| Gate | Result | Evidence |
|:--|:--|:--|
| G1 egress | met | `egress_unsupported` is gone from `crates/*/src`; the `plan_` and `egress_*` guards pass; the CI e2e job passed (run 44008540106 on the tip) |
| G2 I8 | met | `refuses_ground_truth_in_every_agent_bound_place` (guard_ground_truth_refusal.rs) and `i8_canary_never_reaches_the_agent` (guard_cli_e2e/i8_e2e.rs, in the CI list) |
| G3 A1 | met | `spawn_budget_*` in guard_container_guard.rs; `destroy_verifies_by_one_inspect` in container_stop_e2e.rs and the CI list; the 8-spawn stop moved to `unstarted_destroy` by A1 |
| G4 no leak | met | `guard_unstarted_destroy` (two guards) and the B6 row in guard_container_stop.rs; P6 answered in `unstarted_destroy_v0.md` |
| G5 digests | met | `argv.json` identical to base; no fixture line changed a digest; DIGEST 33/33. Three `guard_golden_parity` rows need `CANDLER_HTN_PLUGIN` and were not run (accepted in the carry) |
| G6 R01 | met | `config.upstream_ca_invalid` and `isolation.ground_truth_exposed` are in codes.toml and architecture.md; the `egress_unsupported` row keeps a `lifted =` note; M5b is not in "Decided and scheduled" |
| G7 live smoke | met | Job `01a1e780-5310-7c3a-9718-216900be5694`, model RiVault Agentic-Medium: the agent listed the MCP server's tools, fetched pypi.org and failed on example.com. Evidence in `live_smoke_v1.md` |
| G8 hygiene | met | `cog check 54b83b37..HEAD` clean; no `candler-schemas` commit; every new e2e is in `container-e2e.yml`; clippy and fmt green on the tip |
| G9 reports | met | every verifier closed; the bites are in the carry notes and leaf reports; no `fn confirm_` under `crates/` |

## Residuals

| # | Residual | Why it is open | Trigger to act |
|:--|:--|:--|:--|
| 1 | F1 origin: the refusal `experiment.absolute_path` (from `Comparable::from_resolved`, `crates/candler-schemas/src/execution.rs:1990`) does not name the plug-in that contributed the value | The fix made the fixture path relative; naming the contributor needs a plug-in origin carried into `Comparable` | A second plug-in-derived value needs its origin |
| 2 | `Surface` (`crates/candler-core/src/experiment/surface.rs`) has no production caller; only the guard `guard_resolve_surface::b19` builds it | Its unread `mcp` field was dropped (214927b9). Decided (Eliott, 2026-11-17): deleted in #41 | The opencode and Claude Code drivers read the same cell key differently: then a typed driver input in `candler-harness` pays for itself |
| 3 | F4: a host the proxy denies is not recorded as a proxy denial. In the smoke the agent saw a DNS failure for example.com. The njs filter refuses with a bare `s.done()` (it does not throw), so the client gets 0 bytes and a close | The refusal is correct and the allowlist holds; only the record is missing. Decided (Eliott, 2026-11-17): deferred; its design and limits are the Proxy denial record row of Future features in `__design__/architecture.md` | An eval that must count or explain denied hosts |
| 4 | The gateway tag `candler-gateway:latest` is not a content key (`SIDECAR_TAG`, `crates/candler-core/src/jobs/orchestrate/session.rs`), and the product never builds it | The tag is a test-rig convention; no product path needs it yet | A product path that builds or pulls the gateway image |
| 5 | Coordinator overhead: at fix level the coordinator cost $7.80 and the six fix agents cost $5.94 (the measured figure is in the cost report) | A long-context coordinator re-reads its history on every call | The next milestone's budget, or a coordinator past 150k average context |
| 6 | Smoke diagnostics: "the skill tree could not be archived" does not name the pathspec it looked for (`.claude/skills/<name>/`) | It cost the smoke one wasted round | The next live smoke, or a user report |

## Row-8 ruling items (no new work)

| Item | Where it is tested |
|:--|:--|
| (a) daemon-floor check for `inhibit_ipv4` | **Missing.** colgrep finds `inhibit_ipv4` only in `docker.rs`: `internal_network_gets_no_host_address` and `egress_network_keeps_its_defaults` (the argv of `network create`). No version or daemon-floor check for it exists under `src` or `tests` |
| (b) row 8's IPv6 and link-local listeners | Tested: `egress_allowlist_holds_on_a_real_daemon`, with `probe_host` and `host_link_local`, in `crates/candler-core/tests/it/guard_container_e2e/egress_e2e.rs`. The link-local target is tried only when the host has a bridge address (Linux), so CI may not have exercised it |
| (c) no-egress case with M5a's shape | Partly. `guard_egress_setting::egress_unset_leaves_the_cell` pins the digest; `guard_egress_proxy::plan_proxy_has_no_listed_alias_and_agents_get_add_host` asserts a cell without an allowlist has no `--add-host`; `argv.json` is identical to base (G5). No test names the network of a no-egress cell |

## Proposed roadmap rows

No node under `__roadmap__/candler-v1/` is edited here. Proposals for Eliott's yes:
- m4 or m3: record F4 (proxy denials) when the runner gains a denial record.
- A leaf for residual 2's decision (read `Surface` or delete it) under the next execution milestone.
- Residual 1 and 4 stay in Future features.
