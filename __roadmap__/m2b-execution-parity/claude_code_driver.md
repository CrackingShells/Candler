# Claude Code driver

**Goal**: `ClaudeCodeDriver` in `candler-harness-claude-code` implements `HarnessDriver` as pure data: the reference command line of contract v5, a prepared workspace (settings, MCP configuration, the skill tree of the tagged commit), exit classification, stop, and output collection globs, with no endpoint, no token and no process action.
**Pre-conditions**:
- [ ] [cmd] The Phase 0 scaffold is landed: `for s in '^docs(design): ' '^build(repo): ' '^ci(repo): ' '^feat(schemas): ' '^feat(orchestrator): scaffold'; do git log --format=%s origin/main..feat/m2b-execution-parity | grep -q "$s" || exit 1; done && test -s __roadmap__/m2b-execution-parity/feature_map/feature_map.json`
- [ ] [nl] api_review closed with no open finding on `HarnessDriver`, `Prepared`, `CommandSpec` and `CollectSpec` against M2a, M3, M4 and M5
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
- [ ] [cmd] `test -s __roadmap__/m2b-execution-parity/feature_map/reference_flags.txt && ls crates/candler-harness-claude-code/tests/fixtures/streams | head -1`
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-harness-claude-code -E 'binary(guard_claude_code_driver)'` passes, holding three B tests. B21: the command is `claude --print <prompt> --model <model> --output-format stream-json --verbose --setting-sources project,local --permission-mode <cell value>` (literal from `reference_flags.txt`). B22: the skill files equal `git ls-tree -r <tag>` of a fixture repo with an untracked file and a later commit. B23: output discovery excludes `.claude/`, any path under a `pyvenv.cfg` directory and read-only input mounts, matching a literal list
- ⬜ [cmd] STD, LINUX and SUBJ pass
- ⬜ [nl] The bite record gives a tried mutation per B test: B21 drops `--setting-sources project,local`; B22 archives `HEAD` instead of the pinned commit; B23 removes `.claude` from the exclusion globs
- ⬜ [cmd] The driver is pure: `! git grep -n -e 'std::process' -e 'tokio::process' -e 'std::net' -e 'ANTHROPIC_BASE_URL' -e 'ANTHROPIC_AUTH_TOKEN' -e 'ANTHROPIC_API_KEY' -- crates/candler-harness-claude-code/src`
- ⬜ [cmd] `! cargo tree -p candler-harness-claude-code --edges normal | grep -q candler-core`
- ⬜ [cmd] No scaffolding: `! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-harness-claude-code`
- ⬜ [nl] No verifier is required for this leaf; every gate above is `[cmd]` or the bite record
**References**:
- [R01 §Harness drivers, §Agent environment and secrets](../../__design__/architecture.md) — the driver returns data only; the core owns env and secrets
- [R03 §8 (Auth env contract), §11, §14](../../__reports__/m2b-execution-parity/00-architecture_v0.md) — R15, `treatment {name, tag, commit}`, the skill as a git archive
- [R04 B21, B22, B23](../../__reports__/m2b-execution-parity/01-test_definition_v0.md) — oracles and bites
- [R06 reference_flags.txt](feature_map/reference_flags.txt) — the exact reference command line
- [R07 SANDBOX_REFERENCE.md](/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion) — `git show htn-creator-eval@0.4.2:.claude/skills/htn-creator-workspace/infrastructure/SANDBOX_REFERENCE.md`, the output discovery rules

## Step 1: The reference command line
**Goal**: `command` builds the contract-v5 Claude Code invocation from the cell and carries no endpoint or token.
**Implementation Logic**:
1. `ClaudeCodeDriver::command(&self, &Cell<'_>, &Workspace<'_>) -> Result<CommandSpec, DriverError>` builds the argv `claude`, `--print`, `<prompt>`, `--model`, `<model.id>`, `--output-format`, `stream-json`, `--verbose`, `--setting-sources`, `project,local`, `--permission-mode`, `<permission_mode>`. The prompt is `prompt.template` with each `{key}` replaced by the value of `prompt.values.<key>`; an unknown key is `DriverError::InvalidValue`. A missing `model.id`, `permission_mode` or `prompt.template` is `DriverError::MissingValue`.
2. It never adds `--bare`, `--allowedTools` or `--max-budget-usd` of its own (reference: not passed). Per-run caps arrive as the extra flags `RunCaps::flags()` computes in `experiment_resolution`; the driver appends the cell's `budget.per_run.turns` and `budget.per_run.max_cost_usd` as `--max-turns` and `--max-budget-usd` only when the cell sets them.
3. The env overlay holds `CONFINE_WORK_DIR` (the workspace root, per the reference) and nothing named `ANTHROPIC_*`. The auth contract (R15) puts `ANTHROPIC_BASE_URL` and `ANTHROPIC_AUTH_TOKEN` in the wrapper's hands, and the wrapper refuses an overlay that carries them or any `EnvValue::Secret`. The driver has no `model.endpoint` read at all.
4. `version_probe`, `parse_version`, `supported` (`>=2.1.220, <2.2.0`, the reader's `SUPPORTED`), `capabilities` and `stop` (Term with a grace) are filled; `id` is `claude-code`.
5. Class A in `#[cfg(test)] mod scaffold`: missing and unknown-key cases, the overlay's names, the probe parse of `2.1.220 (Claude Code)`.
**Deliverables**: `crates/candler-harness-claude-code/src/driver.rs` (`ClaudeCodeDriver`, `ClaudeCodeDriver::new`, `command`, `version_probe`, `parse_version`, `supported`, `capabilities`, `stop`, `render_prompt`)
**Consistency Checks**: `cargo nextest run -p candler-harness-claude-code` (expected: PASS)
**Commit**: `feat(adapter-claude-code): build the reference command line`

## Step 2: Prepare the workspace files
**Goal**: `prepare` returns the settings, MCP configuration and skill files as data, with nothing written and no process run.
**Implementation Logic**:
1. `prepare(&self, &Cell<'_>, &Workspace<'_>) -> Result<Prepared, DriverError>` returns `PreparedFile`s: `.claude/settings.json` with `permissions.allow` from `tools.allowed` and `permissions.deny` from `tools.disallowed` (the reference passes no `--allowedTools`; the project settings source carries them), and `.mcp.json` from `mcp.*` with MCP keys camelCase and every other key snake_case (wire rules). `--setting-sources project,local` makes these the only settings Claude Code reads.
2. The skill tree comes from the tagged commit, not from the working tree (I7). The driver stays pure, so it does not run git: `skill_archive_command(&TreatmentRef, source: &Path) -> CommandSpec` is the argv `git -C <source> archive --format=tar <commit>` (`source` is the setting `request.treatment_source`, D2), and `skill_files(tar: &[u8]) -> Result<Vec<PreparedFile>, DriverError>` is a pure in-crate tar reader that returns the entries (regular files and the executable bit; a link, a device or a path leaving the root is `DriverError::InvalidValue`). `run_exec` runs the command through `Supervisor::run_captured` (D1, I15) and passes the bytes to `skill_files`; the files join the `Prepared` list before they are written.
3. The workspace paths in the files are relative; no absolute path is written.
4. Class A in `mod scaffold`: the rendering of `permissions` and `.mcp.json` from a cell, and the tar reader over a hand-built archive (entry, executable bit, refused link, refused `../`).
**Deliverables**: `crates/candler-harness-claude-code/src/prepare.rs` (`ClaudeCodeDriver::prepare`, `render_settings`, `render_mcp`, `skill_archive_command`, `skill_files`), merged into `Prepared.files` before `Isolation::plan`)
**Consistency Checks**: `cargo nextest run -p candler-harness-claude-code` (expected: PASS)
**Commit**: `feat(adapter-claude-code): prepare settings, mcp config and the skill tree`

## Step 3: Exit classification, stop and collection
**Goal**: `classify_exit` and `collect` follow the contract: a Claude Code exit is classified from the trace sessions, and output discovery skips `.claude/`, virtual environments and read-only inputs.
**Implementation Logic**:
1. `classify_exit(&self, &ProcessExit, &[Session]) -> EndedBy` returns `Completed` for a finished result without error, `AgentError {reason}` for an error result with no provider status, and `Cap {cap}` for the max-turns and budget results; a missing or silent trace returns `Unknown`. It never returns `Killed`, `BudgetStop`, `Interrupted` or `GatewayError`: those come from facts of the core (R2).
2. `collect(&self, &Workspace<'_>, &Observed) -> Vec<CollectSpec>` returns the output globs and the transcript sources, and the exclusions: `.claude/**`, any path with a `pyvenv.cfg` in an ancestor directory, and the paths of read-only input mounts (named by `Workspace` inputs). Exclusions are patterns in data, so the core applies them.
3. `live_reader` and `offline_reader` return `ClaudeCodeReader` (M1), unchanged.
4. Class A: `classify_exit` over the fixture streams (completed, max-turns, error result) and the glob list shape.
**Deliverables**: `crates/candler-harness-claude-code/src/driver.rs` (`classify_exit`, `collect`, `live_reader`, `offline_reader`); `crates/candler-harness-claude-code/src/discovery.rs` (`EXCLUDED_DIRS`, `exclusion_patterns`)
**Consistency Checks**: `cargo nextest run -p candler-harness-claude-code` (expected: PASS)
**Commit**: `feat(adapter-claude-code): classify exits and collect outputs`

## Step 4: B21–B23 guards, scaffold deletion and bite record
**Goal**: The command, the skill tree and the discovery exclusions are held by three guards with independent oracles, and no scaffolding stays.
**Implementation Logic**:
1. `tests/guard_claude_code_driver.rs` (`#![cfg(test)]`). B21 `b21_reference_command_line`: the argv equals a literal copied from `reference_flags.txt` (provenance comment with the file and line); positive control: the permission mode follows the cell. Doc lines: `Guards: contract v5 — the agent command`, `Oracle: feature_map/reference_flags.txt`, `Bite: drop --setting-sources project,local`.
2. B22 `b22_skill_tree_is_the_tag`: the test builds a fixture git repo with the tag at an older commit, a later commit changing a file, and an untracked file; runs `skill_archive_command`'s argv with `std::process::Command`, feeds the bytes to `skill_files`, and compares the path set and the contents with `git ls-tree -r <tag>` and `git cat-file blob`. Positive control: a tracked file at the tag is present. Doc lines: `Guards: I7 — the treatment is {name, tag, commit}`, `Oracle: git ls-tree -r <tag>`, `Bite: archive HEAD instead of the pinned commit`.
3. B23 `b23_output_discovery_exclusions`: a fixture tree with `out/a.txt`, `.claude/x`, `venv/pyvenv.cfg` with `venv/lib/y.py`, and a read-only input path; the matched list equals a literal from `SANDBOX_REFERENCE.md` v5. Doc lines: `Guards: I8 — output discovery`, `Oracle: SANDBOX_REFERENCE.md v5`, `Bite: remove .claude from the exclusion globs`.
4. Delete every `#[cfg(test)] mod scaffold` and `tests/scaffold_*`. Try each mutation, restore with `git checkout -- <file>`, and write the bite record in the leaf report.
**Deliverables**: `crates/candler-harness-claude-code/tests/guard_claude_code_driver.rs` (`b21_reference_command_line`, `b22_skill_tree_is_the_tag`, `b23_output_discovery_exclusions`); the leaf report's bite record
**Consistency Checks**: `cargo nextest run -p candler-harness-claude-code -E 'binary(guard_claude_code_driver)' && ! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-harness-claude-code` (expected: PASS)
**Commit**: `test(adapter-claude-code): guard the command, the skill tree and discovery`
