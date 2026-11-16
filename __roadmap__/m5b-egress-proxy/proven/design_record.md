# Design record

**Goal**: R01 describes what M5b built, with the names as merged. M5b leaves "Decided and scheduled work", the refusal `isolation.egress_unsupported` leaves the text, and Future features gains the residuals.
**Estimate**: $1.5 (Sonnet, one sitting)
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `test -f __design__/architecture.md`
- [ ] [cmd] L0 is merged: `git log --format=%s | grep -q '^merge(isolation): land ground_truth_refusal'`
- [ ] [nl] OWNED: `__design__/architecture.md` only. It writes B2 and B3 from the plan and reads the names, codes and paths from the L0 tip. The level's integration review checks its text against the L1 tip. M5a already recorded `docker create`, the client, the roles and the `isolation.egress` setting; verify each section first and change only what is not yet true
**Success Gates**:
- ⬜ [cmd] The new code is in the code table and in R01: `grep -q isolation.ground_truth_exposed docs/contract/codes.toml && grep -q isolation.ground_truth_exposed __design__/architecture.md`
- ⬜ [cmd] The refusal is out of the design: `! grep -q egress_unsupported __design__/architecture.md`
- ⬜ [cmd] M5b is out of "Decided and scheduled": `! sed -n '/^## Future features/,/^### Execution and isolation/p' __design__/architecture.md | grep -q 'M5b builds'`
- ⬜ [cmd] The residuals are listed: `sed -n '/^## Future features/,/^### Experiments and analysis/p' __design__/architecture.md | grep -qi 'domain fronting' && sed -n '/^## Future features/,/^### Experiments and analysis/p' __design__/architecture.md | grep -qi 'TLS interception'`
- ⬜ [cmd] The sidecar entry names its second consumer, MCP servers: `sed -n '/^### Execution and isolation/,/^### Experiments and analysis/p' __design__/architecture.md | grep 'Plug-in sidecar services' | grep -qi 'MCP'`
- ⬜ [cmd] R01 names the destroy rule and the proxy: `sed -n '/^### Process supervision/,/^### Agent environment/p' __design__/architecture.md | grep -qi 'one inspect\|inspect_many' && sed -n '/^### Agent environment and secrets/,/^### Plug-in manifest/p' __design__/architecture.md | grep -qi 'proxy'`
- ⬜ [cmd] SUBJ passes (README, standard gates)
- ⬜ [nl] A spec-verifier (Sonnet, cheap) read the changed sections against the L1 tip and found no contradiction
**References**: [R03 B2, B3, the plan's decisions](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5b-approved-plan.md); [R04 the R01 change table](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md); [R01](../../../__design__/architecture.md); [AGENTS.md §Source files say what is](../../../AGENTS.md) — R01 holds the reasons; [moved leaf](../../m5-container-isolation/client/seams/egress/proof/design_record.md) — source text

## Step 1: Record the milestone in R01
**Goal**: Each section the plan names says what is now true, in the present tense, without history.
**Implementation Logic**:
1. §Process supervision: the destroy rule of D7 (verify once per decision; identity facts are immutable; run-label listing); B2 (a container created but never run is destroyed by its creator, by id, through the guard in the run scope).
2. §Agent environment and secrets: the `isolation.egress` bullet no longer says "until the egress proxy exists". It states: one proxy per distinct allowlist; the per-port and SNI rule; names resolved at connect time; loopback and link-local refused; no TLS interception; bridge versus allowlist.
3. §Contracts & Invariants: I8 names its guards (`guard_ground_truth_refusal::refuses_*`, `i8_canary_never_reaches_the_agent`) and its refusal.
4. §Error model and §Refusals: `isolation.ground_truth_exposed` is added and `isolation.egress_unsupported` leaves R01's text; its row stays in `codes.toml`, lifted, which is where the registry keeps it.
5. §Risks & Mitigations: an egress-bypass row (B3's attack rows are its mitigation evidence).
6. §Future features:
   - remove M5b from "Decided and scheduled work";
   - the "Plug-in sidecar services" entry gains its second consumer: MCP servers that reach real systems with a secret (HTN's rikyu servers), run on the runner side as the model gateway holds its key, never in the agent container;
   - add a URL-path allowlist through TLS interception (trigger: an eval that must allow part of a host);
   - add domain fronting (a residual; trigger: a listed host behind a shared CDN front);
   - add detection of a transformed copy of ground truth (trigger: a leak that digests missed).
**Deliverables**: `__design__/architecture.md`: §Process supervision, §Agent environment and secrets, §Contracts & Invariants, §Error model, §Refusals, §Risks & Mitigations, §Future features
**Consistency Checks**: `grep -q isolation.ground_truth_exposed __design__/architecture.md && ! grep -q egress_unsupported __design__/architecture.md` (expected: PASS)
**Commit**: `docs(design): record the egress proxy and the ground-truth guard`
