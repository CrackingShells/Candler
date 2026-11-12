# opencode_facts: what OpenCode V2 does in a hardened container

Probe leaf of `m4-session-runner`. OpenCode 2.2.7 (the `anomalyco/opencode` release `opencode-linux-arm64.tar.gz`, 120,326,654 bytes, sha256 `c1682057...20e7`, checked by `ADD --checksum` in a scratch image) ran under Candler's container hardening, through the production gateway sidecar, against a scratch copy of the fake model scripted for OpenCode's tools. The fixtures and every answer below come from the fake model: no token went into them. Afterwards Eliott authorised 3 real-model runs (Reasoning-Small, through the gateway sidecar); they are reported in their own section, and no fixture comes from them. Only the one authorised file was fetched, only inside the image build (the rebuild after the first teardown took the layer from the BuildKit cache, no second download).

## Census: no event and no command gives the agent's tool inventory

D9's census rule goes to the coordination session before L1.

| Where I looked | Result |
|:--|:--|
| NDJSON events of `run --format json` | Six kinds seen (`step_start`, `text`, `tool_use`, `step_finish`, `retry`, `error`); none lists tools. |
| `session export` | `info` and `messages` only; no tool list. |
| `debug agents`, `GET /api/agent` | Permission rules per agent (`build`, `plan`, `explore`, `general`, `compaction`, `summary`, `title`), not tools. |
| The 170 operations of the server's OpenAPI (`GET /openapi.json`) | No tools endpoint. `GET /api/skill`, `/api/mcp`, `/api/command` list skills, MCP servers (name, status) and commands only. |
| The request OpenCode sends the model | **Yes.** The Anthropic request's `tools` array is the inventory: `edit glob grep question read shell skill subagent webfetch websearch write execute` (12), plus one direct tool per non-code-mode MCP server tool (`probe-plain_ping`). Code-mode servers appear only inside the `execute` tool's catalog text in the system prompt. |

So `capabilities.census = false` by the D9 default, unless the coordinator wants a wire source: the gateway sidecar does not log bodies, so a body tap would be new work. `GET /api/skill` and `GET /api/mcp` give skills and MCP servers while the in-container server runs, which covers part of an inventory but not the built-in tools. The decision is the coordinator's.

## Verdict

| Question | Answer |
|:--|:--|
| Binary | `opencode`, one file (253,873,144 bytes, sha256 `c577783e...a425`), `--version` prints `opencode v2.2.7`. |
| Run form | `opencode run --standalone --format json --auto -m <provider>/<model> <prompt>`; `--standalone` is a global flag (it also applies to `session export`, `api`, `models`). |
| Stop | SIGTERM and SIGINT both end the run gracefully in under 100 ms, kill the server child and the running tool, and the session is stored as `interrupted`. The in-container interrupt works too. |
| Export | `session export --standalone <id>` works after any ending, including signals, and carries `outcome`. |
| Network | The only outbound attempt: `GET https://models.opencode.ai/experimental/v2.0/api.json?type=chat,decision,image`, at every server boot; the internal network blocks it and the run goes on. |
| Real model | 3 runs spent (budget used up): completed plus export confirmed; SIGTERM landed in the model's reasoning, not in a tool; the in-container interrupt on a real model was **not** exercised (my script error, below). |

## Probe answers

Clock and times are the container's. Cases are in `crates/candler-harness-opencode/tests/fixtures/streams/`.

| # | Question | Evidence | Decision it feeds |
|:--|:--|:--|:--|
| 1 | Binary name, version | `opencode --version` -> `opencode v2.2.7`, exit 0 | version probe: parse `opencode v<semver>` |
| 2 | `run` flags: standalone, format, port, budgets | `run --help`: `--continue --session --fork --worktree --model --agent --format {default,json} --file --title --archived --thinking --auto`. **No `--port`, no turn or cost flag.** `--standalone` and `--server` are global flags. | driver forces `run --standalone --format json --auto` |
| 3 | Budget | Only config: `agents.<id>.steps` (OpenAPI `Config.AgentEncoded.steps`). With `agents.build.steps = 1` the model request carried no shell tool and the run ended in text. Exit status of that run was not recorded. | turn cap = a `steps` value in the overlay; no cost cap exists |
| 4 | Export command | `session export [--sanitize] [<id>]`, JSON on **stdout** (no file). Unknown id: `Session not found: ses_doesnotexist`, exit 1. `session list` prints `id<TAB>title<TAB>date`. | D11: collect with `session export <id> --standalone`, framed `Document` |
| 5 | Export shape | `{info:{id,projectID,model,cost,tokens,outcome,time,title,location}, messages:[user, assistant{content:[text,tool{id,name,state}],finish,rawFinish,cost,tokens,error?}, ..., idle{outcome,error?}]}`. `outcome` is `succeeded`, `interrupted` or `failed`; the closing `idle` message repeats it. | export is authoritative for the trace; `outcome` classifies the end |
| 6 | `debug paths` under isolation | With `HOME`, `XDG_{DATA,CONFIG,CACHE,STATE}_HOME` and `OPENCODE_DB=/home/agent/.xdg/data/oc.db`: data `$XDG_DATA_HOME/opencode`, config `$XDG_CONFIG_HOME/opencode`, cache, state, log `<data>/log`, `db` = the `OPENCODE_DB` file path, `tmp` = `/tmp/opencode`. With HOME only: `~/.local/share/opencode/opencode.db` etc. The DB runs in WAL mode (`-wal`, `-shm` beside it). | `state_paths()`: four XDG dirs, `OPENCODE_DB` as a File; `/tmp/opencode` needs only the noexec tmpfs |
| 7 | Provider with a run-time base URL | `$XDG_CONFIG_HOME/opencode/opencode.json` with `provider.<id> = {npm:"@ai-sdk/anthropic", options:{baseURL:"{env:OC_BASE_URL}", apiKey:"{env:OC_API_KEY}"}, models:{<m>:{}}}`; `{env:VAR}` is substituted at load (`debug config` shows the value, the key as `***`). V1 key names (`provider`) are accepted and normalised to V2 (`providers`, `mcp.servers`). Offline, the bundled `@ai-sdk/anthropic` loads: no npm fetch. `OPENCODE_CONFIG_CONTENT`, `OPENCODE_CONFIG_DIR` also exist in the binary (not exercised). | D9: a provider config that `prepare` writes from the run-time URL (no `GatewayEnv` env pair reaches OpenCode); the gateway URL is `http://gateway:8080/r/<tag>/run/<key>/v1` |
| 8 | Client for the interrupt inside the image | No `curl`, `wget`, `nc`. `python3` is there; so is `opencode api`. | StopSpec pre-stop `Action::Exec` can run `opencode api`; no new tool in the image |
| 9 | Interrupt endpoint | `opencode run --standalone` spawns a child `opencode serve --stdio --port 0` on a random loopback port with Basic auth. The child's `OPENCODE_PASSWORD` is random and **replaces the one Candler sets**; `OPENCODE_SERVER_PASSWORD=...` set by the caller was rejected (401). Port and password were found from inside the container: the port from the child's open socket in `/proc/net/tcp`, the password from `/proc/<child>/environ` (same uid). Then `opencode api --server http://127.0.0.1:<port> session.interrupt --param sessionID=<id>` (= `POST /api/session/{id}/interrupt`) answered `{"interrupted":true}` in 80 ms. | D9 `{session_id}` placeholder works, but the exec must first find port and password; a single exec script can do both. The password cannot be chosen. |
| 9b | Stop by interrupt | After the call: the tool's `tool_use` event (status `error`, "Tool execution interrupted"), `step_finish` reason `error`, then `{"type":"error","error":{"type":"aborted","message":"Session interrupted: user"}}`; **exit status 1**; run over in 125 ms; server child and tool gone. Export: `outcome: interrupted`. | I22 pre-stop action = this exec; exit 1 alone cannot tell it from a failure |
| 10 | SIGTERM, then SIGINT | `timeout --preserve-status -s TERM` at 8 s, shell call running: exit **143**, stdout stops after the last event (no error event), log `shutdown started signal=SIGTERM`, the server child and the tool die with it (process list empty after). SIGINT: exit **130**, same. Export: `outcome: interrupted`, the running call as `error`. TERM is not ignored, so no INT escalation is needed. | `classify_exit`: 143 and 130 = stopped by Candler's signal |
| 11 | Event kinds and where the session id appears | `step_start`, `text`, `tool_use`, `step_finish`, `retry`, `error` (`reasoning`, sub-agent and permission events not observed). Every event carries `sessionID`; it first appears on the **first stdout line** (`step_start`), or on the `error` event when the run fails before a step (unknown local model). A `tool_use` event comes **only after the call completed** (or was interrupted); a running call has no event. `step_finish.reason` is `tool-calls`, `stop` or `error`; its `tokens` are per step. | the live NDJSON gives the session id and early events; tool calls come from the export |
| 12 | Failure: unknown model | Not in the provider list: `error {type:"provider.no-route", message:"Model unavailable: fake/zzz-not-configured"}`, exit 1, no request left. Known to OpenCode, refused by the provider (fake 404): `step_start`, then `error {type:"provider.invalid-request", status:404, response.body}`, exit 1. | |
| 13 | Failure: refused key | Wrong placeholder key, gateway 401: `error {type:"provider.auth", status:401, message:"not the job's token"}`, exit 1. | |
| 14 | Failure: unreachable endpoint | Closed port: 10 `retry` events (attempts 2 to 11, `nextRetryAt` from 2.2 s to 12 s, `provider.transport`, "ECONNREFUSED"), then `error provider.transport`, exit 1, 84.7 s. Unresolvable host: still retrying at 100 s (7 `retry` events, waits up to 30 s), SIGTERM ended it, exit 143, no terminal event. | a transport failure is slow: the run's wall clock cap, not OpenCode, ends it |
| 15 | `classify_exit` classes | 0 completed (`succeeded`); 1 for every error: `failed`, interrupted through the API, `aborted`; 130 and 143 signals. **Exit 1 is three different endings**, told apart only by the last `error` event's `type` or the export's `outcome`. | see below |
| 16 | Skill layout | `GET /api/skill` listed, besides the built-ins `opencode`, `report`, `browser` (`browser` is denied for `build`): `<project>/.opencode/skills/*/SKILL.md` and `.opencode/skill/*`, `<project>/.claude/skills/*`, `<project>/.agents/skills/*`, `$XDG_CONFIG_HOME/opencode/skills/*` (and `skill`), **`$HOME/.claude/skills/*` and `$HOME/.agents/skills/*`**, and every directory in config `skills: [dir]`. Frontmatter `name`, `description`. | SkillTree: write to a directory and name it in `skills`, or use `<project>/.opencode/skills`. Isolation must keep HOME per run: OpenCode reads Claude Code's skills in HOME. |
| 17 | MCP code mode per server | V2 shape `mcp.servers.<name>.codemode` (boolean; `mcp.<name>` of V1 is normalised). Omitted = **on**: the server's tools are in `execute`'s catalog and not direct tools. `codemode:false` = direct tool `<server>_<tool>`. `execute` is offered even with no MCP server. | a cell value under `mcp.servers.*.codemode` (I7), as D11 says; absent must be recorded as on |
| 18 | Other effects | One tool-less request per session (the title agent, same model) makes `info.tokens` = 3 requests (300 in / 60 out) while the stream shows 2 steps (200 / 40). Without `--standalone` a run also works but writes `service.json` (password, pid) in the config and state dirs. | token and cost accounting; reason for `--standalone` |

## `classify_exit`: exit code classes

| Class | Backed by | Fixture |
|:--|:--|:--|
| Completed (exit 0, `succeeded`) | recorded | `completed_o1` |
| Stopped by SIGTERM (143) or SIGINT (130), `interrupted` | recorded | `sigterm_mid_tool`, `sigint_mid_tool`, `unreachable_dns` |
| Stopped by the in-container interrupt (1, `aborted`, `interrupted`) | recorded | `interrupt_api` |
| Provider refused model / key (1, `provider.invalid-request`, `provider.auth`, `provider.no-route`) | recorded | `unknown_model_remote`, `refused_key`, `unknown_model_local` |
| Transport failure after retries (1, `provider.transport`) | recorded | `unreachable_refused` |
| Step cap (`agents.*.steps`) | observed once, exit status not kept, no fixture | none |
| Out of memory, rate limit (429), 5xx, permission denial, sub-agent, reasoning | **documentation or guess only** | none |

## Findings

| # | Finding | Flag |
|:--|:--|:--|
| F1 | No source of the tool inventory except the model request. | **Coordinator, before L1: D9 census rule.** |
| F2 | The leaf expected `--port` and turn or cost budgets on `run`: neither exists. The turn cap is `agents.<id>.steps`; no cost cap. | Spec defect (leaf text); D11 ("stops with an in-container interrupt") still holds. |
| F3 | R01 says code mode is "on by default": confirmed, and the V2 path is `mcp.servers.<name>.codemode`, not `mcp.<name>`. | R01 wording to refine. |
| F4 | The server password is generated by the child and overrides `OPENCODE_PASSWORD`; the interrupt exec must read port and password from `/proc` of the `serve --stdio` child. | Driver design input. |
| F5 | `$HOME/.claude/skills` and `$HOME/.agents/skills` are read by OpenCode. | Isolation: HOME per run. |
| F6 | `session export` needs the same `OPENCODE_DB` and `--standalone` as the run; it prints to stdout. | Driver input. |
| F7 | Scratch deviations: the fake model is a scratch copy (kept outside the repo); one extra script `o1`/`o2`/`nomodel1`. The committed `assets/docker/fakemodel/` is untouched. The scripts answer OpenCode's tool name `shell` (not `bash`). | Deviation, no shared name changed. |
| F8 | Signals were sent by in-container `timeout`, not `kill`: its `--preserve-status` shows the child's own status. | Method note. |
| F9 | Docker network ids before and after differ by one built-in id (`bridge`, not mine); images, containers, volumes are equal. The BuildKit cache still holds the download layer (no prune allowed). | Note. |
| F10 | Not tried: reasoning output (`--thinking`), sub-agents, permission prompts without `--auto`, `--continue`/`--session` resume, Linux CI timing. | What the real-model runs check. |

## Real-model runs (Reasoning-Small, 3 of 3 used)

Health check first: `rv_health.py` passed (network, 5 families); `RiVault/Reasoning-Small` was listed (live backing `Qwen/Qwen3.8-27B`). Route: agent, gateway sidecar (`GW_UPSTREAM_AUTHORITY=api.class2.llm.ai.r-ccs.riken.jp`, system CA bundle, the key piped from `$RIVAULT_LLM_INFERENCE` into the sidecar's tmpfs, the agent held a placeholder), OpenCode provider `@ai-sdk/openai-compatible` (loads offline). The key was never written to disk; a search of the run outputs and the OpenCode home for its value found nothing. Gateway log: 8 `POST /v1/chat/completions`, all 200 (3 title calls, 5 steps). No fixture was stored: the fake drove every case.

| Run | What happened | Differences in shape from the fake runs |
|:--|:--|:--|
| 1 completed | exit 0, 2.4 s, `succeeded`; `session export` worked. | The model made its first step with **no `text` event before `tool_use`** (reasoning only); tool call id is `chatcmpl-tool-<hex>`, not the provider's `toolu_`; `step_finish.cost` is non-zero (0.00054, 0.00013) and `tokens` carry `reasoning` and `cache.read` (4704 on the second step: OpenCode's prompt prefix is cached at the provider); `text` begins with `\n\n`; `rawFinish` is `tool_calls` and `stop` (OpenAI wording). **`reasoning` events do not appear on stdout** without `--thinking`, but the export holds `reasoning` content blocks (`content: [reasoning, tool]`, `[reasoning, text]`). `info.title` was null on the completed export. Same six event kinds, same keys otherwise. |
| 2 SIGTERM | At 14 s the model was still reasoning (270 reasoning tokens, no tool yet), so the signal did **not** land mid-tool. Exit 143, stdout: `step_start` only, no terminal event; export `outcome: interrupted`, the assistant message `finish: error`, `error {type:"aborted", message:"Step interrupted"}` (the fake runs gave "Tool execution interrupted" for a tool). Server child gone. | A real model can spend more than 14 s before its first tool call: a stop test needs to wait for the tool process, not a timer. The mid-tool SIGTERM on a real model is **not confirmed**. |
| 3 interrupt | **My script error:** the wait for the tool process matched the wrapper shell's own command line (it contains the prompt text), so it fired at once, before a session or port existed; the interrupt call failed (`opencode api` printed its usage, rc 1) and the run then completed by itself in 102.8 s (`sleep 100`, tool call input `{command, timeout: 150000}`, exit 0, `succeeded`). | The in-container interrupt with a real model is **not confirmed**; the budget is spent and no fourth run was made. The fake-model interrupt (case `interrupt_api`) stands. |

Client-side cost (OpenCode's own `info.cost`, not a RiVault bill): run 1 0.00067, run 2 0.00021, run 3 0.00034, about 0.0012 in all; run 3 held 9408 cached input tokens, 2127 uncached.

What stays open for a later real-model run, if Eliott grants one: the mid-tool SIGTERM (wait for the tool process by an exact match, for example `/usr/bin/bash -c` plus the command text), and the interrupt call with a real session; both are recorded with the fake and behave as the API says.

## Gates

| Gate | Result |
|:--|:--|
| Stub crate member and builds | PASS (`cargo metadata ... select(.name=="candler-harness-opencode")`, `cargo build -p candler-harness-opencode` Finished) |
| `cargo nextest run -p candler-harness-claude-code --no-tests=fail` | PASS (10 passed) |
| PROVENANCE has `version` and `sha256` | PASS |
| Each stream has `stdout.jsonl`, `exit.toml`, `expected.toml` | PASS (prints nothing); 9 cases, the count gate passes |
| Docker image, container and volume lists equal before and after | PASS |
| Report keywords, SUBJ, OWNED | PASS (run after the report commit) |

Docker before: 12 images, 3 containers, 33 volumes. After: the same ids. Created and removed by exact id, in two rounds (fake round; real-model round): images `7e22a5f24742`, `82694e15975f`, `e89d81126e93`, then `f8fd6cff09e4`, `aa2ebf00b6ce`; containers `71b168d4d0a4`, `8b1a3b403bcf`, then `4f0f79541eac`; networks `c4a58410a1d0`, `d2050fda7a79`, then `883cff06c1ce`, `ca3b2f430131`. One mistaken `docker rmi` with a network id failed harmlessly (no such image).
