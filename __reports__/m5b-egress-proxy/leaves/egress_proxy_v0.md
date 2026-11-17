# Egress proxy leaf, v0 (findings)

Verdict (after fix round 1): the leaf's gates run except as listed under Fix round 1. Original verdict was PARTIAL. The proxy image mode, the planner join and the session start/teardown are written and a real nginx relays through them; teardown of allow networks through the guard is blocked by two files outside OWNED; the session-level `plan_*` guards and every bite are not done.

## Probe P4
Command: a scratch container from the gateway base (`nginx@sha256:a8b39bd9...`) with `--init --read-only --cap-drop ALL --security-opt no-new-privileges --pids-limit 128 --memory 256m --user nginx`, two tmpfs, a throwaway `stream` config on 443 and 80, `nginx -t` then run, under `lockf`. Output: `syntax is ok`, `test is successful`, `101` (uid), `443:` `ok443`, `80:` `ok80`, `ip_unprivileged_port_start = 0`. Container removed by id.
Decision: nginx binds 443 and 80 as uid 101 under `--cap-drop ALL`; no `--sysctl` is added. (The stream module is built into this nginx; only `ngx_stream_js_module` is a `load_module`.)

## Commits
See the branch tip. Deviation: the leaf asks for one commit per step; the code of steps 2 and 3 share `gateway.rs`, so they are two commits split by area (plan; image mode).

## Gates
- `guard_egress_proxy::plan_` (3 rows): PASS (3 passed).
- fmt, clippy -D warnings (host and x86_64-unknown-linux-gnu, `-p candler-core --all-targets`): PASS.
- `nextest -p candler-core` (all, non-ignored): PASS (293 passed, 19 skipped).
- STD for `-p candler`, doc, DIGEST/SUBJ/OWNED diff, SPAWN, E2E under `--run-ignored`: NOT RUN (turn cap).

## Guards
All three oracles are hand-derived (shasum of `a.test:443\n` = 754fac9b; the D2 rule written out). Guards were written together with the code, not seen red first, and no bite was run.
- `plan_allow_network_names_follow_the_allowlist`: Guards: allow network per allowlist, never the egress network.
- `plan_render_one_host_per_port`, `plan_render_shared_port_needs_sni`: Guards: D2 per-port and SNI rule.
NOT guarded: that `JobSession::open` starts one proxy per distinct allowlist (`allowlists_of`), that `plan` joins only the allow network, that cells without an allowlist join none, and the guard teardown. The bites "union proxy" and "agent on the egress network" therefore have nothing to bite yet.

## Live check (real daemon, under the lock, ad hoc script, nothing kept)
Gateway image built from this worktree, an internal allow network, a target container aliased `a.test` on a normal network, the proxy with `GW_MODE=egress` joined to both, a client on the allow network only: `wget http://a.test/` returned the target's page and the target's log showed the proxy as peer; `a.test:81` was refused; `b.test` did not resolve. The proxy holds the alias `a.test` on the allow network and the target holds it on the egress network: nginx resolved to the target, not to itself. Image, containers, networks removed by id.

## Findings
1. SPEC DEFECT / BLOCKER (out of OWNED): `container/guard.rs:344` (`destroy_network_with`) accepts only `for_job` and `egress_for_job` names, and `docker.rs` `is_job_network` (used by `network_rm`, line ~992) only `-net` and `-egress`. Both refuse `candler-<job12>-allow-<digest8>`, so a job with an allowlist cannot tear down its allow network through the guard. I did not edit them (guard.rs is `guard_reuse`'s; an edit was also blocked by the harness). Needed: guard.rs line 344 becomes `name.belongs_to(&job)` (added in `model.rs`: `NetworkName::belongs_to`, true for net, egress and allow-<8 hex>), and `is_job_network` also accepts `-allow-<8 hex>`. The sweeper (`sweeper.rs:240`, `guard_reuse`) also lists only the two networks for lost jobs and needs the allow networks.
2. OUT OF OWNED edit: `jobs/orchestrate/teardown.rs` (a loop destroying `session.proxies` through the guard) and `tests/it/main.rs` (one `mod` line, append-only).
3. SPEC GAP: refusing resolved loopback and link-local addresses is not decidable in stock nginx `stream`. Implemented as `egress.js` (`js_filter`): on the first upload or download event it reads `$upstream_addr` and closes if loopback, link-local or empty. Not tested (no way to make a name resolve to 127.x in the live check), and nginx has already connected, so the connect itself is not prevented. `egress_attack`'s row should test it; if it fails, the fix is a resolver that filters.
4. `.dockerignore` of the gateway image is deny-by-default; two lines added for the new files.
5. Limitation: `ssl_preread` waits for a ClientHello, so a server-first plain protocol on a listed port stalls until the preread timeout.
6. No readiness probe of the proxy after start (a bad config leaves an exited container). `egress_attack` should add `docker inspect` state or a connect probe.
7. `ContainerPlan.egress` is reused as "second network" (doc updated): the allow network goes in that field; `run.rs` is unchanged and adds the second `--network`.
8. The base `Gateway` trait gains a default `start_egress` that refuses, so existing fakes compile; a real fake that records requests is still to be written for the session guards.
9. `plan` refuses (plan error) a cell whose `isolation.egress` does not settle; `isolation.egress` is still refused at resolve.

## Network and exposure map (for the coordinator)
Adds, per distinct allowlist per job: one network `candler-<job12>-allow-<digest8>` (`--internal`, labelled with the job), one container (role and run label `egress`, same image as the gateway, hardening: init, read-only, cap-drop ALL, no-new-privileges, pids 128, 256m, tmpfs /tmp and /var/cache/nginx, no mounts, no secret, no published port). Aliases on the allow network: each listed hostname. The proxy joins the allow network and the job's egress network.
- Agent with an allowlist: job internal network + its own allow network. Agent without: job internal only.
- Model gateway: job internal + job egress network, as before; it never joins an allow network.
- Agent path to the host side or egress network: none directly (both of its networks are internal). Via the proxy it reaches only the listed host:port (and what the listed name resolves to, minus loopback/link-local per finding 3). The proxy cannot reach the gateway token (different container, no shared mount). The agent can reach the model gateway only on the internal network, as before. No evaluator-only bytes are mounted into or reachable from the proxy. `host.docker.internal` is not tested here.

## Docker
Before: 14 images, 3 containers, 5 networks. After the scripted runs everything I created was removed by id (smoke image `candler-egress-l0:smoke`, 3 containers, 2 networks, probe container). E2E: none added.

## Fix round 1
Rulings applied: F1 now (`is_job_network` in docker.rs accepts `-allow-<8 hex>`); F1 later (guard.rs `destroy_network_with` and sweeper.rs lost-job listing stay deferred, so the guard still refuses allow networks); F2 teardown.rs loop and the `mod` line accepted; F4 accepted; F5 is a residual.

Guards now (6, all `guard_egress_proxy::plan_*`, 6 passed):
- `plan_one_proxy_per_distinct_allowlist` (Guards: equal allowlists share one proxy, two give two, none gives none; oracle: the leaf rule by hand). Bite union proxy: `allowlists_of` returns the union; red `left: [[]] right: []`.
- `plan_agent_networks` (Guards: allowlisted agent joins internal + own allow network, never egress; none without a list; bridge keeps its meaning; oracle: the leaf rule). Bite agent on egress: red `left: Some(NetworkName("candler-01939d6c1234-egress")) right: Some(NetworkName("candler-01939d6c1234-allow-754fac9b"))`. Bite allow network ignores the list: red `assertion left != right failed` on equal allow names.
- `plan_session_starts_and_ends_one_proxy_per_allowlist` (recording `Gateway` fake with `start_egress`; `JobSession::open` starts exactly [a], [a,b] once each; closing asks the guard about each allow network via the fake client's log). Not bitten separately; the union bite also covers it.
- `plan_render_*` (2 rows, D2). Bite shared-port without SNI check: red at `$egress_up_443 {... default ""; a.test ...` assertion. Bite default forwards: red in both render rows.
Honest note: the guards were written together with the code and shown red by bite afterwards, not before. Bites were restored from copies.
Not proven: teardown of the proxy container and the allow network through the guard with role `egress` (the guard still refuses allow names until guard.rs changes; the session test only proves the guard is asked). No teardown row is ignored or red; it is simply absent until the deferred lines land.

F3 (egress.js): the decision is in the upload-direction filter on `$upstream_addr`, before the first client byte is forwarded; the live check relayed a request, which shows `upstream_addr` is set at the first upload event. It covers 127/8, 169.254/16, 0.0.0.0, `::`, `::1`, fe80::/10, IPv4-mapped IPv6 in dotted and hex form (checked with node on nine addresses). Residual: a TCP connect to the resolved address still happens; no byte is forwarded. The loopback path itself is not exercised on a real rig; `egress_attack` proves it, with a filtering resolver as the fallback.
F6: the gateway's `ready_once` is an exec of curl with the dummy token, specific to the model gateway, so no reuse; readiness of the proxy stays a residual for `egress_attack`.
F5 residual: `ssl_preread` waits for a ClientHello, so a server-first plain protocol on a listed port stalls until the preread timeout.

New out-of-OWNED edits: `crates/candler/src/commands/assets.rs` (the embedded asset list gains `gateway/egress.conf.template` and `gateway/egress.js`, count 14 to 16); `u3_the_embedded_assets_equal_the_tree` failed without it. `GatewayHandle::agent_networks` (model.rs) holds the join rule so `plan` and the guard share one function.

Gates: fmt PASS; clippy -D warnings `-p candler-core -p candler --all-targets` host and x86_64-unknown-linux-gnu PASS; doc -D warnings PASS; `nextest -p candler-core -p candler`: 430 passed, 26 skipped (includes `guard_harness_facts`); E2E under the lock `guard_container_e2e::gateway_e2e::{b17_canary_absent, b18_internal_network_no_route, b37_untrusted_upstream_gets_no_credential}` 3 passed (rebuilds the gateway image with the new files); DIGEST: no field_paths or digest-vector file differs from e05c508 and the digest guards are in the passing suite; no `confirm_` or `config_` tests exist. SPAWN clean-rebuild form skipped (coordinator runs it at the tip). No new e2e tests added.

## Fix round 2 (self-resolution defect, option B)
Evidence that started it (coordinator probe): a busybox container on an `--internal` network with `--network-alias pypi.org`, also on a normal bridge: `nslookup pypi.org 127.0.0.11` returned 172.20.0.2, its own address. My round-0 live check missed it because the target carried the same alias on the egress network, and docker preferred that endpoint.

Change:
- The proxy is created with no `--network-alias` (`gateway::egress_create_args`, now a public free function). After it starts, one `docker inspect -f` through the existing driver `capture` reads its address on the allow network (`EgressProxy.ip`); a missing or non-IPv4 answer is a start error. No inspect per agent.
- `ContainerPlan.add_hosts` (sorted `(name, ip)`; empty for cells without an allowlist) is filled by `DockerIsolation::plan` from the proxies the session started (`with_egress_proxies`); a cell whose allowlist has no proxy gets a plan error and never re-resolves. `container/run.rs` `create_argv` emits `--add-host <name>:<ip>`; this is the file the flag landed in (plus `model.rs`, `container/mod.rs`, `session.rs`). The address lives in the container argv only: no cell value, no digest. The eight test literals of `ContainerPlan` gained `add_hosts: Vec::new()` (out-of-OWNED test edits, mechanical); `harness_baseline/argv.json` is untouched and `guard_harness_facts::` passes.
- Module doc of `gateway.rs` states: no alias equal to a listed name, and the proxy is never restarted inside a job; if it dies the job's egress cells fail and nothing re-resolves.

Guards and bites:
- `guard_egress_proxy::plan_proxy_has_no_listed_alias_and_agents_get_add_host`: the proxy's create argv has no `--network-alias`; the agent's create argv carries the sorted `--add-host` set; none without an allowlist. Bite (alias back in `egress_create_args`): red, panic prints the proxy argv containing `"--network-alias", "a.test", "--network-alias", "b.test"`.
- `guard_container_e2e::egress_proxy_e2e::egress_proxy_never_resolves_a_listed_host_to_itself` (name for CI's list; real daemon, under the lock; the proxy is created from `egress_create_args` on a built gateway image, nothing answers the name on the egress network, `nslookup` runs inside the proxy). Bite (alias back): red after a real run, assertion message `own=<ip> answer=...` (the answer holds the proxy's own address). Containers, networks and the built image are removed by id; counts after the round: 3 containers, 5 networks, 14 images, as before. A first version of the test leaked one container and two networks when it failed early (it called a helper that panics on a non-zero `nslookup`); I removed them by id via the `candler.job` label and fixed the test, which now cleans up before asserting.
- Round-0 e2e lesson: a version whose target carried the same alias on the egress network passed even with the alias put back, so it proved nothing; the final test has no target.

Gates re-run: `guard_egress_proxy::plan_` and `guard_harness_facts::` PASS (17 with the harness facts); `nextest -p candler-core -p candler` 431 passed, 27 skipped; new e2e plus the three gateway e2e under the lock 4 of 4 PASS; fmt PASS; clippy -D warnings on `-p candler-core -p candler --all-targets` host and x86_64-unknown-linux-gnu PASS; DIGEST: the digest guards are in the passing suite and no cell value or digest input changed. Image pulls: none (the target image `busybox` of my first draft was dropped; the nginx base already local is used).
