# Design record

**Goal**: R01 describes what M5 built, with the names as merged. M5 leaves "Decided and scheduled work", and Future features gains the residuals and HTN's second sidecar consumer.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `test -f __design__/architecture.md`
- [ ] [nl] OWNED: `__design__/architecture.md` only. It reads the final names, codes and paths from the merged milestone tip, not from the plan
**Success Gates**:
- ⬜ [cmd] Every new code is in both the code table and R01: `for c in isolation.egress_invalid isolation.egress_with_bridge isolation.plugin_environment_missing isolation.ground_truth_exposed; do grep -q "$c" docs/contract/codes.toml && grep -q "$c" __design__/architecture.md || exit 1; done`
- ⬜ [cmd] The configuration table holds both new paths: `sed -n '/The configuration surface/,/^### Raw traces/p' __design__/architecture.md | grep -q 'isolation.egress' && sed -n '/The configuration surface/,/^### Raw traces/p' __design__/architecture.md | grep -q 'isolation.build.plugin'`
- ⬜ [cmd] The manifest section names the environments: `sed -n '/^### Plug-in manifest/,/^### Suite file/p' __design__/architecture.md | grep -q 'agent_environments'`
- ⬜ [cmd] No cidfile is left in the design: `! sed -n '/^### Process supervision/,/^### Agent environment/p' __design__/architecture.md | grep -qi 'cidfile'`
- ⬜ [cmd] M5 is out of "Decided and scheduled": `! sed -n '/^## Future features/,/^### Execution and isolation/p' __design__/architecture.md | grep -q 'M5 builds'`
- ⬜ [cmd] The sidecar entry names its second consumer: `sed -n '/^### Execution and isolation/,/^### Experiments and analysis/p' __design__/architecture.md | grep 'Plug-in sidecar services' | grep -qi 'MCP'`
- ⬜ [cmd] SUBJ passes
- ⬜ [nl] A spec-verifier (Sonnet, cheap) read the changed sections against the merged code and found no contradiction
**References**: [R06 the R01 change table](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md); [R01](../../../../../../__design__/architecture.md); [AGENTS.md §Source files say what is](../../../../../../AGENTS.md) — R01 holds the reasons

## Step 1: Record the milestone in R01
**Goal**: Each section the plan's change table names says what is now true.
**Implementation Logic**:
1. §Process supervision:
   - `docker create`, then the record, then `docker start -a` as the supervised child; no cidfile;
   - one client and one control builder;
   - the roles agent, gateway, egress and control;
   - the destroy rule of D7 (verify once per decision; identity facts are immutable; run-label listing).
2. §Agent environment and secrets:
   - the egress bullet: one proxy per allowlist, the DNS, SNI and address rules, no TLS interception, bridge versus allowlist;
   - the configuration table rows for `isolation.egress` (cell value) and `isolation.build.plugin` (setting);
   - the client allowlist, with `PATH`'s reason if `docker_client` kept it;
   - the two install routes for an unpublished package, condensed.
3. §Plug-in manifest:
   - the `agent_environments` row;
   - D4's rules: the allowed paths, the layer below the experiment file, scalars yielding, lists appending, agent-only inputs;
   - "The agent's environment is declared by the experiment" becomes "…by the experiment, and by the plug-in environment it selects".
4. §Contracts & Invariants: I8 names its guards and its refusal. The `Isolation` signatures are updated if they changed.
5. §Error model and §Refusals: the new codes.
6. §Risks & Mitigations: an egress-bypass row.
7. §Future features:
   - remove M5 from "Decided and scheduled work";
   - the "Plug-in sidecar services" entry gains its second consumer: MCP servers that reach real systems with a secret (HTN's rikyu servers), run on the runner side as the model gateway holds its key, never in the agent container;
   - add a URL-path allowlist through TLS interception (trigger: an eval that must allow part of a host);
   - add domain fronting (a residual; trigger: a listed host behind a shared CDN front);
   - add detection of a transformed copy of ground truth (trigger: a leak that digests missed).
8. Write in the present tense, without history.
**Deliverables**: `__design__/architecture.md`: §Process supervision, §Agent environment and secrets, §Plug-in manifest, §Contracts & Invariants, §Error model, §Refusals, §Risks & Mitigations, §Future features
**Consistency Checks**: `for c in isolation.egress_invalid isolation.egress_with_bridge isolation.plugin_environment_missing isolation.ground_truth_exposed; do grep -q "$c" __design__/architecture.md || exit 1; done` (expected: PASS)
**Commit**: `docs(design): record container isolation`
