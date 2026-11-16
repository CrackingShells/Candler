# Egress proxy

**Goal**: An allowlisted agent reaches its listed hosts and ports through a per-allowlist egress proxy and nothing else. The token-holding model gateway keeps serving and cannot be used as a proxy. A cell without an allowlist pays nothing (D1, D2).
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] OWNED: the egress mode in `crates/candler-core/src/isolation/gateway.rs`, `assets/docker/gateway/*`, the network choice in `crates/candler-core/src/isolation/container/mod.rs` (the `isolation.network` match), `isolation/model.rs` (the allow network name), `jobs/orchestrate/session.rs` (starting and ending the proxies), new guard and e2e files
- [ ] [nl] The README's P1 and P3 answers are recorded: whether the agent also needs `--dns` set to a dead resolver, and that the gateway base has `ssl_preread`
**Success Gates**:
- ⬜ [cmd] The plan: an allowlisted agent joins the internal network and its own allow network, never the egress network; equal allowlists share one proxy; two allowlists get two proxies and each cell joins only its own; a cell without an allowlist joins none; proxies and allow networks are destroyed through the guard with role `egress`: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_egress_proxy::plan_/)'`
- ⬜ [cmd] The rendered proxy config follows D2's per-port and SNI rule, against hand-written expected configs: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_egress_proxy::config_/)'`
- ⬜ [cmd] The adversarial e2e passes, each row's oracle being the target container's own connection log: `cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*egress_/)'`
- ⬜ [cmd] Cells without `isolation.egress` keep their digests and their argv: DIGEST, and `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_harness_facts::/)'`
- ⬜ [cmd] SPAWN, STD (`-p candler-core -p candler`), LINUX, E2E and SUBJ pass
- ⬜ [nl] The leaf report records each row's observed target log and each bite
- ⬜ [nl] The adversarial verifier closed with no open finding. It attacks: DNS (external names, tunnelling through the embedded resolver, rebinding to loopback or link-local); IP literals, IPv4 and IPv6; redirects; the model gateway as a proxy (`CONNECT`, absolute URIs, Host headers); cross-cell allow networks; the host gateway and `host.docker.internal`; SNI or port confusion; nginx pinning an upstream IP at start; binding a low port under `--cap-drop ALL` as a non-root user; and the plug-in's appended egress
**References**: [R06 D1, D2](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md); [R01 §Agent environment and secrets](../../../../../__design__/architecture.md) — the model token, the two networks

## Step 1: Plan one proxy per distinct allowlist
**Goal**: The job plans and tears down one egress proxy and one allow network per distinct allowlist, and wires each allowlisted agent to its own.
**Implementation Logic**:
1. Guards first, seen red (`plan_*`, with the recording fakes):
   - an allowlisted agent's networks are the internal network and `candler-<job12>-allow-<digest8>`;
   - equal allowlists share one proxy;
   - two distinct allowlists give two proxies, and each cell joins only its own;
   - a cell without an allowlist joins none;
   - teardown destroys proxies and networks through `ContainerGuard`.

   Bite: one proxy holding the union of a job's allowlists.
2. `<digest8>` is the first 8 hex characters of the canonical list's digest.
3. The proxy container is created through the one client on `hardening(limits)` with role `egress`. Its aliases on the allow network are the listed hostnames. It joins the job's egress network. It holds no secret.
4. `isolation.network = bridge` keeps its meaning; `egress_setting` already refuses it together with an allowlist.
**Deliverables**: `crates/candler-core/src/isolation/gateway.rs`: `EgressProxy`, `start_egress`; `crates/candler-core/src/isolation/model.rs`: `NetworkName::allow_for`; `crates/candler-core/src/isolation/container/mod.rs`: the allow network in `plan`; `crates/candler-core/src/jobs/orchestrate/session.rs`: start and teardown; `crates/candler-core/tests/it/guard_egress_proxy.rs`: `plan_*`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_egress_proxy::plan_/)'` (expected: PASS)
**Commit**: `feat(isolation): plan one egress proxy per allowlist`

## Step 2: Render the egress mode of the gateway image
**Goal**: The gateway image has an `egress` mode: nginx `stream` with `ssl_preread`, one server per listed port.
**Implementation Logic**:
1. Guards first, seen red (`config_*`), against hand-written expected configs:
   - a port owned by one host forwards everything to it, and an SNI, if present, must equal the host;
   - a port shared by several hosts requires a matching SNI;
   - any other SNI, or a missing SNI on a shared port, is closed;
   - upstream names resolve at connect time (a `resolver` with a variable, so the IP is not pinned at start);
   - resolved loopback and link-local addresses are refused, while private ranges are allowed.
2. `assets/docker/gateway/`: the entrypoint picks the mode, and the stream template is rendered from the canonical list. The model gateway's mode is unchanged.
3. Measure whether nginx can bind a listed port under 1024 as the image's non-root user with `--cap-drop ALL` (Docker sets `net.ipv4.ip_unprivileged_port_start=0` by default). Record the answer, and use `--sysctl` only if it is needed.
4. If P1 showed that an internal network resolves external names, the agent gets `--dns` set to a dead resolver, so only container aliases resolve.
**Deliverables**: `assets/docker/gateway/` (the egress template and the entrypoint mode); `crates/candler-core/src/isolation/gateway.rs`: `render_egress`; `guard_egress_proxy.rs`: `config_*`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_egress_proxy::config_/)'` (expected: PASS)
**Commit**: `feat(isolation): forward listed hosts and ports through the egress proxy`

## Step 3: Attack the allowlist on a real daemon
**Goal**: The allowlist holds against every way a container could reach a host that is not listed, while the model gateway still works.
**Implementation Logic**:
1. Set up the hermetic rig, with no internet:
   - target containers on the job's egress network, aliased `allowed.test` and `denied.test`, each logging every connection it receives;
   - the gateway image built through the core;
   - the agent running the testagent image (found or built through `test_image`), with a client script.
2. Rows of `egress_*`, each checked in the target's own log, never by the client's error:
   - the listed host:port is reachable, plain and over TLS with SNI;
   - the listed host on another port fails;
   - an unlisted host fails;
   - the IP literal of a reachable target fails;
   - an IPv6 literal fails;
   - an external DNS name does not resolve;
   - `host.docker.internal` and the host-gateway IP are unreachable;
   - an unlisted SNI on a listed port is closed;
   - a listed name that resolves to loopback or link-local is refused;
   - a redirect from the listed host to an unlisted one fails;
   - the model gateway's `CONNECT` and absolute-URI `GET http://denied.test/` never reach `denied.test`;
   - a model call through the gateway against the fake model (`assets/docker/fakemodel`) still succeeds.
3. Bite: the agent joined to the egress network turns several rows red.
4. Remove every container and network the rig made, by id.
**Deliverables**: `crates/candler-core/tests/it/guard_container_e2e/egress_e2e.rs`: `egress_*`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*egress_/)'` (expected: PASS)
**Commit**: `test(isolation): attack the egress allowlist on a real daemon`
