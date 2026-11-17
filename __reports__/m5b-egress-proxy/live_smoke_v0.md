# M5b live smoke: findings (v0)

Date: 2026-11-17. Tip b8d601a2 (`feat/m5b-egress-proxy`), release build. Nothing committed.

## Verdict

Run `01a1e717-d0b8-78cc-bcab-9e5db621c4bb` (run key `618ff87f...-r1-a1`): valid, `ended_by = completed`, exit 0, 12.6 s.
(1) MCP tools listed: **FAIL**. (2) pypi.org fetched through egress: **FAIL**. (3) example.com unreachable: **PASS** (weak: DNS failure, no proof the proxy denied it).

## Setup

| Item | Value |
|:--|:--|
| Model | `RiVault/Agentic-Medium` (DeepSeek-V4-Flash), endpoint = RiVault base URL, secret `RIVAULT_LLM_INFERENCE` |
| Driver | `claude-code` in the container, `permission_mode = bypassPermissions` |
| Command | `lockf -k /private/tmp/claude-501/m5b-docker.lock candler run $HOME/candler-m5b-smoke/experiment.toml` (cwd a work dir with `candler.toml` `[plugins] dirs`) |
| Plug-in env | copy of `crates/candler-core/tests/fixtures/plugins/agent-env`, environment `tiny` (`isolation.build.plugin = "agent-env/tiny"`, MCP server `tools`, vendored wheel build); `mcp.servers.tools.args` edited, see F1 |
| Egress | `"isolation.egress" = ["pypi.org:443"]` (resolve prints it as `experiment_file`) |
| Cost | `run.cost_usd` 0.148092 (client-side); tokens in 14,761, out 4,094, cache read 35,840; 6 turns, 5 Bash calls |

## Evidence (run store, `raw/stdout.jsonl.gz`, `measured`)

| Outcome | Record evidence |
|:--|:--|
| 1 MCP tools | `measured.census.mcp_servers = []`; `census.init_tools` has no `mcp__*` tool; init line `mcp_servers: []`; the agent ran `claude mcp list`: "No MCP servers configured". Run workspace holds `.claude` and `env/` (the server file was mounted), no `.mcp.json`. |
| 2 pypi.org | `python3 urllib` to `https://pypi.org/simple/`: `ssl.SSLError: [SSL: SSLV3_ALERT_UNEXPECTED_MESSAGE]`; retry with verification off: `SSLV3_ALERT_HANDSHAKE_FAILURE`. No HTTP status, no bytes. The base image has no `curl`. |
| 3 example.com | `URLError <urlopen error [Errno -3] Temporary failure in name resolution>` |
| Egress component | job events: `container.created role=egress` (seq 65) and `container.destroyed role=egress found=recorded` (seq 115); `candler-gateway` and `candler-ext` images were built for the job. |

## Findings (not worked around, except F1 to get a run)

- F1. `candler run` refuses the M5a fixture as shipped: `a cell cannot be built: value at mcp.servers.tools.args holds an absolute path (a file enters a cell as an input digest)`. The fixture's `/work/env/server.py` (a container path, valid for the core-level e2e that never builds a cell) is refused. To proceed I changed the scratch copy to `env/server.py`. A plug-in environment with an MCP server cannot be expressed with a container-absolute path through the CLI.
- F2. Even with the relative path, the Claude Code session saw no MCP server (F2 evidence above). `resolve` listed `mcp.servers.tools.*` as derived from the plug-in, and no `.mcp.json` is in the run workspace. Cause not diagnosed (the driver's `render_mcp` writes the object at its `mcp` root as `mcpServers`; whether `mcp.servers.<name>` yields `mcpServers.servers.<name>`, or the file was never prepared, is unchecked). This is outside B5 but blocks outcome 1.
- F3. With `isolation.egress = ["pypi.org:443"]` the TLS handshake to pypi.org failed with an SSL alert instead of succeeding, so the allowed host was not reachable from the agent. Not diagnosed (proxy log not read; budget). Candidates: proxy handling of CONNECT from urllib, or an agent-side proxy variable. Needs the egress container's log, which was destroyed with the job.
- F4. The proxy denial for example.com is not recorded as such in the run: the agent only saw a name-resolution failure, and `inspect` shows no egress census or denial counter.
- Minor: stderr holds `[claude-code:unrecognized_model] RiVault/Agentic-Medium`; the run still completed.

## Docker

| List | Before | After |
|:--|:--|:--|
| images | 16 | 19 after the run; 3 created by it removed by exact id (`5f47f6aee140` candler-env, `7332f4977dd8` candler-gateway, `f7206f62054f` candler-ext) -> 16, the four kept ids untouched |
| `ps -aq` | 3 | 3 (job destroyed its containers) |
| networks | 5 | 5 |

## Where things are

Run store kept for evidence: `$HOME/candler-m5b-smoke/work/.candler/store` (run `.../runs/618ff87f...-r1-a1`, job `.../jobs/01a1e717-...`), plus `state/`, `experiment.toml`, `plugins/` there. Deleted: `/private/tmp/claude-501/m5b-target-smoke`, the skills repo.

## Diagnosis (nothing fixed)

### F3: the egress proxy corrupts every TLS handshake (root cause confirmed)

Cause: `assets/docker/gateway/egress.js` (`guard`, the `s.on('upload'...)` / `s.send(data, flags)` pair) forwards the stream through njs as a string. In njs 0.9.6 (nginx 1.28.3) the `data` of a `js_filter` callback is a UTF-8-decoded string, so every non-UTF-8 byte of the TLS ClientHello is replaced by U+FFFD before `s.send` re-encodes it. The upstream (pypi.org) gets a damaged hello and answers with a 7-byte TLS alert; the client reports `SSLV3_ALERT_UNEXPECTED_MESSAGE` or `..._HANDSHAKE_FAILURE`. The leads (IPv6, multiple addresses, `proxy_next_upstream off`) are not the cause: `resolver ... ipv6=off` returned three IPv4 addresses and `$upstream_addr` was `151.101.x.223:443` at every callback, `refused()` was false.

Reproduction without the agent or a model (Docker under the lock, own ids removed): an `egress` proxy with the rendered server for `pypi.org:443` on a user bridge, and a client from the kept base image (`998771cda4ef`) with `--add-host pypi.org:<proxy ip>`, `python3 urllib`.

| Variant | Result |
|:--|:--|
| as shipped (`js_filter egress.guard`) | `SSLError ... UNEXPECTED_MESSAGE` / `HANDSHAKE_FAILURE`, same as the run |
| no `js_filter` line | `200`, body returned |
| shipped js with a log line | first upload frame: `len=510 ge128=52 fffd=47 gt255=52` (47 replacement characters), then `DOWN len=7` (the alert) |
| `s.buffer_type='buffer'` | no effect, `typeof data` is still `string` |
| `s.sendUpstream()` / `s.sendDownstream()` instead of `s.send(data)` | handshake times out (no working forward) |

The proxy's own error log shows nothing at level `warn`, which is why the job gave no hint. The image `candler-test-gateway:5e96a424a04c` (kept, id 01d0c306e9f6) holds a different `egress.js` than the repo, so I rebuilt the repo's assets into a throwaway image for the js variants; the first two table rows ran on the kept test image, the three js rows on the rebuilt repo assets (the failure is the same on both).

Smallest fix I would make (untested beyond the table): the address check must not sit in the data path. Candidates, in order: (1) keep the check, but drop the data callbacks: do it once with `js_access`/`js_preread` on a resolved address (resolve the upstream name in njs or via a `js_set` variable) so nginx forwards raw buffers itself; (2) if the check has to follow the connect, find the njs call that hands over a Buffer (not found in 0.9.6 in the time available) rather than a string. I did not find a working variant, so the fix needs a short spike, with a real TLS client as the test (the leaf's tests passed because they used plain text or refused cases).

Bug age: new in M5b. `egress.js` was added by 1904f9a1 (feat(isolation): forward listed hosts and ports through the egress proxy), the `egress.conf.template` was last changed by be99e2e6 (2026-11-17); the proxy is an M5b leaf. Note `main` is 633 commits behind this branch, so "on main" is not a useful comparison.

### F2: the plug-in's `mcp.servers.tools.*` never becomes a `.mcp.json` (root cause confirmed by reading)

Cause: two independent mismatches, the first one fatal.
1. Shape. The cell holds the plug-in's values as flat leaf paths (`mcp.servers.tools.args`, `mcp.servers.tools.command`, as the run manifest's `comparable` lists them). `render_mcp` (`crates/candler-harness-claude-code/src/prepare.rs:79`, `value_at(cell, MCP)` with `MCP = "mcp"`, `candler-schemas/src/execution/paths.rs:25`) goes through `Cell::get` (`crates/candler-harness/src/spec.rs:1252`), which is an exact-path lookup. No resolved value has the path `mcp`, so it returns `None` and no file is prepared. Evidence: the run workspace holds `.claude/settings.json` (`{}`) and `env/` and no `.mcp.json`. The core's `Surface` reader (`experiment/surface.rs`, `reader.object(paths::MCP)`) does assemble the nested object from leaves, which is why the absolute-path check saw `mcp.servers.tools.args`; the driver does not use it.
2. Key level. Even with an object, `prepare.rs:86-90` writes the object at `mcp` as `mcpServers` ("each server as given"). A plug-in value path `mcp.servers.<name>.*` would give `mcpServers.servers.<name>`, a server named `servers` with no command, not `mcpServers.<name>`. The manifest/plug-in contract (`mcp.servers.tools.*`) and the driver's mapping disagree by one level.

Smallest fix: in `render_mcp`, read `mcp.servers` through the same nested-object reader the core uses (or collect every cell value with the prefix `mcp.servers.` into an object) and write that object as `mcpServers`; add a driver test with leaf-path cells (the existing driver tests do not exercise it). Claude Code also needs the project server approved in `-p` mode (`--mcp-config` or `enableAllProjectMcpServers`); I have not verified this, check it when fixing.

Bug age: not M5b. `render_mcp` came with eed45a00 (2026-11-06, M2b) and was last touched by a5b68318, a refactor; no egress commit touches it. M5a's guard test never ran `candler run` (it builds the `Fragment` and renders the file itself with its own `render_mcp` helper in `plugin_env_e2e.rs:215`), so the gap was invisible until this live run.

### F1: absolute path refused (fixture defect with a product gap)

The refusal is the core's long-standing rule (`SurfaceError::AbsolutePath`, `experiment/surface.rs:184`, added by ce926c9f on 2026-11-06): a path in a value is a file, files enter a cell as digests. The fixture (4e5f3509, 2026-11-16) writes `/work/env/server.py`, a container path, which the rule forbids; its test does not build a cell, so nothing caught it. So the fixture is wrong for `candler run`. The product gap: the refusal does not say that a container path is not expressible, and nothing says what the MCP command's working directory is (my relative `env/server.py` resolves only if the server starts in `/work`, unverified because of F2). Smallest fix: change the fixture to a relative path (or an `mcp` args convention agreed with the driver) and make the refusal name the plug-in as the source.

### Docker after the diagnosis

images 16 (the same ids as at the start), `ps -aq` 3, networks 5. Six throwaway images (`m5b-diag-gw`) and the network `m5b-diag-net` created and removed by exact id; the proxy container removed each round.
