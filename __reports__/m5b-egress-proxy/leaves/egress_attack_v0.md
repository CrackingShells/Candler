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
    
