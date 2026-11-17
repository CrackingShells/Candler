# egress_attack leaf, steps 1 to 3 (findings)

## Probe P1-CI
CI run 43895495477 (workflow_dispatch on the scratch branch `probe/m5b-p1-ci`, since a push alone does not trigger container-e2e; the branch is deleted, remote and local).
A busybox testagent container on `docker network create --internal`, running `nslookup example.com`:
`Server: 127.0.0.11 ... ** server can't find example.com: SERVFAIL`, exit 1. The runner's host resolver is 168.63.129.16, so the answer is the same as locally (M5a).
Decision: an internal network does not resolve external names on CI. The agent's create gets no `--dns` flag; `container/run.rs` is untouched.

## Result (steps 1 to 3 only, PARTIAL: step 4 is not mine this round)
Rig `guard_container_e2e::egress_e2e::egress_allowlist_holds_on_a_real_daemon`: PASS, 12.5 s wall (14 s with the cargo test harness; the job's 20 minute timeout is unchanged).
Observed target logs (bytes received, [allowed, a, b, denied, loopback/link-local]): row 1 plain [37,0,0,0,0]; 1 SNI a [0,67,0,0,0]; 1 SNI b [0,0,67,0,0]; row 2 gateway answered `HTTP/1.1 200 OK`; rows 3 to 9 and 11 all zero in every target, row 8 host listener 0 bytes; row 10 warm (decoy) [0,0,0,37,0], then after the resolver flips rebind.test to 127.0.0.1 plus loop.test and ll.test: all zero. 2b holds: no listed name has an alias on a proxy network; the rig's resolver (via `--dns` on the proxy's create) answers them.
Bites (env `EGRESS_RIG_BITE`): agent_on_egress RED (row 5: sinks 0, 1, 3 received 37/37/74 bytes); pinned (`valid=3600s`) RED (row 10: decoy received 37 bytes after the flip); no_filter did NOT go red (see findings).
## Findings
1. no_filter bite stays green: with an egress.js that forwards every byte, the loopback and link-local sinks still log 0 bytes. Either the sinks in the proxy's namespace are not reached (no positive control exists for them) or nginx refuses loopback upstream by itself. Row 10 stays green for the right reason only if the sinks are reachable; this needs a positive control before the verifier. Not resolved (turn cap).
2. Rig race: gateway `start` resolves `upstream.test` at nginx start; a fake that joins late gives `NotReady`. The rig polls the network every 10 ms. gateway_e2e has the same race.
3. Deviations: row 2 uses the python TLS fake of `support::gateway` (as gateway_e2e), not `assets/docker/fakemodel`; the proxy is made from core's `egress_create_args` plus `--dns <resolver>` and bite mounts, not `start_proxy`, so the rig can aim the proxy's resolver. Row 6 has no IPv6 target (Docker nets here have no v6): it only shows nothing listening answers. Bite switch lives in the test as an env var.
4. Residuals recorded, not fixed: the TCP connect to loopback/link-local still happens (CONN is logged, RECV is zero); no post-start readiness probe; ssl_preread stalls server-first protocols; the proxy is not restarted inside a job; `host.docker.internal` (row 8) reached nothing.
5. The 3 expected names are in container-e2e.yml; `i8_canary_never_reaches_the_agent` lands from ground_truth_e2e. Not done: SPAWN, STD, LINUX, DIGEST and SUBJ gates, a cull check, clippy of the new file (one compiler warning in the `it` test binary not inspected), step 4.
## Docker
Before and after: containers equal, networks equal apart from one network that is not mine; images: the two `candler-test-*` images `TestImage` built were removed by id. Two failed bite runs (the race) left two exited gateway containers and four networks, removed by exact id.

## Fix round 1

Row 10 was vacuous. Cause: the proxy listens on 0.0.0.0 on every listed port, so a sink sharing its namespace could not bind 127.0.0.1:7001/7003 (sink log: OSError 98 Address already in use) and nginx connecting to 127.0.0.1 reaches only itself. The link-local sink at 169.254.169.254 on lo was likewise never reached (local sink received 0 bytes under no_filter).
Fix: a rig network on 169.254.231.0/24 (--internal, made and removed by id) that the proxy joins; the sink on it listens on 7002 and 7003; the resolver answers ll.test with it, and rebind.test is flipped to it. Docker accepted the link-local subnet. loop.test (127.0.0.1) has no sink: a loopback sink cannot share the listed port with the proxy, so it is held by the proxy-side refusal only and the row proves nothing about loopback bytes (recorded limit). Pass condition unchanged: zero bytes in the sinks. The one compiler warning (dropping_copy_types on drop(row)) is removed.

- rig clean: test guard_container_e2e::egress_e2e::egress_allowlist_holds_on_a_real_daemon ... ok

- rig agent_on_egress: test guard_container_e2e::egress_e2e::egress_allowlist_holds_on_a_real_daemon ... FAILED
    the allowlist leaked:
    row 5 an IPv4 literal: sink 0 received 37 bytes, expected 0
    row 5 an IPv4 literal: sink 1 received 37 bytes, expected 0
    row 5 an IPv4 literal: sink 3 received 74 bytes, expected 0
    row 8: the host listener received 37 bytes
    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
    test guard_container_e2e::egress_e2e::egress_allowlist_holds_on_a_real_daemon ... FAILED

- rig pinned: test guard_container_e2e::egress_e2e::egress_allowlist_holds_on_a_real_daemon ... FAILED
    the allowlist leaked:
    row 10 loopback, link-local and rebound to link-local: sink 3 received 37 bytes, expected 0
    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
    test guard_container_e2e::egress_e2e::egress_allowlist_holds_on_a_real_daemon ... FAILED
    
    failures:
    

- rig no_filter: test guard_container_e2e::egress_e2e::egress_allowlist_holds_on_a_real_daemon ... FAILED
    the allowlist leaked:
    row 10 loopback, link-local and rebound to link-local: sink 4 received 74 bytes, expected 0
    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
    test guard_container_e2e::egress_e2e::egress_allowlist_holds_on_a_real_daemon ... FAILED
    
    failures:
    

## Fix round 2 (verifier of the round-1 rig: 1 blocker, 7 should-fix)

Fixed:
- B1 (blocker, product): `egress.js` read only the first entry of `$upstream_addr`, and nginx retries the next resolved address by default, so a listed name answering [a refusing address, a link-local address] reached the link-local sink. The filter now refuses when any entry of the comma list is refused, and the stream block sets `proxy_next_upstream off;` (commit `fix(isolation): refuse every address nginx tries for an upstream`). New row `10b`: `multi.test:7004` answers [an egress-network address with no listener, the link-local sink]; eight attempts; every sink must receive 0 bytes. Bite `EGRESS_RIG_BITE=first_entry_retry` (the old filter plus the retry default) is red: `row 10b [refusing address, link-local sink]: sink 4 received 148 bytes, expected 0`. Clean: all zero.
- S1: the `denied` sink carries the aliases `denied.test` and `external.test` on the egress network, so rows 4, 5, 7 and 8 go red under `agent_on_egress`: row 4 sink 3 37 bytes; row 5 sinks 0, 1 and 3 37, 37 and 74 bytes; row 7 sink 3 37 bytes; row 8 the host listener 37 bytes (rows 4 and 7 were green before).
- S2: row 9 also sends SNI `rebind.test`, `external.test` and `allowed.test` to `a.test:9443`; sink 3 listens on 9443 and stays at 0.
- S4: `low.test:443` is listed; new must-succeed row "1 a port below 1024 under cap-drop" delivers 37 bytes to its sink, so the proxy binds 443 as a non-root user without capabilities.
- S7: every container and network the rig makes carries `candler.rig=<job>`; the sweep (also on panic) lists by that label and by `candler.job=<job>` and removes by id; the run asserts that nothing carrying either label remains.
- S3: the agent's second network now comes from the product's `GatewayHandle::agent_networks`; the clean run asserts it equals the job network and the allow network, and `agent_on_egress` asks it for a `bridge` cell (the agent keeps its allow network to reach the proxy), so the bite flips product code. Row 11 stays green under the bite: the gateway's `proxy_pass` is static, which is the design (holds by construction).

Residuals, accepted: S5 (row 8 is meaningful on Linux CI only), S6 (no second allow network), R1 to R6 as in the verifier's notes. `multi.test` relies on nginx rotating its resolved addresses, hence eight attempts; the bite is red in the run above.

Gates: rig clean 15.2 s wall (18.4 s test), `egress_proxy_never_resolves_a_listed_host_to_itself` green; the gateway and egress guards 19 of 19; Docker before and after: images and containers equal, no network of the rig left (the one network id that changed is the daemon's default `bridge`, not the rig's).

## Step 4 (lift the refusal, restore the egress rows)

Commit `feat(isolation): enforce isolation.egress and lift its refusal`.

Lifted and deleted:
- `isolation.egress_unsupported` is no longer emitted. `CHECK_UNSUPPORTED`, `refuse_until_proxy` and `EgressRefusal::Unsupported` are gone from `resolve/steps/egress.rs` (with the module doc's "Nothing enforces an allowlist yet" paragraph); its row is gone from `resolve/refusal.rs`. The experiment-wide step now runs `settle` alone.
- One caller outside the leaf's named files: `runner/plan.rs` (the per-cell call) wrapped `settle` in `refuse_until_proxy`; the wrapper is removed there, nothing else changed. The leaf says it touches no other `src/` file; the deletion cannot compile without this edit.
- `docs/contract/codes.toml`: the `[resolve."isolation.egress_unsupported"]` row keeps `side`, `enum`, `variant` and gains `lifted = "`isolation.egress` is accepted and enforced by the egress proxy."`; the header documents `lifted` beside `retired`.
- `contract_pages/refusals.rs`: `retired_entries` reads either key; a lifted row renders `Lifted, no refusal emits it: <sentence>`; a row with both keys, or an empty value, is an error. `docs/contract/index.md` re-blessed with `CANDLER_BLESS_CONTRACT=1` (one line changed, the code's description).
- One file the leaf does not name: `tests/it/guard_refusal_codes.rs` (R1) demanded that every live row of `codes.toml` is raised by a variant, so the lifted row failed it. It now reads `lifted`, excludes a lifted row from the live rows, fails when a refusal still emits a retired or lifted code, and fails on a row with both keys or an empty value.

Restored rows (all in `guard_egress_setting.rs` and `guard_plugin_environment.rs`): `egress_refused_at_resolve` is `egress_checked_at_resolve` (a bad list refused, a well-formed list resolves, held as written); `egress_refused_per_cell` is `egress_checked_per_cell` (a factor level's list plans as the canonical list `["a.org:443","pypi.org:443"]`, same digest for the unsorted and the sorted spelling, digest pinned as `LISTED_DIGEST`, another list another digest); `layer_lists_append` asserts the plug-in's list alone (`a.com b.com`) and beside the experiment's (`a.com b.com c.com`) as cell values; `layer_egress_merge_reads_the_plugin_report_only` asserts the fresh and the stored resolution hold `["pypi.org:443"]`. Oracle caveat: the canonical lists are hand-written; `LISTED_DIGEST` was read from the first green run and pinned, so the independent oracle for the digest is the equality of spellings and the inequality with `BASE_DIGEST` and with another list.

Bites, each seen red once and restored from a copy (md5 equal before and after):

| Bite | Red line (tail) |
|---|---|
| index.md not re-blessed | `FAIL candler::it contract_pages::d1_regenerating_the_pages_changes_nothing` (7 run: 6 passed, 1 failed) |
| entries left unsorted (`sort_by_cached_key` removed) | `FAIL candler-core::it guard_egress_setting::egress_checked_per_cell` |
| refusal re-inserted in `settle` (any entry refused) | `FAIL candler-core::it guard_egress_setting::egress_checked_at_resolve`; the same bite also reddened `guard_plugin_environment::layer_lists_append` and `layer_egress_merge_reads_the_plugin_report_only` in an earlier run |
| a refusal row emits `isolation.egress_unsupported` again ("no longer emitted") | `FAIL candler-core::it guard_refusal_codes::r1_every_variant_code_equals_its_row` |
| row with both `retired` and `lifted`, core | `FAIL ... guard_refusal_codes::r1_every_variant_code_equals_its_row` |
| row with both keys, pages | `FAIL candler::it contract_pages::d1_regenerating_the_pages_changes_nothing` and `d3_every_code_on_a_page_is_in_the_catalog` |
| empty `lifted`, pages | `FAIL candler::it contract_pages::d3_every_author_code_has_a_page` and `d1_...` |

Gates (exit code and summary line) follow.

| Gate | Exit | Summary |
|---|---|---|
| `cargo fmt --all --check` | 0 | clean |
| clippy `-p candler-core -p candler --all-targets -D warnings` | 0 | Finished |
| LINUX clippy `x86_64-unknown-linux-gnu --workspace --tests` | 0 | Finished |
| STD nextest `-p candler-core -p candler` | 0 | 446 run, 446 passed, 29 skipped |
| `contract_pages` | 0 | 7 passed |
| Step 4 consistency (egress and plug-in guards) | 0 | 13 passed |
| `guard_harness_facts` (DIGEST, GOLDEN unchanged) | 0 | 10 passed |
| no `egress_unsupported` under `crates/*/src`; row has `lifted` | 0 | ok |
| `cog check a416b7af..HEAD` | 0 | No errored commits |

SUBJ was read as `cog check` (the README's gate list was not opened). The real-daemon rig was not run here; no Docker was touched.
