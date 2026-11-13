# Design record

**Goal**: R01 states what M4 built: the facts a driver supplies, OpenCode V2's confirmed facts, session mode and its close rule, the controls, the imported-strata rule and the new crate; nothing in R01, `CONTRIBUTING.md` or the README is left stale by the milestone.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] Read first: the leaf reports of every merged leaf, and R09's decisions D1–D12 as amended by those reports
- [ ] [nl] OWNED: `__design__/architecture.md`, `README.md` (only if it lists the crates)
**Success Gates**:
- ⬜ [cmd] G7: `s=$(sed -n '/^### OpenCode v2/,/^### Session runner/p' __design__/architecture.md); for k in 'event' 'exit code' 'signal' 'budget' 'binary'; do echo "$s" | grep -qi "$k" || echo "missing $k"; done; ! echo "$s" | grep -qi 'not yet confirmed'` prints nothing
- ⬜ [cmd] G4: §Run manifest states the imported rule: `sed -n '/^### Run manifest/,/^### Normalised/p' __design__/architecture.md | grep -q 'import.rules' && sed -n '/^### Run manifest/,/^### Normalised/p' __design__/architecture.md | grep -qi 'imported'`
- ⬜ [cmd] Session mode is recorded: `for k in 'runner.mode' 'runner.idle_s' 'session.end' 'session.control'; do grep -qF "$k" __design__/architecture.md || echo "$k"; done` prints nothing, and the configuration table lists `runner.mode` as a cell value
- ⬜ [cmd] The workspace section names the new crate: `sed -n '/^### Workspace/,/^## Operation API/p' __design__/architecture.md | grep -q 'candler-harness-opencode'`
- ⬜ [cmd] The old controls sentence is gone: `! grep -q 'run in every session experiment' __design__/architecture.md`
- ⬜ [cmd] The README names the new crate if it lists crates: `! grep -q 'candler-harness-claude-code' README.md || grep -q 'candler-harness-opencode' README.md`
- ⬜ [cmd] SUBJ passes
- ⬜ [nl] The `spec-verifier` agent finds every decision D1–D12 (as amended) stated in R01, and no R01 sentence the milestone's code contradicts
**References**: [R01](../../../../../../__design__/architecture.md); [R09](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-approved-plan.md); `__reports__/m4-session-runner/leaves/` — what each leaf found

## Step 1: Record the driver traits and OpenCode
**Goal**: §Harness drivers and §OpenCode v2 say what is true.
**Implementation Logic**:
§Harness drivers: the trait signatures as built (D9), which facts a driver supplies and which the core owns, the registry in the binary (D10), the skill tree, the shared fixture suite, the in-container pre-stop and the authoritative export. §OpenCode v2: replace "Not yet confirmed" with the confirmed facts and their values (binary, version, event schema, exit codes, signals, budgets, export, state paths, provider configuration, census, codemode). §Workspace: the new crate.
**Deliverables**: `__design__/architecture.md`: §Harness drivers, §OpenCode v2, §Workspace
**Consistency Checks**: `! sed -n '/^### OpenCode v2/,/^### Session runner/p' __design__/architecture.md | grep -qi 'not yet confirmed'` (expected: PASS)
**Commit**: `docs(design): record what a driver supplies and what opencode does`

## Step 2: Record session mode and imported strata
**Goal**: §Session runner, the configuration table and §Run manifest say what is true.
**Implementation Logic**:
§Session runner: D1–D6 (opt-in mode, the close rule and why it never cuts behaviour short, the measured stopping, the controls as instrument checks, hooks, profiles as table-valued model levels); remove "run in every session experiment". The configuration table: `runner.mode` and `runner.idle_s` as cell values. §Run manifest: the `session.*` measured paths, the imported-strata rule (D7: `import.rules`, the legacy build in `comparable.values`, dirty pooling and its assumption, mixed strata refused), and the census rule for a harness without a tool inventory if one was ruled.
**Deliverables**: `__design__/architecture.md`: §Session runner, §Agent environment and secrets (the table), §Run manifest
**Consistency Checks**: `grep -qF 'runner.idle_s' __design__/architecture.md && ! grep -q 'run in every session experiment' __design__/architecture.md` (expected: PASS)
**Commit**: `docs(design): record session mode and the imported-strata rule`

## Step 3: Name the new crate in the README
**Goal**: The README's crate list names the OpenCode driver and what `candler-harness` now holds.
**Implementation Logic**:
In the README's crate list, add `candler-harness-opencode` beside the Claude Code driver, and say that `candler-harness` also holds the skill-tree trait and the shared driver fixture suite. A `docs(readme)` commit releases a core patch, so it touches nothing else.
**Deliverables**: `README.md`: the crate list
**Consistency Checks**: `grep -q 'candler-harness-opencode' README.md` (expected: PASS)
**Commit**: `docs(readme): name the opencode driver crate`
