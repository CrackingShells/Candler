# Install on Claude Code

**Goal**: On Claude Code, a cell's instruction text loads as project memory and its hooks run, and every run's census names the memory files that loaded. (P0)
**Estimate**: Sonnet, $7, cap $14. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use one slot of `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, and the leaf removes it when it merges. The leaf deletes the containers it starts.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds
- [ ] `[nl]` The probe report settles (a) and (b)
**Success Gates**:
- ⬜ G3 (Claude Code). The *instructions*, *hooks* and *config check* rows show the Claude Code mapping: `.claude/CLAUDE.md`; the `hooks` key of `.claude/settings.json`, with files under `.claude/hooks/`; and the probe's check or `no (…)`. On the base, no such rows exist.
- ⬜ The plan of a cell with all three values renders `.claude/CLAUDE.md` holding the text and a `.claude/settings.json` whose `hooks` key equals the payload byte for byte as JSON. It places the files under `.claude/hooks/` with mode 0755, and the home `--settings` file holds an `InstructionsLoaded` observer in headless mode as in session mode. On the base, the plan has none of these.
- ⬜ A cell without these values renders the same `.claude/settings.json` as on the base. Only the home observers file is added.
- ⬜ The census of a run that recorded `InstructionsLoaded` lists `{path: .claude/CLAUDE.md, scope: Project}`, read from a recorded `.candler/hooks.jsonl` fixture.
- ⬜ An adversarial Sonnet review finds no way for a payload to disable Candler's observers, or to write outside `.claude/`.
**References**: [Architecture report](../../__reports__/agent_config/00-architecture_agent_config_v0.md) — the shapes, the error model, the invariants and §Execution model; `crates/candler-harness-claude-code/src/prepare.rs` (`render_settings`), `session.rs` (`HOOK_EVENTS`, `hooks()`), `discovery.rs` (`EXCLUDED_DIRS`); the probe report

## Step 1: Render and install
**Goal**: The driver writes the values where Claude Code reads them.
**Implementation Logic**:
Extend `render_settings` with the `hooks` key. Do not write a second settings renderer. Add the instructions file and the hook files as `PreparedFile`s. Move the observation hooks file so it is passed in headless mode too, and add `InstructionsLoaded` to `HOOK_EVENTS`. Remove whatever this makes obsolete.
**Deliverables**: `crates/candler-harness-claude-code/src/prepare.rs`, `session.rs`, `driver.rs` (`plan`, `answer`, `fact`); plan guards
**Consistency Checks**: `cargo nextest run -p candler-harness-claude-code` (expected: PASS)
**Commit**: `feat(harness): install instructions and hooks on Claude Code`

## Step 2: Census and check
**Goal**: A run says which memory files loaded, and the driver supplies its harness check.
**Implementation Logic**:
Read the `InstructionsLoaded` records into the census. Implement `config_check` and `read_config_check` from the probe: any error or warning becomes a finding. Flip the answers to Mapped.
**Deliverables**: the census reader; `config_check`, `read_config_check`; the census guard
**Consistency Checks**: `cargo nextest run -p candler-harness-claude-code` and `cargo nextest run -p candler contract_pages` (expected: PASS)
**Commit**: `feat(harness): record loaded memory files on Claude Code`
