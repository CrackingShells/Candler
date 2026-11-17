# M5b live smoke: findings (v1)

Date: 2026-11-17. Tip 3ba9ca28, release build (`/private/tmp/claude-501/m5b-target-mcp`). Nothing committed; the checkout was not edited.

## Verdict

Run key `2f9e642134685d256d46fa86729f342e2c24aa4967a2fdf07ace0ea1c45f608d-r1-a1`, job `01a1e780-5310-7c3a-9718-216900be5694`: valid, `ended_by = completed`, exit 0, 12.9 s.
(1) MCP tools listed: **PASS**. (2) pypi.org fetched through egress with a real TLS handshake: **PASS**. (3) example.com unreachable: **PASS** (weak, as in v0: the agent saw a DNS failure, not a proxy denial).

## Setup (as v0, changes only where the fixes require)

| Item | Value |
|:--|:--|
| Model | `RiVault/Agentic-Medium`, one run, cost `run.cost_usd` 0.149110 (client-side); 5 turns; usage in 23,695 / out 2,609 / cache read 10,752 |
| Plug-in | `agent-env` re-copied from the tip, unpatched; `mcp.servers.tools.args = ["env/server.py"]`, environment `tiny` |
| Egress | `"isolation.egress" = ["pypi.org:443"]` |
| Store | fresh `CANDLER_STORE=$HOME/candler-m5b-smoke/work/.candler/store_v1b` (v0's store pins the old cell and treatment commit, so the same experiment was refused as "a re-checked fact differs"); `store_v1` holds one job that failed before any model call (see Notes) |
| Gateway image | `docker build --pull=false -t candler-gateway:latest assets/docker/gateway` under the lock, base `nginx@sha256:a8b39bd9...` from the local cache, no network step. There was no earlier `candler-gateway:latest` to record or remove. |

## Evidence (stored run: `raw/stdout.jsonl.gz`, `manifest.json`, `workspace/`)

| Outcome | Evidence |
|:--|:--|
| 1 MCP tools | init line `mcp_servers: [{tools, connected}]`; `measured.census.mcp_servers = [tools, connected]`; `census.init_tools` holds `mcp__tools__add` and `mcp__tools__echo`; the agent's `ToolSearch select:mcp__tools__add,mcp__tools__echo` returned both tool references. Workspace holds `.mcp.json` = `{"mcpServers":{"tools":{"command":"python","args":["env/server.py"]}}}`. |
| 2 pypi.org | Bash `urllib.request.urlopen('https://pypi.org/simple/')` printed `200` and `b'<!DOCTYPE html>...pypi:repository-version...'`; verification was on (v0 got `SSLV3_ALERT_*`). Job events: `container.created role=egress` and `container.destroyed role=egress found=recorded`. The egress container, and so its log, is gone with the job; the stored run has no proxy log. A reach to pypi from a job network with no other route is the evidence. |
| 3 example.com | `URLError: <urlopen error [Errno -3] Temporary failure in name resolution>`. The agent's first try with `curl` failed with 127 (not installed). |

## What changed since v0

`git log --oneline 5a98f7fc..3ba9ca28 --merges`:

| Finding | Merge | Effect seen |
|:--|:--|:--|
| F2 (MCP never rendered) | 591b2e78 merge(runner): land the mcp rendering fix and the relative fixture path; 99bd51bd merge(isolation): drop the surface's unread mcp field | `.mcp.json` written with `mcpServers.tools`; server connected |
| F1 (absolute fixture path) | 591b2e78 (same merge) | `candler run` accepts the fixture as shipped, relative `env/server.py` |
| F3 (TLS corrupted) | 84b7891f merge(isolation): forward egress bytes through njs buffer events | real handshake to pypi.org, HTTP 200 |
| Test | 3ba9ca28 merge(runner): drive the mcp guards through the production driver | n/a (not exercised by this smoke) |

## Correction to v0's Diagnosis (F2 section)

v0 says the core's `Surface` reader assembled the MCP object, and that this is why the absolute-path check saw `mcp.servers.tools.args`. That is wrong. The refusal came from the cell-level check `Comparable::from_resolved` (`candler-schemas/src/execution.rs:1990`), which looks at the flat resolved leaf paths. `Surface` has no production caller. The F1 refusal in v0 therefore did not come from `experiment/surface.rs`, and 99bd51bd removing the surface's unread `mcp` field is consistent with that.

## Notes

- The tip works end to end, but three smoke-setup traps cost two unpaid attempts (no model call): (a) a store from a prior run of the same experiment refuses a re-run once the plug-in or the treatment repo changed, with a "re-checked fact differs" message that is clear; (b) the skills source repo must keep the skill at `.claude/skills/<name>/` (the archive pathspec); my first recreation put it at `<name>/` and the job failed with `the cell cannot be planned: the skill tree could not be archived`, which does not name the pathspec it looked for (a small diagnostics gap).
- Still open from v0: F4 (a proxy denial is not recorded as such, outcome 3 stays weak) and the `unrecognized_model` stderr line for `RiVault/Agentic-Medium` (the run completed).

## Docker

| List | Before | After |
|:--|:--|:--|
| images | 18 | 18: three created by the run removed by exact id after checking no container uses them: `11bf3337aba5` candler-ext, `1707fcad137d` candler-gateway:latest, `e8d7ec4c4534` candler-env. `998771cda4ef` and `e2c0dcf461fb` kept. |
| `ps -aq` | 7 | 7, same ids except that another session's `yzq-tb-bix-*` container set changed; I touched none |
| networks | 5 | 5 |

The run store, `experiment.toml`, `plugins/`, `skills/` (recreated, git tag v1) stay under `$HOME/candler-m5b-smoke` as evidence.
