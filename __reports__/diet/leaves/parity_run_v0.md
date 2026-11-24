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
