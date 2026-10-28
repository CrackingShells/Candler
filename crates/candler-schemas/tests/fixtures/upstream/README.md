# Upstream catalog snapshots

Two tool-catalog snapshots captured from real MCP servers, in HTN's `tool-catalog-snapshot/0`
shape (`servers_dir` and `stubbed_modules` at the top level). They are inputs to the catalog tests,
which convert them to the `/1` shape in the test; the files themselves are verbatim.

Source repository: `mcp-python-ingestion` (`/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion`),
licence Apache-2.0.

## `rikyu_mcp.json`

| | |
|---|---|
| Tag | `htn-creator-eval@0.4.2` |
| Commit | `28be2c158fbc6ee786a789242c2d8fd47bfde9a8` |
| Path | `.claude/skills/htn-creator-workspace/infrastructure/htn_creator_eval/catalogs/rikyu_mcp.json` |
| Blob | `c3a1206abf4507cab53cd3f41ff6ffa4967e1757` |
| SHA-256 | `8e331d938505087e34839ba8d8d0b596032041af2dd1710315684c8d39f5b438` |
| Servers | `rikyu-hpc` (26 tools), `rikyu-docs` (3 tools) |
| Captured | `2026-10-28T01:09:04+00:00`, python 3.12.12, hpc-agent-core 0.5.0, mcp 2.0.0, pydantic 2.13.4 |

Copied byte for byte (`git show 'htn-creator-eval@0.4.2:<path>'`). Server sources, at the same
commit, under `.claude/skills/htn-creator-workspace/servers/rikyu_mcp/`:

| Source | Blob | Digest recorded in the snapshot |
|---|---|---|
| `hpc_server.py` | `60baf22df5deaaf46e18ac7febe6a964ebce3727` | `sha256:b2af8a23ff6f5ceef36217b7727bf515fbf345472d81e1872bbed43b57e55b4b` |
| `docs_server.py` | `0b75ec4fb499ebe98d95d70aa5285c8f367a9d1d` | `sha256:527acd8f431571903257dd51578b2a21bf8bb27b20d274e6fdfe82169968d638` |

The capture command is in the docstring of the capture script (below).

## `bio_opentrons.json`

| | |
|---|---|
| Servers | `gripper-server` (1 tool), `liquid-server` (10), `module-server` (19), `movement-server` (6) |
| Server sources | `.claude/skills/htn-creator-workspace/servers/bio-opentrons/` at tag `htn-creator-eval@0.4.2` (`28be2c158fbc6ee786a789242c2d8fd47bfde9a8`) |
| Captured | `2026-10-28T15:10:36+00:00`, python 3.12.12, mcp 1.28.1, pydantic 2.13.4 |
| SHA-256 | `e049a47a610a69b8d407af169f7467fa3d5a5dfb2b999de4d0f8e8be0f0db35f` |
| Capture script | branch `feat/htn-eval-capture-lowlevel-servers`, commit `93ded966e710a6342efc66973c1af4fad48002a7` of `mcp-python-ingestion` (unpushed, not landed) |

The four servers use the low-level `mcp.server.Server` with `@app.list_tools()`, which needs mcp 1.x:
mcp 2.0.0's `Server` has no `list_tools`. mcp 1.28.1 and pydantic 2.13.4 are the versions the
repository's `infrastructure/uv.lock` pins and the golden regrade's `PROVENANCE.json` records. The
script at the tag awaits `server.list_tools()` and cannot read these servers; the commit above makes
it call the registered `tools/list` handler of a low-level server, and its output for the rikyu
servers is unchanged. Server sources, under the path above:

| Source | Digest recorded in the snapshot |
|---|---|
| `gripper_server.py` | `sha256:13b8472ce71cc074dfa02bcc12d4af3b4a4915eb55f5c18c1c8b604da6d53ade` |
| `liquid_server.py` | `sha256:a58d9c9abf64429005f79ba06142c1be939c8a18f0438ed764d123a31c90d99c` |
| `module_server.py` | `sha256:62f0ea336fc5a196d5180fbecea65f1b59d01723553c334febf1113c977730df` |
| `movement_server.py` | `sha256:47a2f81337dedb6ebe31a811db55cfb339a41857a3357c3842d57ce88b2493cb` |

Command, from the root of a `mcp-python-ingestion` worktree at that commit, with `<out>` the path of
this file:

```
uv run --isolated --no-project --python 3.12 \
    --with 'mcp==1.28.1' --with 'pydantic==2.13.4' \
    python -I .claude/skills/htn-creator-workspace/infrastructure/scripts/capture_catalog_snapshot.py \
    --servers-dir .claude/skills/htn-creator-workspace/servers/bio-opentrons \
    --server bio-opentrons.gripper_server:app --server bio-opentrons.liquid_server:app \
    --server bio-opentrons.module_server:app --server bio-opentrons.movement_server:app \
    --pin mcp --pin pydantic \
    --out <out>
```

## Capture script

`.claude/skills/htn-creator-workspace/infrastructure/scripts/capture_catalog_snapshot.py`, blob
`a7f07f1886301c788143b7f7b1d75ffb072dc963` at tag `htn-creator-eval@0.4.2`. It keeps `name`,
`description`, `inputSchema` and `outputSchema` of each tool and sorts tools by name. The bio-opentrons
tools carry no other field.
