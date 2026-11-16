# Egress proxy

**Goal**: An allowlisted agent reaches its listed hosts and ports through a per-allowlist egress proxy and nothing else. The token-holding model gateway keeps serving and cannot be used as a proxy. A cell without an allowlist pays nothing (D1, D2). `isolation.egress` is still refused at resolve until `egress_attack` proves the rows and removes the refusal, so this leaf is exercised through the planner and the renderer, not through `candler run`.
**Estimate**: $6.0 (Sonnet, one sitting)
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] OWNED: the egress mode in `crates/candler-core/src/isolation/gateway.rs`, `crates/candler-core/src/isolation/model.rs` (the allow network name), the `isolation.network` match in `crates/candler-core/src/isolation/container/mod.rs`, `crates/candler-core/src/jobs/orchestrate/session.rs` (starting and ending the proxies), `assets/docker/gateway/*`, a new `crates/candler-core/tests/it/guard_egress_proxy.rs`. It does not edit `run_request` (`unstarted_destroy`, L1) nor `resolve/steps/egress.rs` (`egress_attack`, L1)
- [ ] [cmd] The gateway base has the stream SNI module (P3, answered in M5a): `docker run --rm --entrypoint nginx "$(sed -n 's/^FROM //p' assets/docker/gateway/Dockerfile | head -1)" -V 2>&1 | grep -q ssl_preread`
**Success Gates**:
- ⬜ [cmd] The plan: an allowlisted agent joins the internal network and its own allow network, never the egress network; equal allowlists share one proxy; two allowlists get two proxies and each cell joins only its own; a cell without an allowlist joins none; proxies and allow networks are destroyed through the guard with role `egress`: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_egress_proxy::plan_/)'`
- ⬜ [cmd] Cells without `isolation.egress` keep their digests and their argv: DIGEST, and `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_harness_facts::/)'`
- ⬜ [cmd] The egress mode exists in the image and the core: `git grep -nE 'fn render_egress|fn start_egress|fn allow_for' -- crates/candler-core/src/isolation && git grep -n 'stream' -- assets/docker/gateway`
- ⬜ [cmd] SPAWN, STD (`-p candler-core -p candler`), LINUX, E2E and SUBJ pass (README, standard gates)
- ⬜ [nl] P4 is answered in the leaf report with its command and output, and the `--sysctl` decision follows from it. The report records each Bite. Confirms (`config_*` against hand-written expected nginx configs) are culled before merge
**References**: [R04 D1, D2](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md); [R03 Test design](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5b-approved-plan.md); [R01 §Agent environment and secrets](../../__design__/architecture.md) — the model token, the two networks; [moved leaf](../m5-container-isolation/client/seams/egress/egress_proxy.md) — source text

## Step 1: Probe P4, a low port under --cap-drop ALL
**Goal**: Know whether nginx can bind a listed port below 1024 as the image's non-root user with `--cap-drop ALL`, so `--sysctl` is added only if needed.
**Implementation Logic**:
1. Question: can the gateway image's nginx bind port 443 as its non-root user under `--cap-drop ALL`? Docker sets `net.ipv4.ip_unprivileged_port_start=0` by default.
2. Run it in a scratch container made from the gateway image through the one client, with the same hardening the proxy will get (`hardening(limits)`), a throwaway `stream` config listening on 443 and 80, and `nginx -t` then start. Remove the container by id. Wrap the run in the B4 flock.
3. Decision it feeds: step 3 adds `--sysctl net.ipv4.ip_unprivileged_port_start=0` to the proxy's create only if the bind failed. The script is not kept.
**Deliverables**: `__reports__/m5b-egress-proxy/leaves/egress_proxy_v0.md`: section "Probe P4" (the command, the output, the decision)
**Consistency Checks**: `grep -q 'Probe P4' __reports__/m5b-egress-proxy/leaves/egress_proxy_v0.md` (expected: PASS)
**Commit**: `docs(isolation): record the answer of probe P4`

## Step 2: Plan one proxy per distinct allowlist
**Goal**: The job plans and tears down one egress proxy and one allow network per distinct allowlist, and wires each allowlisted agent to its own.
**Implementation Logic**:
1. Guards first, each seen red (`plan_*`, integration with the recording fake driver; the consumer is cell isolation):
   - an allowlisted agent's networks are the internal network and `candler-<job12>-allow-<digest8>`, never the egress network;
   - equal allowlists share one proxy;
   - two distinct allowlists give two proxies, and each cell joins only its own;
   - a cell without an allowlist joins none;
   - teardown destroys proxies and networks through `ContainerGuard`.

   Bite: one proxy holding the union of a job's allowlists. Bite: the agent on the egress network.
2. `<digest8>` is the first 8 hex characters of the canonical list's digest.
3. The proxy container is created through the one client on `hardening(limits)` with role `egress`. Its aliases on the allow network are the listed hostnames. It joins the job's egress network and holds no secret.
4. `isolation.network = bridge` keeps its meaning; `egress_setting` already refuses it together with an allowlist. `session.rs` starts the proxies once per job after the gateway, and ends them with it.
**Deliverables**: `crates/candler-core/src/isolation/gateway.rs`: `EgressProxy`, `start_egress`; `crates/candler-core/src/isolation/model.rs`: `NetworkName::allow_for`; `crates/candler-core/src/isolation/container/mod.rs`: the allow network in the `isolation.network` match of `plan`; `crates/candler-core/src/jobs/orchestrate/session.rs`: start and teardown; `crates/candler-core/tests/it/guard_egress_proxy.rs`: `plan_*`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_egress_proxy::plan_/)'` (expected: PASS)
**Commit**: `feat(isolation): plan one egress proxy per allowlist`

## Step 3: Render the egress mode of the gateway image
**Goal**: The gateway image has an `egress` mode: nginx `stream` with `ssl_preread`, one server per listed port, following D2.
**Implementation Logic**:
1. The rule (D2), rendered from the canonical list by `render_egress`:
   - a port owned by one host forwards everything to it, and an SNI, if present, must equal the host;
   - a port shared by several hosts requires a matching SNI; any other SNI, or a missing one, is closed;
   - upstream names resolve at connect time (a `resolver` with a variable, so the IP is not pinned at start);
   - resolved loopback and link-local addresses are refused, while private ranges are allowed.
2. `assets/docker/gateway/`: the entrypoint picks the mode and renders the stream template from the canonical list. The model gateway's mode is unchanged.
3. Apply P4's decision. P1 is decided: locally an `--internal` network resolves no external name (M5a recorded SERVFAIL), so the agent gets no `--dns` here. `egress_attack`'s probe P1-CI decides CI, and any `--dns` flag is added there.
4. Drive the work with hand-written expected configs and `nginx -t` in the gateway image; these confirms are culled before merge. The real proof is `egress_attack`'s rig.
**Deliverables**: `assets/docker/gateway/`: the egress template and the entrypoint mode (`nginx.conf.template`, `entrypoint.sh`); `crates/candler-core/src/isolation/gateway.rs`: `render_egress`
**Consistency Checks**: `git grep -n 'fn render_egress' -- crates/candler-core/src/isolation/gateway.rs && cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_egress_proxy::plan_/)'` (expected: PASS)
**Commit**: `feat(isolation): forward listed hosts and ports through the egress proxy`
