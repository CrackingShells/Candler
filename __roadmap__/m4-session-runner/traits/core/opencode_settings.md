# OpenCode settings

**Goal**: The OpenCode driver maps each cell setting OpenCode 2.2.7 can honour with the same meaning (effort, permission mode, tool lists, turn cap), refuses the rest with `experiment.setting_unsupported` naming the path, declares both in one table the capability page can read, and M4's driver tests shrink to table-driven guards that keep every bite (amendment A2).
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The refusal it extends is merged: `git log --first-parent --format=%s feat/m4-session-runner | grep -qxF 'merge(runner): land setting_unsupported'`
- [ ] [nl] Read first: the brief `m4-opencode-settings-brief.md` in the shared notes folder (the facts found so far, the six conditions, the consolidation targets); `__reports__/m4-session-runner/leaves/opencode_facts_v0.md`, `opencode_driver_v0.md` and `setting_unsupported_v0.md`
- [ ] [nl] OWNED: `crates/candler-harness-opencode/**`; `crates/candler-harness/src/{driver.rs,conformance.rs}` (the settings table item, with a default, and what the consolidated conformance guard needs); `crates/candler-harness/tests/conformance_shapes.rs`; `crates/candler-harness-claude-code/tests/conformance.rs`; `Cargo.lock` (this crate's dependencies only). Not `docs/contract/codes.toml`: no new code is expected, and a new refusal variant goes to the coordinator first
**Success Gates**:
- ⬜ [cmd] K1 still passes for every driver: `cargo nextest run -p candler-harness --features conformance --no-tests=fail -E 'test(/^conformance_/)' && cargo nextest run -p candler-harness-claude-code -p candler-harness-opencode --no-tests=fail -E 'test(/^conformance_/)'`
- ⬜ [cmd] O1 with the settings rows: every row of the driver's settings table is a case of one command guard, mapped rows assert the argv and written config, refused rows assert `experiment.setting_unsupported` and the path, and a table row without a case (or a case without a row) turns the guard red: `cargo nextest run -p candler-harness-opencode --no-tests=fail -E 'test(/^opencode_/)'`
- ⬜ [cmd] The settings table is declared, not implied: one declared item in `candler-harness` (default empty) that the OpenCode driver fills and both `prepare` and the command guard read; every tool-name row (`Bash`, `Edit`, `Write`, `Read`, `Glob`, `Grep`, `WebFetch`, `WebSearch`, `Task`, `Skill`, …) and every permission mode has a guard case, and the guard asserts the cases and the table rows are the same set; `plan` combined with `tools.allowed`, with a subagent call and with MCP tools, and `tools.allowed` combined with `tools.disallowed`, are rows: `cargo nextest run -p candler-harness-opencode --no-tests=fail -E 'test(/^opencode_command_/)'`
- ⬜ [cmd] Effort is proven on the wire by recorded fake-model fixtures (no tokens): for each effort level the driver maps, the recorded request body carries that level in the field the provider package sends (`output_config.effort` and `thinking` for `@ai-sdk/anthropic`); a level or model with no variant is refused; the turn cap is refused, because OpenCode's `steps` forces a text-only, prompt-injected last request and ends the run as completed, where Claude Code's `--max-turns` ends it as a turn cap (fix round 1); the fixtures are listed in `tests/fixtures/PROVENANCE.toml`: `cargo nextest run -p candler-harness-opencode --no-tests=fail -E 'test(/^opencode_wire_/)'`
- ⬜ [cmd] The headless Claude Code command line is unchanged: `git diff --quiet 9366622 -- crates/candler-harness-claude-code/src crates/candler-harness-claude-code/tests/guard_claude_code_driver.rs && cargo nextest run -p candler-harness-claude-code --no-tests=fail -E 'binary(guard_claude_code_driver)'`
- ⬜ [cmd] The workspace builds and passes with the core and the binary untouched: `git diff --quiet 9366622 -- crates/candler-core crates/candler docs && cargo nextest run --workspace --no-tests=fail`
- ⬜ [cmd] STD (`-p candler-harness -p candler-harness-claude-code -p candler-harness-opencode`), LINUX, DIGEST and SUBJ pass
- ⬜ [nl] The leaf report has: the equivalence table (each setting, Candler's meaning, the OpenCode construct, mapped or refused and why), each claim cited to the v2.2.7 docs or source by file and line; the answers to the coordinator's questions (does a deny rule bind under `--auto`; does `plan` match Claude Code's plan mode; does `steps` count what `--max-turns` counts); the test count before and after per file (`guard_opencode_driver.rs` 28, `conformance_shapes.rs` 17, the Claude Code `conformance.rs` 8) and one recorded bite per merged guard
- ⬜ [nl] The `opencode_settings` verifier closes with no open finding: it re-reads the v2.2.7 source for every mapped row and attacks the equivalence (a mapping that only partly agrees, a dropped level, a default that differs for an unlisted tool), and re-injects one bite per consolidated guard
**References**: [R01 §OpenCode v2, §Harness drivers](../../../../__design__/architecture.md); [the brief](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-opencode-settings-brief.md); [the refusal it extends](../../../../__reports__/m4-session-runner/leaves/setting_unsupported_v0.md); OpenCode at tag `v2.2.7` (`gh api repos/anomalyco/opencode/contents/<path>?ref=v2.2.7`) and context7 `/websites/opencode_ai/v2`

## Step 1: Settle each setting against the pinned source
**Goal**: A probe answers, per setting, whether OpenCode 2.2.7 has a construct with Candler's meaning, from the docs and the source, never from `--help`.
**Implementation Logic**:
1. For `model.effort`, `permission_mode` (each value), `tools.allowed`, `tools.disallowed`, `budget.per_run.turns` and `budget.per_run.cost_usd`, read the V2 docs and the v2.2.7 source and write the equivalence table into the leaf report, with citations. Candler's meaning is what R01 and the Claude Code driver give today.
2. Record two fake-model fixtures in the `base-opencode` image through the gateway sidecar (no RiVault, no tokens): one effort variant, one `steps = N` run. Use an existing `base-opencode` image if one is present; note `docker images -q` before, and remove by exact id only what this leaf built. If the fake model cannot serve OpenCode's provider package, stop and report.
3. A setting whose equivalence is only partial is refused, with the reason in the refusal's detail.
**Deliverables**: `__reports__/m4-session-runner/leaves/opencode_settings_v0.md` (the equivalence table); `crates/candler-harness-opencode/tests/fixtures/` (the two recorded runs, their PROVENANCE rows)
**Consistency Checks**: `cargo nextest run -p candler-harness-opencode --no-tests=fail` (expected: PASS, nothing changed yet in the driver)
**Commit**: `test(adapter-opencode): record effort and step-cap runs on the fake model`

## Step 2: Consolidate the tests
**Goal**: Each behaviour has one guard, the cases of a behaviour are rows of its table, and every bite the old tests had is still seen.
**Implementation Logic**:
1. `guard_opencode_driver.rs` (28 tests) becomes about 10 behaviour guards as the brief lists them; `opencode_stop_interrupts_inside_the_container_then_terms` is removed (it only checks the script's text).
2. `conformance_shapes.rs` (17 tests): the three shape runs and the session-surface guard stay; the 13 checks that refuse a broken driver become one table, one row per check, each row seen red when its check is removed from `conformance.rs`.
3. The Claude Code `conformance.rs` (8 tests): the tracker and pending-work cases become one table; the fixtures, the session command and the gateway names stay.
4. Behaviour is unchanged in this step: the old and the new suites pass on the same code. Every guard keeps its `Guards:`/`Oracle:` lines and its gate prefix. A later fix adds a row, never a test.
**Deliverables**: the three test files; the bite per guard in the leaf report
**Consistency Checks**: `cargo nextest run -p candler-harness --features conformance -p candler-harness-claude-code -p candler-harness-opencode --no-tests=fail` (expected: PASS)
**Commit**: `test(runner): fold m4's driver tests into table-driven guards`

## Step 3: Map what is equivalent, refuse the rest
**Goal**: The driver turns each equivalent setting into OpenCode's construct, refuses the others, and declares both in one table.
**Implementation Logic**:
1. In `candler-harness`, a declared settings table on the driver (one row per setting path: mapped, with a few words on how, or refused, with why), with a default that declares nothing, so Claude Code and the core's fakes compile unchanged. The capability page (`contract_docs`) will read it.
2. In the OpenCode driver, `command` and `prepare` read that table: mapped rows write the agent, `--auto`, the model variant, the permission rules or `agents.<id>.steps` as Step 1 settled; refused rows return `DriverError::unsupported` naming the path. The tool table maps Candler's tool names to OpenCode actions explicitly; a name with no row is refused, and the default for an unlisted action is stated in the table.
3. The command guard of Step 2 gains one row per table row, red first.
**Deliverables**: `crates/candler-harness/src/driver.rs`: the settings table item; `crates/candler-harness-opencode/src/driver.rs`: the table, the mapping; `crates/candler-harness-opencode/tests/guard_opencode_driver.rs`: the rows
**Consistency Checks**: `cargo nextest run -p candler-harness-opencode --no-tests=fail -E 'test(/^opencode_/)'` (expected: PASS)
**Commit**: `feat(adapter-opencode): map the settings opencode can honour`
