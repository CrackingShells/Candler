# plugin_environment step 3 report (findings)

Base d6389815. Step 3 is built without the catalog assertion (coordination ruling on F1).

## 1. Commits
| sha | subject |
|---|---|
| 4e5f3509 | test(isolation): serve live MCP tools from a plug-in environment |
| (this) | reports(isolation): record the plugin_environment step 3 leaf |

## 2. Gates
| gate | result | last line |
|---|---|---|
| SPAWN | PASS | 9 tests run: 9 passed |
| STD (`-p candler-core`): fmt, clippy -D warnings, nextest, doc -D warnings | PASS | 287 passed, 19 skipped |
| LINUX | PASS | clippy x86_64-unknown-linux-gnu, rc 0 |
| SUBJ | PASS | cog check and length rc 0 |
| E2E (`guard_container_e2e::` and `guard_cli_e2e::`) | PASS | 16 tests run: 16 passed |
| the leaf's check, `plugin_environment_serves_live_mcp_tools` | PASS | 1 passed (3 s) |
| OWNED | PASS | the diff holds the fixture, `plugin_env_e2e.rs`, the one `mod` line and this report |

## 3. Guard
`guard_container_e2e::plugin_env_e2e::plugin_environment_serves_live_mcp_tools`
- Guards: R06 D4, R01 section Agent environment: a container built from a plug-in environment runs its MCP server from its inputs; the agent lists exactly its tools; the evaluator-only file is absent by name and bytes; everything built is removed by id.
- Oracle: the tools of `agent/server.py` written by hand in the test (names, schemas, the `tinydep-1.0` tag only the extension's wheel supplies); the canary read on the host; `docker inspect` of each id after removal.
- Bites, each injected from a copy and restored from it, each red:
  | bite | failing message |
  |---|---|
  | extension left out of the build | client failed, code 1: `ModuleNotFoundError: No module named 'tinydep'` from `/work/env/server.py` |
  | inputs not mounted | client failed, code 2: `can't open file '/work/env/client.py'` |
  | resolved `mcp.*` dropped from the client's config | client failed, code 1 (the server it launched printed no JSON-RPC) |
- No bite for the canary scan (a mounted canary); the scan is only by name and bytes over every regular file under 1 MiB outside /proc, /sys, /dev.

## 4. Claims checked
| claim | code showed |
|---|---|
| fixture path `tests/fixtures/plugins/agent-env/` | as the leaf says; a `plugins.dirs` entry is the plug-in directory itself (holds `plugin.toml`), not its parent |
| `DockerIsolation::plan` on the resolved cell | not used: `plan` needs a `Cell`, directories and a gateway, and inputs are not mounted by `plan` but by `runner/exec` `mount_inputs`. The test builds a `ContainerPlan` as `create_e2e.rs` does and mounts the resolved inputs the way `mount_inputs` does (read-only store blobs at `/work/<path>`), through `run_request` and `collect_and_destroy` |
| agent inputs mount as directories | they are single files, each a read-only blob at `/work/<at>/<file>` over an empty mount point |
| env layer installs the dependency | no: `--no-build` on a lock; the extension installs the vendored wheel with `--no-index`; the base, env layer and extension all come through `DockerImageService::ensure` |

## 5. Findings
- **D1 (deviation, ruled).** No catalog assertion (F1).
- **D2 (deviation).** `plan` is not called; see section 4. A plan-level guard of the input mounts is `runner/exec`'s.
- **D3.** The extension Dockerfile needs `USER root` and `USER agent` around its install; the env image ends as `agent`.
- **D4.** The wheel (`tinydep`, 953 bytes) was written by hand with `zipfile`; it is a binary fixture, `.gitignore` does not exclude it.
- **D5.** The env layer for an empty lock and the base were already on the daemon, so the run did not reach PyPI; a cold daemon needs it for `uv venv --seed`, as `env_image_e2e` does.

## 6. Docker
Before: 16 images, 3 containers, 5 networks. After: images and containers identical by id; one network id differs each way, created and removed by the sibling's run, not mine (my network is removed by name). Pulled: none. My test builds one image (the extension) and removes it by id.
E2E test name: `guard_container_e2e::plugin_env_e2e::plugin_environment_serves_live_mcp_tools`

Target dir and scratch removed.
