# Egress attack

**Goal**: The allowlist holds against every way a container could reach a host that is not listed, while the model gateway still works, shown on a real daemon by one rig (B3). Only then is `isolation.egress_unsupported` removed, as the leaf's last commit, and the egress rows M5a removed are restored.
**Estimate**: $8.5 (Sonnet, one sitting; the adversarial verifier included)
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] L0 is merged: `git log --format=%s | grep -q '^merge(isolation): land egress_proxy'`
- [ ] [nl] OWNED: new `crates/candler-core/tests/it/guard_container_e2e/egress_e2e.rs` (with its `mod` line in `guard_container_e2e.rs`); `.github/workflows/container-e2e.yml` (the expected-guard list; the plan names this leaf its owner, so the other two L1 leaves that add an e2e hand it their names); `crates/candler-core/src/resolve/steps/egress.rs` and `crates/candler-core/src/resolve/refusal.rs` (the refusal's removal); `docs/contract/codes.toml` and `docs/contract/index.md` (the code's row; not in the plan's list, but the contract pages check all three together); the two guard files that carry egress rows, `crates/candler-core/tests/it/guard_egress_setting.rs` and `crates/candler-core/tests/it/guard_plugin_environment.rs`; `crates/candler-core/src/isolation/container/run.rs` only for one `--dns` flag, and only if P1-CI says the agent needs it. It touches no other `src/` file; a defect found there is reported
**Success Gates**:
- ⬜ [cmd] The rig passes, each row judged by its target's own connection log: `cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*egress_/)'`, run under the B4 flock; the same passes in CI's `container-e2e` job (`gh pr checks <N>`)
- ⬜ [cmd] The restored rows pass: `isolation.egress` parses, canonicalises and digests as a cell value: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_egress_setting::/) | test(/^guard_plugin_environment::/)'`
- ⬜ [cmd] The refusal is gone and the pages agree: `! git grep -n egress_unsupported -- ':(glob)crates/*/src/**' docs && cargo nextest run -p candler --no-tests=fail -E 'test(/contract_pages/)'`
- ⬜ [cmd] CI's expected list names the three new e2e guards: `for n in egress_allowlist_holds_on_a_real_daemon destroy_verifies_by_one_inspect i8_canary_never_reaches_the_agent; do grep -q "$n" .github/workflows/container-e2e.yml || exit 1; done`
- ⬜ [cmd] DIGEST and GOLDEN pass, cells without egress keep their argv (`cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_harness_facts::/)'`), and SPAWN, STD (`-p candler-core -p candler`), LINUX, E2E and SUBJ pass (README, standard gates)
- ⬜ [nl] P1-CI is answered in the leaf report. The report records each row's observed target log and each Bite
- ⬜ [nl] The adversarial verifier (Sonnet, a fresh agent that did not write the rig) closed with no open finding before Step 4's commit. It attacks: DNS (external names, tunnelling through the embedded resolver, rebinding to loopback or link-local); IP literals, IPv4 and IPv6; redirects from a listed host to an unlisted one; the model gateway as a proxy (`CONNECT`, absolute URIs, Host headers); cross-cell allow networks; the host gateway and `host.docker.internal`; SNI or port confusion; nginx pinning an upstream IP at start; binding a low port under `--cap-drop ALL` as a non-root user; and the plug-in's appended egress
- ⬜ [nl] `isolation.egress_unsupported` is no longer emitted, and its `codes.toml` row is retired, not deleted: the row stays, because `codes.toml` is append-only and a release carried the code (CONTRIBUTING.md); it is retired, in the form the coordination session rules (a retired row names a replacement, and this one has none). The ruling is recorded in the leaf report
**References**: [R04 D1, D2 and the attack list](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md); [R03 B3 and the guard table](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5b-approved-plan.md); [R01 §Agent environment and secrets](../../../__design__/architecture.md); [moved leaf](../../m5-container-isolation/client/seams/egress/egress_proxy.md) — step 3 is the source text

## Step 1: Probe P1-CI, does an internal network resolve external names on CI
**Goal**: Know whether the agent needs `--dns` set to a dead resolver on CI.
**Implementation Logic**:
1. Question: does `docker network create --internal` resolve external names on the CI runner? Locally it does not (M5a: SERVFAIL).
2. Run it on a scratch branch `probe/m5b-p1-ci` carrying one throwaway ignored e2e that creates an internal network, runs `getent hosts example.com` in a testagent container on it, and removes both by id. Push the scratch branch, read the result with `gh run view --log` once the run ends (never cancel it), and delete the branch. The throwaway is never merged.
3. Decision it feeds: if it resolves, the agent's create gets `--dns` set to a dead resolver (one flag in `container/run.rs`), and the rig's external-DNS row must pass with it.
**Deliverables**: `__reports__/m5b-egress-proxy/leaves/egress_attack_v0.md`: section "Probe P1-CI" (the CI run id, the output, the decision)
**Consistency Checks**: `grep -q 'Probe P1-CI' __reports__/m5b-egress-proxy/leaves/egress_attack_v0.md` (expected: PASS)
**Commit**: `docs(isolation): record the answer of probe P1-CI`

## Step 2: Attack the allowlist on a real daemon
**Goal**: One rig of 11 rows shows the allowlist holds, with the refusal still in place (the rig plans through the core's planner, not through resolve).
**Implementation Logic**:
1. The hermetic rig, no internet: target containers on the job's egress network aliased `allowed.test` and `denied.test`, each logging every connection it receives; the gateway image through the core; the fake model (`assets/docker/fakemodel`); one agent container from the testagent image (found or built through `test_image`) running a client script for every row. Wrap the run in the B4 flock.
2. The rows of `egress_allowlist_holds_on_a_real_daemon`, each judged in the target's log, never by the client's error.
   Must succeed:
   1. the listed host:port, plain when one host owns the port, and with SNI on a shared port;
   2. a model call through the gateway against the fake model.

   Must fail:
   3. another port of the listed host;
   4. an unlisted host;
   5. an IPv4 literal;
   6. an IPv6 literal;
   7. an external DNS name;
   8. the host gateway and `host.docker.internal`;
   9. a wrong or missing SNI on a shared port;
   10. a listed name that resolves to loopback or link-local (the rig may need its own resolver container for the proxy's `resolver`);
   11. the gateway as a proxy: `CONNECT` and absolute-URI `GET http://denied.test/` never reach `denied.test`.
3. Bites, each seen red and recorded: the agent on the egress network (rows 3 to 8, 10 and 11 go red); an upstream IP pinned at start (row 10 red); the loopback filter removed (row 10 red).
4. Remove every container and network the rig made, by exact id; `docker images -q` and `docker ps -aq` before and after are equal.
**Deliverables**: `crates/candler-core/tests/it/guard_container_e2e/egress_e2e.rs`: `egress_allowlist_holds_on_a_real_daemon`; `crates/candler-core/tests/it/guard_container_e2e.rs`: its `mod` line
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*egress_/)'` (expected: PASS)
**Commit**: `test(isolation): attack the egress allowlist on a real daemon`

## Step 3: List the new e2e guards in CI
**Goal**: A renamed, cfg'd-out or skipped e2e fails the `container-e2e` job instead of passing silently.
**Implementation Logic**:
1. Append `egress_allowlist_holds_on_a_real_daemon`, `destroy_verifies_by_one_inspect` (`guard_reuse`) and `i8_canary_never_reaches_the_agent` (`ground_truth_e2e`) to the `for name in …` list of "Check every expected guard ran and passed".
2. Check the job's 20-minute timeout against the measured rig time; raise it only if the rig's time says so, and report it.
3. Push the branch so the cold daemon runs the rig early. The `i8_` name is checked once `ground_truth_e2e` is merged; the coordinator reads CI at the level tip.
**Deliverables**: `.github/workflows/container-e2e.yml`: the three names in the expected list
**Consistency Checks**: `for n in egress_allowlist_holds_on_a_real_daemon destroy_verifies_by_one_inspect i8_canary_never_reaches_the_agent; do grep -q "$n" .github/workflows/container-e2e.yml || exit 1; done` (expected: PASS)
**Commit**: `ci(isolation): expect the egress, destroy and I8 e2e guards`

## Step 4: Remove the refusal and restore the egress rows
**Goal**: A valid `isolation.egress` is accepted at resolve and per cell. This is the leaf's last commit; it lands only after the adversarial verifier closed with no open finding and the rig is green in CI.
**Implementation Logic**:
1. Rows first, seen red: in `guard_egress_setting.rs` and `guard_plugin_environment.rs`, replace each assertion of `isolation.egress_unsupported` (resolve, per cell, a plug-in's list alone and beside the experiment's, stored resolution) with its restored form. Guard: `isolation.egress` parses, canonicalises and digests as a cell value. Oracle: hand-written canonical lists and digests. Bite: unsorted entries change the digest. The old `egress_refused_*` rows are renamed to what they now assert.
2. Delete `CHECK_UNSUPPORTED`, `refuse_until_proxy` and `EgressRefusal::Unsupported` in `egress.rs`; the module doc's "Nothing enforces an allowlist yet" paragraph goes with them. Delete the row in `refusal.rs`; keep the `[resolve."isolation.egress_unsupported"]` table in `codes.toml` and retire it as the coordination ruling says (the row stays, because `codes.toml` is append-only and a release carried the code (CONTRIBUTING.md)); bless `index.md` (`CANDLER_BLESS_CONTRACT=1`, README Contract pages gotcha).
3. A cell without `isolation.egress` is untouched: DIGEST, GOLDEN and `guard_harness_facts::` stay green.
4. Confirm the retirement's form against the coordination ruling before the merge.
**Deliverables**: `crates/candler-core/src/resolve/steps/egress.rs` (without `refuse_until_proxy`); `crates/candler-core/src/resolve/refusal.rs`; `docs/contract/codes.toml`; `docs/contract/index.md`; `crates/candler-core/tests/it/guard_egress_setting.rs` and `guard_plugin_environment.rs`: the restored rows
**Consistency Checks**: `! git grep -n egress_unsupported -- ':(glob)crates/*/src/**' docs && cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_egress_setting::/) | test(/^guard_plugin_environment::/)'` (expected: PASS)
**Commit**: `feat(isolation): enforce isolation.egress and drop its refusal`
