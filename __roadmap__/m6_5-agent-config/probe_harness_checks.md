# Probe harness checks

**Goal**: The plan names, for each harness, a command that reports whether Candler's install of instructions and hooks is valid, and says what that command does without a TTY and without network. Where no such command exists, the plan says `no`. (P0, it decides L2)
**Estimate**: Sonnet, $3, cap $6. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use one slot of `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, and the leaf removes it when it merges. The leaf deletes the containers it starts.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds
**Success Gates**:
- ⬜ `__reports__/agent_config/01-findings_probe_harness_checks_v0.md` answers every question of Step 1. Each answer quotes the command, its exit code and its output, captured in the `base` image (Claude Code 2.1.311) or the `base-opencode` image (OpenCode 2.2.7).
- ⬜ For each harness, the report names the command that L2 wires into `config_check`, and what in its output counts as an error or a warning. If no command is reliable, the report says `no` and why.
- ⬜ `docker ps -a --filter label=m6_5-probe -q` prints nothing at the leaf's close.
**References**: [Architecture report](../__reports__/agent_config/00-architecture_agent_config_v0.md) — the shapes, the error model, the invariants and §Execution model

## Step 1: Probe the checks
**Goal**: Measured answers, not documentation, for each check the plan relies on.
**Implementation Logic**:
Use throwaway containers labelled `m6_5-probe`, with network `none`. Write three installs by hand: a valid one, an error one (bad JSON in `.claude/settings.json`; an OpenCode plugin with no default export) and a warning one (a Claude Code hook on an unknown event). They are the files L2 will render. Ask:
(a) Does `claude doctor` run without a TTY and without network? What is its exit code? Does it report errors and warnings in the project settings' hooks?
(b) Does `InstructionsLoaded` fire under `claude --print --setting-sources project,local` for `.claude/CLAUDE.md`, and what does its payload hold? Use a gateway that refuses the request, so no model is needed.
(c) Do `opencode debug config` and `opencode plugin list` need a server or network? Do they show a rejected config and a failed local plugin?
(d) Does OpenCode's `<config>` directory follow `XDG_CONFIG_HOME`, so that `$XDG_CONFIG_HOME/opencode/AGENTS.md` and `$XDG_CONFIG_HOME/opencode/plugins/` load?
This step changes no product code.
**Deliverables**: `__reports__/agent_config/01-findings_probe_harness_checks_v0.md`, written with `writing-reports`, type findings
**Consistency Checks**: `test -s __reports__/agent_config/01-findings_probe_harness_checks_v0.md` (expected: PASS)
**Commit**: `reports(api): probe the harness checks for instructions and hooks`
