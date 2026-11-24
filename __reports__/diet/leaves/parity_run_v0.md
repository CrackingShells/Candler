# Parity run leaf, v0

One cell with a real MCP server, run on Claude Code and on OpenCode with code mode at its default,
shows on each harness that the server is installed and that the stored trace holds the agent's call
to its tool as the same `ToolOrigin::Mcp`.

Base `a1f14719` (`origin/feat/integrity` at the branch point). Tip: the tip of `leaf/l3-parity-run`.

## Live record (step 2)

Two runs of the guard's experiment shape, one per harness, on RiVault (reachable; `rv_health` ok).
The experiment is the guard's: one cell, `permission_mode = "bypassPermissions"`, model
`RiVault/Reasoning-Medium`, the declared server `parity` (`python3 servers/parity/parity_mcp.py`
from an agent input), code mode left out; only the `[harness]` name differs. The key's variable is
`RIVAULT_LLM_INFERENCE` (`rv_config.py var llm`); no key was printed. No run was repeated.

| | Claude Code | OpenCode |
|---|---|---|
| job | `01a20f84-4e1a-7846-b388-c9ba5c76d190`, completed | `01a20f84-9378-71bd-9f47-12e2da5782fe`, completed |
| alias | `RiVault/Reasoning-Medium` | `RiVault/Reasoning-Medium` |
| model the proxy returned | `RiVault/Reasoning-Medium` (the `model` of the streamed answers, 8 times in stdout, once in stderr) | `RiVault/Reasoning-Medium` (the `model.id` of the export) |
| backing model | `deepseek-ai/DeepSeek-V4-Flash` (`rv_models.py`, the live listing; the proxy answers with the alias, not the backing name) | the same |
| resolved `mcp.*` (`candler inspect`) | `mcp.servers.parity.command = "python3"`, `.args = ["servers/parity/parity_mcp.py"]`, `.codemode = false` | the same three values |
| stored trace, tool calls | `ToolSearch` builtin, `ToolSearch` builtin, `mcp__parity__stamp` with origin `{kind: mcp, server: parity, tool: stamp}` | `parity_stamp` with origin `{kind: mcp, server: parity, tool: stamp}` |

Both traces attribute the call to the same server and tool. The shape matches the guard's: Claude
Code reaches the tool through its own `ToolSearch` (it defers MCP tools), OpenCode lists it at once,
and the one MCP call reads the same in both stored traces. Numbers are not compared. The runs' store,
scratch worktree files and containers are removed (`docker ps -a` shows none of the job's; no
network is left).

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| Guard in `container-e2e` with real images, production gateway, scripted fake model; per harness the server in the resolved `mcp.*` values and in the rendered MCP file, and the scripted call stored as `ToolOrigin::Mcp` with one server and tool | PASS | `cargo nextest run -p candler --test it -E 'test(parity_one)' --run-ignored only`: PASS, 22.5 s locally; CI `PASS [37.879s] (3/23) ... guard_cli_e2e::parity_e2e::parity_one_installed_server_one_traced_call`; `container-e2e` green on PR 79 |
| Fails on the base on OpenCode with `origin: other` | PASS, on a different base than the leaf names | `origin/feat/integrity` already holds the trace-parity reader, so the failure is shown on `29573006` (merge of #74). The guard does not compile there unchanged (`ToolOrigin::mcp_tool` is absent); with its oracle adapted to the origin that had no `tool` (`{kind: mcp, server: parity}`) it fails: `opencode: MCP tool calls [] among [{"kind":"other"}]`. `candler inspect` shows no `mcp` member there either (`no mcp.servers.parity in null`, both harnesses) |
| No census, no server status | PASS | the guard reads `candler inspect`'s `mcp` member, the files in the run, and the stored trace |
| Same experiment file, harness the only difference; code mode at default, no code-mode case | PASS | the guard asserts the two files differ in exactly the line `name = "claude-code"` / `name = "opencode"`; `codemode` is not written, and the resolved value is `false` |
| Fixture needs nothing outside the base images | PASS | one stdlib `python3` file. Both agent base images carry `python3` (probed: `/usr/local/bin/python3` in the Claude Code and the OpenCode image; neither has `node`) |
| One live run per harness, same shape, call attributed to the server; store and containers removed | PASS | the live record above |
| Container-e2e added time | PASS | see Time |
| "What it leaves" at three levels | PASS | below |
| fmt, clippy `-D warnings`, nextest workspace, doc `-D warnings`, `container-e2e` | PASS | fmt clean; clippy clean after two doc backticks; `732 tests run: 732 passed, 32 skipped`; `cargo doc` exit 0; CI `container-e2e` success |
| Bite (step 3), edit A: OpenCode's reader drops the server from the origin | PASS (fails as it should) | `opencode: MCP tool calls [] among [{"kind":"other"}], expected [{"kind":"mcp","server":"parity","tool":"stamp"}]` |
| Bite, edit B: Claude Code's renderer skips the server | PASS (fails as it should) | `claude-code: 0 .mcp.json files hold the server under /mcpServers: []` and `claude-code: MCP tool calls [] among []` |

Deviations: the guard is in `crates/candler/tests/it/guard_cli_e2e/parity_e2e.rs`, not
`candler-core`'s tests, because the producer it reads (run, resolved values, trace) is the
`candler` binary. The step 1 commit line is 77 characters and the `commit-msg` hook allows 72, so it
reads `test(runner): hold both harnesses to one MCP install and traced call`. The pull request title
is cut to 66 characters. The fresh verifier of step 3 is the lead's.

## Time

- Locally the guard runs in 22.5 s for both harnesses (images cached).
- CI, `Container e2e (Linux)` on PR 79: 7 min 40 s, of which the guard took 37.9 s. The latest run on
  a leaf branch before it (`leaf/l3-pages`) took 7 min 12 s: about 28 s more.

## What it leaves

How it is developed. The guard was written by reading the i8 guard and the control runner once,
then run against the real images; two failed runs before the first pass cost seconds, not builds
(the fake model had to learn Claude Code's `ToolSearch`). Docker use: one fake-model image and
container per run, removed by exact id by the guard; the live runs used the cached images and their
stores are removed. No image was built that was not there. The two scratch builds (the old base, the
two bite edits) shared one scratch target dir, removed after.

What it leaves in the code. One guard, one fixture file, one script in the fake model; the i8 guard
and this one share `Made`, `must` and `fake_host` instead of copying them. The guard builds no
state: the run, its resolved values, the rendered file and the trace all come from `candler run`
and `candler inspect`. It asserts what the leaf names and no more. The oracle for the origin is the
type's own constructor. Nothing is kept for another reader.

How it is consumed. The experimenter's view does not change. A maintainer who lets one harness
install or trace a declared server differently now gets a red `container-e2e`, naming the harness,
the file or the origin that differs.

## Not covered

- Permission defaults. The experiment sets `permission_mode = "bypassPermissions"` (the mode both
  drivers map; OpenCode refuses `default`), so the guard and the live runs prove MCP install and
  trace parity, not what each harness does with no mode. That choice is still open.
- Code mode: no case, by the leaf.
- The rendered file is read from the run's stored home and workspace by name (`.mcp.json`,
  `opencode.json`); a harness that renders under another name needs its row in the guard.
- A tool that is deferred behind a harness's own search (Claude Code) is reached by the fake model
  through `ToolSearch`; a Claude Code version that changes that protocol breaks the script, not the
  guard's assertions.

## Observations for the lead

- The leaf and the lead's reading assumed the OpenCode image has no `python3` and that `node` is on
  both images. Probed on the local images: both have `python3`, neither has `node`.
- On the merge of #74 `candler inspect` shows no `mcp` member; it arrived with #77.
