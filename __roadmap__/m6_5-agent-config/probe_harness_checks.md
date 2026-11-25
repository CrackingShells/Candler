# Probe harness checks

**Goal**: The plan names, for each harness, a command that reports whether Candler's install of instructions and hooks is valid, and says what that command does without a TTY and without network. Where no such command exists, the plan says `no`. It also settles where Claude Code's global file loads, whether both harnesses read a read-only file mounted at the workspace root, and which session-start hook fires headless. (P0, it decides L2)
**Estimate**: Sonnet, $3, cap $6. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use one slot of `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, and the leaf removes it when it merges. The leaf deletes the containers it starts.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds
**Success Gates**:
- ⬜ `__reports__/agent_config/03-findings_probe_harness_checks_v0.md` answers every question of Step 1. Each answer quotes the command, its exit code and its output, captured in the `base` image (Claude Code 2.1.311) or the `base-opencode` image (OpenCode 2.2.7).
- ⬜ The report names where L2 installs Claude Code's global file (the run's home with `user` among the setting sources, or the workspace's parent directory) and why, and the order in which each harness puts the global and the workspace text in front of the model.
- ⬜ For each harness, the report names the command that L2 wires into `config_check`, and what in its output counts as an error or a warning. If no command is reliable, the report says `no` and why.
- ⬜ For each harness, the report names the session-start hook L3 uses and where its output lands in the first model request (system text or first message). If none reaches the first request, the report says so, and the plan returns to Eliott before L2.
- ⬜ `docker ps -a --filter label=m6_5-probe -q` prints nothing at the leaf's close.
**References**: [Architecture report, v2](../../__reports__/agent_config/02-architecture_agent_config_v2.md) — the shapes, the error model, the invariants and §Execution model

## Step 1: Probe the checks
**Goal**: Measured answers, not documentation, for each check the plan relies on.
**Implementation Logic**:
Use throwaway containers labelled `m6_5-probe`, with network `none`. Write three installs by hand: a valid one, an error one (bad JSON in `.claude/settings.json`; an OpenCode plugin with no default export) and a warning one (a Claude Code hook on an unknown event). They are the files L2 will render. Ask:
(a) Does `claude doctor` run without a TTY and without network? What is its exit code? Does it report errors and warnings in the project settings' hooks?
(b) Does `InstructionsLoaded` fire under `claude --print --setting-sources project,local` for the workspace's `CLAUDE.md`, and what does its payload hold? Use a gateway that refuses the request, so no model is needed.
(c) Do `opencode debug config` and `opencode plugin list` need a server or network? Do they show a rejected config and a failed local plugin?
(d) Does OpenCode's `<config>` directory follow `XDG_CONFIG_HOME`, so that `$XDG_CONFIG_HOME/opencode/AGENTS.md` and `$XDG_CONFIG_HOME/opencode/plugins/` load?
(e) Under `--setting-sources user,project,local` with a home that holds only `~/.claude/CLAUDE.md` and Candler's files, does that file load, and does `user` load anything else? Does a `CLAUDE.md` in the workspace's parent directory load instead?
(f) Is a single file bind-mounted read-only at the workspace root (`/work/CLAUDE.md`, `/work/AGENTS.md`) read by each harness, as `InstructionsLoaded` or an `Instructions from:` block shows? In which order do the global and the workspace text reach the model?
(g) On each harness, which session-start hook fires in a headless run (Claude Code `SessionStart` with `--print`; an OpenCode plugin hook that adds to the system text, or the nearest one), and does its output reach the first model request, in the system text or the first message? Read the request at a gateway that records it.
This step changes no product code.
**Deliverables**: `__reports__/agent_config/03-findings_probe_harness_checks_v0.md`, written with `writing-reports`, type findings
**Consistency Checks**: `test -s __reports__/agent_config/03-findings_probe_harness_checks_v0.md` (expected: PASS)
**Commit**: `reports(api): probe the harness checks for instructions and hooks`
