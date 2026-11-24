# M6c Integrity, level 3: harness parity

## Context

The P7 dogfooding pass ran an experiment with an MCP server under OpenCode and got two results. The run record did not name the server. The stored catalog snapshot said `servers: []`. The cause is in Candler. The snapshot comes from a constant program (`CATALOG_PROGRAM`, `commands/preflight.rs:42`), and the CLI tests' fake docker prints the same constant (`tests/it/support/cli_run.rs:168`). OpenCode's gateway census never reports servers (`census_init` passes `None`, `gateway.rs:1264`). Behind this sits a larger gap: nothing says, for each harness, what happens to each setting an experiment can make. `settings_table()` is optional, only OpenCode fills it, and the capability page has 8 hand-written rows.

This is the yardstick, quoted verbatim from the README purpose paragraph:

> - **Execution:** run agents under controlled conditions (experiment matrix, runners, isolation, per-run tool-catalog snapshots, provenance).

Running a cell on two harnesses is "controlled conditions" only if both harnesses get the same settings, or each harness says why it cannot. The provenance is truthful only if the run record names the tools and servers the agent actually had.

**Eliott's rulings**, binding on this plan:
- Claude Code is the reference harness. OpenCode is aligned to it. A defect found on the Claude Code side is fixed in both drivers.
- The README phrase becomes **"per-run tool census"** (asked in this session).
- **`codemode` defaults to `false`** (asked in this session). `codemode = true` becomes an explicit OpenCode-only option, and Claude Code refuses it. Resolving writes the default into the cell.

## What the study settled

| Question | Answer | Evidence |
|---|---|---|
| Is there already one configuration vocabulary? | Yes: `CONFIGURATION_PATHS: [PathRow; 49]`, which the resolver, the request-key check, the typed readers and the contract pages all read. Level 3 extends it and adds no second table. | `candler-core/src/resolve/paths.rs:120` |
| Claude Code hooks vs `--setting-sources project,local` | Hooks are written only in session mode, to `<agent home>/.candler/settings.json`, passed with `--settings`, and they only observe. `--settings` is a source of its own, outside `project,local`. No experiment path sets hooks. P8 cites the Claude Code settings doc for this. | `candler-harness-claude-code/src/session.rs:99-117` |
| Where do `inputs` and `env` go? | `inputs` are mounted read-only into the workspace (`runner/exec/mod.rs:215 mount_inputs`). `env` becomes the harness process environment, minus reserved names (`resolve/steps/env.rs`, `isolation/container/run.rs:78`). Both are Candler-provided on both harnesses. | |
| Does anything write CLAUDE.md or AGENTS.md? | No. colgrep finds no hit in `crates/`. A treatment or input can still carry one into the workspace, and each harness then reads its own file. P9 probes what each harness reads. | colgrep, 0 hits |
| Does OpenCode expose server names? | Configured names: `opencode debug config` prints `mcp.servers`. Live status: `mcp.list` answers inside a session only and is empty before one. Under code mode, tool names reach the gateway only as `execute`. P10 settles this with a probe. | probe on OpenCode 2.2.7, image `candler-base:src-b062c666af0a` |
| Defect on the Claude Code side | `render_mcp` serialises `McpServer` verbatim into `.mcp.json`. So `codemode`, `name` and `type = local/remote` (OpenCode words) reach Claude Code unchecked, and `codemode` is dropped without a word. | `candler-harness-claude-code/src/prepare.rs:99-115` |
| Catalog schema | `tool-catalog-snapshot/1` is also a plug-in wire format: catalog providers, Tier D, upstream fixtures. Level 3 removes the **producer** and leaves the schema alone. | `candler-schemas/src/catalog.rs` |

## Design

**One declaration.** The declaration is a typed enum `AgentSetting` in `candler-harness`. It lists what an experiment can set, or what Candler sets, that reaches the agent:
- model id, endpoint and effort;
- permission mode;
- allowed and disallowed tools;
- `env`;
- MCP servers, with sub-entries for transport, `codemode` and `name`;
- skills (from the treatment) and instruction files;
- `inputs`;
- the wall, turn and cost budgets;
- session mode;
- hooks, declared as Candler-owned with no experiment path, so the page says so.

The trait gains a required method `answer(&self, AgentSetting) -> Answer`, with no default. `Answer` is `Mapped { construct }`, `Candler { how }` or `Refused { reason }`, plus value-level answers for settings with a closed set of values: permission modes, effort levels, transports and `codemode`.
- The match is exhaustive, so adding a setting stops every driver from compiling until it answers.
- `settings_table()` becomes the derived table. `check_settings` refuses a set value whose answer is `Refused`, through the existing `experiment.setting_unsupported` code. The refusals OpenCode makes today by hand move into the table.
- `PathRow` gains `agent: Option<AgentSetting>`. A guard checks that every variant is named by a row, or is marked as having no path.

**One record.** The census is the record of what the agent had: `census.tools`, plus `census.mcp_servers` as a list of `{name, status, tools}` on **both** harnesses.
- Claude Code: `system/init` (`parse.rs:172`).
- OpenCode: names and status from `mcp.list` against the run's own OpenCode child, as `CensusSource::Collected` (the variant exists and is unused). Tools from the gateway: `<server>_<tool>`, matched against the configured names, and resolving refuses names that collide once normalised.
- The constant catalog producer goes: `CATALOG_PROGRAM`, `CommandCatalogProvider::capture`, `CatalogStep` and the fake docker constant. `comparable.snapshots` leaves the cell digest. A pre-run digest cannot hold a post-run census, so the digest compares the **resolved** `mcp.servers`, defaults filled.

**Both drivers render from types.** Each driver builds its own config shape from the typed `McpServer`. Nothing is passed through serde. Claude Code writes `stdio|http|sse`: `local` maps to `stdio`, and `remote` maps to `http` or is refused, which P9 decides. `codemode = true` is refused. `name` becomes the key. A conformance check holds both rendered files to the keys each harness documents.

## Leaves (new subtree `dogfood/parity/`; depth gives the order)

| Leaf | Level | Goal (one line) | Gate seen failing on the base | Model | Est / cap |
|---|---|---|---|---|---|
| **P8 `declaration`** | 3.1 | `AgentSetting`, required `answer`, Claude Code's table, OpenCode's table moved onto the enum, `PathRow.agent`, conformance completeness | A cell setting `budget.per_run.max_cost_usd` on OpenCode is refused by the table row, not by ad hoc code. A new variant fails compile in both drivers (shown on a scratch branch). | Claude (Sonnet implementer, lead review) | $10 / $20 |
| **P9 `render`** | 3.2 | Both drivers render MCP from types. `codemode` defaults to false, filled at resolve. Claude Code refuses `codemode = true`. The instruction-file answer comes from a probe. | On the base, the `.mcp.json` of a cell with `type = "local"` and `codemode` carries both keys. On the tip it carries `stdio` and no `codemode`. | Claude (Sonnet) | $6 / $12 |
| **P10 `census`** | 3.2 | Census `mcp_servers {name, status, tools}` on both harnesses. OpenCode collects through `mcp.list` plus gateway names. Step 1 probes code mode: a session tool API, then the `execute` description, then a `tools/list` that Candler runs in the isolation. The first that works wins, and the rest are recorded. | On the base, a gateway census of a cell with one server has no servers. | Claude (Sonnet, lead on the probe fork) | $12 / $24 |
| **P11 `pages`** | 3.2 | Capability and configuration contract pages generated from `AgentSetting` × drivers. This replaces the 8 hard-coded rows. Non-setting capabilities (gateway, stop, trace) keep their typed source. The census record is documented (S6). | The page test fails when a driver's answer changes and the page is not regenerated. | **RiVault**: Reasoning-Medium, then Reasoning-Large, then Sonnet, with a mandatory Sonnet review and a check that thinking ran | tokens + $2 review / $4 |
| **P12 `snapshot_out`** | 3.3 | Remove the constant producer and the fake docker constant. `comparable` drops snapshots, with a digest version bump. `inspect` shows the census, and old runs still read. README wording becomes "per-run tool census". | `guard_catalog_snapshot` is replaced by a census guard that fails on the base, where the record of a declared server is empty. | Claude (Sonnet) | $8 / $16 |
| **P13 `parity_run`** | 3.3 | The parity procedure: one cell, both harnesses, a real stdio MCP server (shell fixture; the OpenCode image has no python3), OpenCode with `codemode` false and true. It fails unless each record names the server and its tools. It runs in container-e2e with the gateway's scripted model, plus one live RiVault run per harness as the proof. | The same test on the base fails on OpenCode (no servers). | Claude (Sonnet); live runs on RiVault | $6 / $12 |
| **Seam review 3** | 3.4 | One review across P8–P13, Sonnet with three questions first, then the level is closed in the knowledge-transfer report | | Sonnet | $3 / $6 |

Writers never share files within a level. P9 owns `prepare.rs` in both drivers and `servers.rs`. P10 owns `gateway.rs`, `census.rs`, `parse.rs` and the census path of the OpenCode driver. P11 owns `tests/it/support/contract_pages/`. If P9 and P10 both need `opencode/driver.rs`, P10 rebases onto P9.

**Spend.** The leaves are estimated at $47 with a $96 cap. Lead orchestration is estimated at $20. The work before level 3 already cost about $127, against a $100 gate that Eliott raised to $127. The amendment rewords the spend gate: **level 3 stays within $70 measured from transcripts, and no row passes its cap without stopping**. RiVault is used only for P11, which is not critical: it generates pages from a typed source, and the page test checks them. P8, P9, P10, P12 and P13 decide what the record says or what the agent sees, so they stay on Claude.

**Compatibility.** Changing the comparable set and the digest version changes the cell-digest vectors (`cell_digest.toml`, `catalog_content_digest.toml`). This is a deliberate, versioned change: old manifests stay readable, history is not rewritten, and new vectors are added beside the old ones. Per standing rule it goes to the coordinator, as a pre-condition of P12 and not as a question for Eliott.

## The principle, carried into every agent I supervise

Level 3 is one pass of the alignment work. If an implementer makes its gate pass by leaving excess behind, the level fails its own purpose, even when every test is green. So the principle is how each leaf is briefed, supervised and accepted. It is not a checklist at the end (D4).

- **Brief.** Each implementer reads, on its own branch, the paragraph of `AGENTS.md` called "What sane work means in Candler". Its brief then adds a prose paragraph written for its leaf, saying what sane work means there at each of the three levels:
  - how it is developed: probe before building, size the work to what is free, and run each RiVault run only when the work needs it;
  - what it leaves in the code: one producer, one reader, nothing passed through, and nothing kept "for compatibility" without a reader;
  - how it is consumed: what the experimenter reads and what the run record says.

  A new notes file, `integrity-level3-implementer-brief.md`, adds the level's rules to the existing `integrity-implementer-brief.md`. The machine rules are written into it: one target dir per leaf, removed at merge; at most 3 builds; Docker containers named, removed by name, never pruned; scratch deleted at each step.
- **Joints.** I talk with each implementer when its guard first fails on the base, and before it opens its PR. Each time I ask what the change leaves behind at each level, and I read the diff for a second producer, a second reader, a wrapper or a passthrough. Examples: the census added while `CATALOG_PROGRAM` stays; the table added while the hand-written refusals stay; a shim for old runs that has no reader.
- **Acceptance.** A leaf that works by adding where it should replace is stopped and re-briefed, not merged. Each leaf report has a short prose section, "What it leaves", covering the three levels, and seam review 3 asks about it first. The mandatory Sonnet review of P11's RiVault output checks the principle as well as correctness.
- **Sweep.** The "What sane work" paragraph of `AGENTS.md` names "per-run tool-catalog snapshots" and "a catalog snapshot" as stored state. P12 rewords it in the same change as the README, as Eliott's README ruling requires, so the rule and the yardstick do not drift apart.
- **My own work.** I lead the level with the same rules: subagents only where a brief can be written, no polling, and spend measured from transcripts at each joint.

## Judged on three levels

- **Development.** A new setting is a compile error in every driver until each answers it, so silent acceptance cannot come back. Leaves are cut by concept: declaration, render, record, parity. Each gate is an observation seen failing on the base.
- **What it leaves in the code.**
  - Removed: the constant producer and its matching fake, the serde passthrough in Claude Code's renderer, OpenCode's hand-written refusals, and the 8 hand-written page rows.
  - Added: one enum, one answer per driver per entry, and one census shape.
  - Kept: the `tool-catalog-snapshot/1` schema (plug-in format) and `CONFIGURATION_PATHS` (extended).
- **How it is consumed.** An experimenter reads one generated page that says, per harness, whether each setting is mapped, provided by Candler, or refused and why. A run record names the servers and tools the agent had, on either harness. The same cell means the same agent view by default, now that the `codemode` default matches the reference.

## After approval (in this session)

1. Load `managing-roadmaps`. With dirtree-rdm, add `causes/checkpoint/snapshot/review/dogfood/parity/` with the leaves above, written in the template `catalog_snapshot.md` uses (Goal, Estimate, Pre-conditions, Success Gates, References, Steps).
2. Amendment A3 in `__roadmap__/integrity/README.md`, sweeping the gates as it goes:
   - the goal gains harness parity;
   - the "done" gate covers `parity/`;
   - the spend gate is reworded as above;
   - the Status graph and Nodes table come from dirtree-rdm.
3. Write a briefing in the milestone format (verdict, done table with evidence, amendments, next table, goal check) to `__reports__/diet/07-briefing_integrity_level3_v0.md`.
4. Commit on `plan/integrity-level3` (cut from `origin/feat/integrity` 27d1d1dd), push, and open a PR into `feat/integrity` only, titled `merge(api): plan M6c level 3, harness parity`, with a body that speaks from outside. Bind it with the ccd_pr tools and send the coordinator a note.

## Verification

- `dirtree-rdm` validates the tree, and the Status graph matches the Nodes tables.
- Every leaf has a gate seen failing on the base, a model, an estimate and a cap.
- The briefing's goal check quotes the README purpose paragraph verbatim.
- `git log origin/feat/integrity..HEAD` shows planning commits only, and `main` is untouched.
