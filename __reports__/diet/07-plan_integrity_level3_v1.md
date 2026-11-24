# Plan: M6c Integrity, level 3, harness parity (v1, revision 2)

Supersedes `07-plan_integrity_level3_v0.md`, after the maintainer's rulings on it. Approved.


## Context

The P7 dogfooding pass ran an MCP experiment on OpenCode. The agent called `converter.convert` in every run, yet nothing Candler stored says so in a form that compares with Claude Code:
- The stored "catalog snapshot" comes from a constant program that always prints `servers: []` (`CATALOG_PROGRAM`, `commands/preflight.rs:40-42`). The CLI tests' fake docker prints the same constant, so the level-2 guard agreed with the code on the wrong value.
- OpenCode's trace reader marks every MCP call `origin: other` (`candler-harness-opencode/src/export.rs:160-203`). Claude Code's reader marks the same call `ToolOrigin::Mcp{server}` (`candler-harness-claude-code/src/parse.rs:386-398`). So traces of one cell do not compare across harnesses.
- Claude Code receives OpenCode's MCP keys verbatim (`prepare.rs:99-115`, a serde passthrough).
- What happens to each setting on each harness is stated in five different shapes, and two of them let a driver say nothing.

The PR #70 plan is revised under Eliott's rulings:
1. **Harmonise the declaration lists.** Fold them into the one idiom this codebase does best ("harmonizing such lists is scalability"). The per-driver answer to every configuration path is part of that shape. A missing answer fails to compile, or fails one test that walks every path for every registered driver. A default empty answer does not count.
2. **Fix the Claude Code MCP passthrough, on both sides.** Map `local`/`remote` to Claude Code's own types. Claude Code refuses code mode, with a reason.
3. **Non-goals:** hooks an experiment can set, and CLAUDE.md/AGENTS.md. Both belong to a later feature round. Level 3 is integrity and alignment.
4. **No `opencode debug config` as a source.** The configured side is already single-sourced, by level 1's `one_reader` and `mcp_schema`. The level-2 snapshot surface goes along with its producer.
5. **Reframe.** Which servers the agent called is for the consumer to parse from the trace. Candler owes two things: a truthful record of what it installed (the resolved `mcp.*` values, which the cell digest already hashes) and a faithful trace that compares across harnesses.
6. **Asked in this session:** the census keeps **`census.tools` only**, as the comparability verdict's input, and never names servers. OpenCode code mode is **declared, not parsed**.
7. **Kept from revision 1:** Claude Code is the reference harness, and a defect found on its side is fixed in both drivers. The README says "per-run tool census". `codemode` defaults to `false`.

The yardstick, from the README purpose paragraph:

> - **Execution:** run agents under controlled conditions (experiment matrix, runners, isolation, per-run tool-catalog snapshots, provenance).

## What the study settled

| Question | Answer | Evidence |
|---|---|---|
| Which list idiom is best here? | A `const` array of `Copy` rows holding `&'static str` and enum fields, read by `const fn`s and by the page generators (`CONFIGURATION_PATHS`). Completeness is enforced by an exhaustive `match` with no wildcard, as `refusal_table!` does for errors. | `resolve/paths.rs:85-120`, `refusal.rs:301-416` |
| Which shapes are outliers? | `SettingRow` (a `Vec` of `String`s, optional, filled only by OpenCode). `Capabilities` (bools behind a builder whose unset fields default to `false`). The capability page's `ROWS` (fn pointers that string-match `"budget.per_run.turns"`). | `driver.rs:106-125`, `spec.rs:518-593`, `contract_pages/capabilities.rs` |
| Where can a per-driver answer live? | In each driver crate. Drivers depend on `candler-harness` and `candler-schemas`, not on `candler-core`, so the key enum lives in `candler-harness`. Only `crates/candler` sees every driver, so it hosts the walk over all drivers. | `Cargo.toml` graph |
| Is the configured MCP side single-sourced? | Yes. The cell digest's `comparable.values` holds every resolved value, `mcp.*` included. Dropping `comparable.snapshots` is enough, and nothing new is needed. | `execution.rs:1970` |
| What uses the census? | `manifest/compare.rs` `class_verdict`. When `census.init_tools` differ between arms and no declared `tools.*`/`mcp.*` factor differs, that is a comparability deviation. The trace cannot rebuild this list, because it records only calls (`EventKind` has no init event). | `compare.rs`, `trace.rs:741` |
| Who produces `tool-catalog-snapshot/1` in real use? | **The HTN consumer.** Its `scripts/capture_catalog_snapshot.py` writes `/1`, and its Tier D (`_catalog_snapshot.py`) reads `/1` and refuses anything else. Both were released in htn-creator-eval@0.5.3 (commit 11212ea1). They name Candler's `catalog-snapshot.schema.json` as the definition. Inside Candler only the constant and the tests produce it. | consumer repo |
| What did level 2 add for the consumer? | `InspectRun.catalog`, filled by `inspect.rs` `catalog_of`. It reads the blob behind `comparable.snapshots`, and it is in `inspect-run.schema.json`. | `inspect.rs:215-292` |

So `/1` leaves Candler with its own producer, its resolve step and its level-2 consumer surface: a schema Candler neither writes nor reads is not Candler's. The HTN consumer keeps grading, since Tier D checks only the schema marker of its own committed file, and an HTN-side follow-up takes the schema file into that repository.

## Design

### One shape for every per-harness list

The rows are `Copy` and hold `&'static str`. Every key is a fieldless enum with a `const ALL`. Each driver states each fact once, in an exhaustive `match` with no wildcard, and the drivers deny `clippy::wildcard_enum_match_arm`. Tables and pages are walks over `ALL`. Nothing is built as a `String` at run time.

- **`Answer`** in `candler-harness`: `Mapped { construct: &'static str }`, `Candler { how: &'static str }`, `Refused { reason: &'static str }`. It is the one answer type for every key.
- **`AgentSetting`**: what an experiment sets that reaches the agent. That is model id, endpoint and effort; permission mode; allowed and disallowed tools; `env`; `inputs`; skills; MCP servers, with transport, `codemode` and `name`; the wall, turn and cost budgets; and session mode. A setting with a closed set of values carries the value, as in `Effort(EffortLevel)`, `Transport(Transport)` and `Codemode(bool)`. A new value is then also a compile error.
- **`DriverFact`**: what a driver does on its own. That is graceful stop, live events, the in-harness guard, execution outside the child, and the census. It replaces `Capabilities`'s bools and its builder. `budgets` and `session` are dropped, because `AgentSetting` already answers them and they would be a second statement.
- Both enums get a required trait method with no default: `answer(&self, AgentSetting) -> Answer` and `fact(&self, DriverFact) -> Answer`. Removed: `settings_table()`, `SettingRow`, `Capabilities` and `CapabilitiesBuilder`. Each `check_settings` refuses a set value whose answer is `Refused`, through the existing `experiment.setting_unsupported` code. OpenCode's hand-written refusals (`MODE_REFUSALS` and the others in `prepare.rs:131-195`) become match arms.
- **Configuration paths:** `PathRow` gains a required column `reach: Reach`, either `Agent(AgentSetting)` or `CandlerOnly`. `row()` takes it as an argument, so no row has a default. One test in `crates/candler` walks `CONFIGURATION_PATHS × registered drivers` and checks two things. A path that reaches the agent has its driver's answer on the configuration page. Every `AgentSetting` is reached by a path or is listed in an explicit `NO_PATH` const.
- **`refusal_table!` is unchanged.** It is already this idiom for errors: static rows, a match per variant, docs generated from the rows, and a cross-check with `codes.toml`.

### MCP rendered from types on both drivers

- Each driver builds its own file from the typed `McpServer`. Nothing goes through serde.
- **Claude Code:**
  - `local` becomes `stdio`, with `command`/`args`/`env`.
  - `remote` becomes `http`, with `url`/`headers`. P9 checks this against the Claude Code MCP doc and cites it, and refuses a field Claude Code has no place for.
  - `name` becomes the key.
  - `Codemode(true)` is `Refused`: "Claude Code offers MCP tools directly; it has no code mode". `Codemode(false)` is `Mapped`.
- **OpenCode** keeps its own shape, now built from types as well.
- Resolve writes the `codemode = false` default into the cell, which changes the digest (see Compatibility). A conformance check holds each rendered file to the keys its harness documents.

### One trace form for an MCP call

- The OpenCode reader marks `<server>_<tool>` as `ToolOrigin::Mcp { server }`. It matches against the cell's resolved server names and does not guess from the spelling.
- Resolve refuses a set of servers whose OpenCode prefixes are ambiguous, for example `a` and `a_b`.
- The tool's own name is also spelled differently: `mcp__s__t` on Claude Code, `s_t` on OpenCode. So `ToolOrigin::Mcp` gets an optional `tool` field holding the bare tool name, and both traces carry one spelling. It is optional so that traces stored before the change still read, and one test parses an old trace without it. The coordinator accepted this wire change (`ToolOrigin` has `deny_unknown_fields`).
- The stored Claude Code trace keeps the raw `system/init` line, so a consumer can recover server status by post-processing, as for calls. This was the coordinator's condition for dropping `census.mcp_servers`, and P10 holds it as a gate.
- **Code mode, declared:** `Codemode(true)` on OpenCode is `Mapped`, and its construct says the trace records `execute` calls with empty `wrapped_calls`, so MCP calls inside them are not attributed. Nothing parses `execute`. The parity procedure runs with code mode off, the default. This is why the default is off: with code mode on, the trace shows only `execute`, and the two harnesses no longer compare.

### The census, reduced

- The census keeps `census.tools`, `census.init_tools` and `census.source`, with one producer per harness: Claude Code's `system/init`, and OpenCode's gateway.
- `census.mcp_servers` is removed, and with it Claude Code's status reading.
- `SERVER_START_WAIT`'s doc (`jobs/orchestrate/servers.rs:22`) stops claiming the census records servers.
- No `mcp.list` or `debug config` collection is added.

### The snapshot surface out

- **Removed:**
  - `CATALOG_PROGRAM`, `CommandCatalogProvider` and the provider seam behind it, if colgrep finds no other implementor;
  - `CatalogStep` and the fake docker's catalog constant;
  - `comparable.snapshots`, under a new digest version;
  - `InspectRun.catalog` and `catalog_of`;
  - the refusal codes only these emitted.
- `inspect` and the op API show the run's resolved `mcp.*` values from `comparable`. An old sealed run still reads. Its stored blob is left in the store and not shown, with no shim.
- **Also removed:** `tool-catalog-snapshot/1`, with its types, generated schema and fixtures. Its only producer and reader are the HTN consumer's, which gets a follow-up to hold the schema file itself.
- The old oracle vectors stay beside the new ones, because the maintainer's own stores (golden and HTN runs) hold old manifests, and their files say so.
- Before `InspectRun.catalog` is dropped, P12 confirms that nothing in htn-creator-eval@0.5.3 reads `inspect`'s `catalog` field. If something does, the drop waits for the coordinator.

## Non-goals

- Hooks an experiment can set, and CLAUDE.md/AGENTS.md or other instruction files. Both are for a later feature round. Neither is an `AgentSetting`, and no probe is run for them.
- `opencode debug config` as a source of anything.
- Reporting which servers the agent called, or their status. That is post-processing of the trace, done by the consumer.
- Parsing OpenCode `execute`.
- The HTN-side change that takes the `/1` schema file into that repository. It is a follow-up there.

## Leaves (subtree `dogfood/parity/`; depth gives order)

| Leaf | Level | Goal | Gate seen failing on the base | Model | Est / cap |
|---|---|---|---|---|---|
| **P8 `declaration`** | 3.1 | `Answer`, `AgentSetting` and `DriverFact`, with required `answer`/`fact` in both drivers. `SettingRow`, `settings_table`, `Capabilities` and its builder are removed. `PathRow.reach` and the walk over all drivers. | A driver without a session answer compiles and passes on the base, through the builder's default `false`. On the tip, a new variant fails to compile in both drivers (shown on a scratch branch). A cost cap on OpenCode is refused by its match arm. | Sonnet, lead review | $12 / $24 |
| **P9 `mcp_render`** | 3.2 | Both drivers render MCP from types. `local`→`stdio`, `remote`→`http`. Claude Code refuses `codemode = true`. The default is filled at resolve. | On the base, the `.mcp.json` of a cell with `type = "local"` and `codemode` carries both keys. On the tip it carries `stdio` and no `codemode`. | Sonnet | $6 / $12 |
| **P10 `trace_parity`** | 3.2 | OpenCode's reader attributes MCP calls as `ToolOrigin::Mcp`, with the optional bare `tool` name. Resolve refuses ambiguous prefixes. The census is reduced to tools, and the stored Claude Code trace keeps its raw `system/init` line. | On the base, the OpenCode export of a call to `s_t` with server `s` reads `origin: other`. | Sonnet | $8 / $16 |
| **P11 `pages`** | 3.2 | The capability and configuration pages are walks over `AgentSetting::ALL`, `DriverFact::ALL`, `CONFIGURATION_PATHS` and the registered drivers. `ROWS` and `cap()` go. | On the base, changing OpenCode's cost-cap answer leaves the page test passing. | **RiVault** Reasoning-Medium → Reasoning-Large → Sonnet. The Sonnet review is mandatory, and the lead checks that thinking ran. | tokens + $2 / $4 |
| **P12 `snapshot_out`** | 3.3 | Remove the constant producer, the resolve step, `comparable.snapshots` (new digest version) and `InspectRun.catalog`. `inspect` shows the resolved `mcp.*`. README and AGENTS.md swept. | A guard on `candler run` → `inspect`, with one declared server, finds the server in the shown `mcp.*` values. On the base `inspect` shows only a catalog that says `servers: []`. | Sonnet | $8 / $16 |
| **P13 `parity_run`** | 3.3 | One cell on both harnesses with a real stdio fixture server, code mode off. It asserts (a) the server is installed: in the resolved `mcp.*` and in each driver's rendered file; and (b) the stored trace shows the scripted call as the same `ToolOrigin::Mcp` form on both. It runs in container-e2e, and one live RiVault run per harness is the proof. | The same test on the base fails on OpenCode (`origin: other`). | Sonnet; the live runs on RiVault | $6 / $12 |
| **Seam review 3** | 3.4 | One review across P8–P13 with three questions first: added where it should replace? one answer per driver per key, one statement on the pages? one trace form across harnesses? Then the knowledge-transfer report. | | Sonnet | $3 / $6 |

Writers never share files within a level:
- P9 owns both drivers' `prepare.rs` MCP renderers and `servers.rs`.
- P10 owns `export.rs`, `parse.rs`, `census.rs`, `gateway.rs`'s census, and the resolve name check.
- P11 owns `contract_pages/`.

P8 touches both drivers' `driver.rs` and the `prepare.rs` refusals before level 3.2 starts. P9 and P10 then rebase onto it.

**Spend.** The leaves are estimated at $45 with a $90 cap, and lead orchestration at about $20. The gate is unchanged: **level 3 stays within $70, measured from transcripts, and no row passes its cap without stopping.** RiVault is used only for P11: its pages are generated and the page test holds them. P8–P10, P12 and P13 decide what the record and the trace say, so they stay on Claude.

**Compatibility, ruled by the coordinator:**
- **P9, accepted:** the digest changes when the `codemode` default is filled, so the resolved cell records what Candler installs.
- **P10, accepted:** the optional `tool` field on `ToolOrigin::Mcp`, holding the bare tool name. It is a wire change, because of `deny_unknown_fields`, and it is optional so old traces still read.
- **P12, accepted:** the new digest version without `snapshots`. Its vectors are added beside the old ones (`cell_digest.toml`, `catalog_content_digest.toml`), which stay for the maintainer's own stores and say so. `InspectRun.catalog` leaves `inspect-run.schema.json` once P12 has confirmed that htn-creator-eval@0.5.3 has no `inspect` caller reading it.
- **`/1`:** it leaves Candler in P12 (revision 3). htn-creator-eval@0.5.3 keeps grading; an HTN-side follow-up takes the schema file.

## The principle, carried into every agent I supervise

Level 3 is one pass of the alignment work. If an implementer makes its gate pass by leaving excess behind, the level fails its own purpose, even when every test is green.

- **Brief.**
  - Each implementer reads `AGENTS.md` "What sane work means in Candler", then a paragraph written for its leaf on the three levels:
    - how it is developed: probe before building, size to what is free, a RiVault run only when the work needs it;
    - what it leaves: one producer, one reader, one shape, nothing passed through, nothing kept for compatibility without a reader;
    - how it is consumed: what the experimenter reads, what the record and the trace say.
  - `integrity-level3-implementer-brief.md` adds the machine rules: one target dir per leaf, removed at merge; at most 3 builds; containers named and removed by name, never pruned; scratch deleted at each step.
- **Joints.** I talk with each implementer when its guard first fails on the base, and before its PR. I ask what the change leaves at each level, and I read the diff for a second producer, a second reader, a passthrough or a shim. Examples: `Capabilities` kept beside `DriverFact`; `settings_table` kept "for the pages"; a census field kept with no reader.
- **Acceptance.** A leaf that adds where it should replace is re-briefed, not merged. Each leaf report has a "What it leaves" section, which seam review 3 reads first.
- **Sweep.** P12 rewords the README phrase, `AGENTS.md`'s stored-state list and the live candler-v1 M6c Goal that quotes the phrase, in one change. Done leaves keep their words as history.
- **My own work.** The same rules apply to how I lead: subagents only where a brief can be written, no polling, spend measured from transcripts at each joint.

## Judged on three levels

- **Development.** A new setting, value or driver fact is a compile error in every driver until each answers it. A new path cannot be written without saying whether it reaches the agent. Leaves are cut by concept: one shape, render, trace form, removal, parity.
- **What it leaves.**
  - Removed: `SettingRow`, `settings_table`, `Capabilities` and its builder, `ROWS` and `cap()`, the serde passthrough, OpenCode's hand-written refusal lists, the constant producer and its resolve step, `comparable.snapshots`, `InspectRun.catalog`, and `census.mcp_servers`.
  - Added: `Answer`, two key enums of one shape, a `reach` column, and one name check at resolve.
  - Kept: `CONFIGURATION_PATHS` (extended), `refusal_table!` and `census.tools` (one reader). `/1` is removed with the producer.
- **How it is consumed.** One generated page says, per harness, what happens to each setting and fact. `inspect` shows the MCP config Candler installed. The trace of one cell attributes the same MCP call the same way on both harnesses, which is what a consumer parses. Code mode's limit is stated where it is chosen.
- **Trades, accepted by the coordinator:**
  - P8 grows, which costs more to develop and buys one shape to leave.
  - Claude Code's record loses its server status, which buys one fewer producer. The trace keeps the calls and the raw `system/init` line, so the consumer can still recover the status (a P10 gate).
  - `/1` leaves Candler, which costs the HTN consumer one follow-up and leaves Candler no schema it does not use.

## After approval (in this session)

1. With dirtree-rdm, rework `parity/`:
   - rewrite `declaration.md`, `mcp_render.md`, `pages.md`, `snapshot_out.md` and `parity_run.md`;
   - replace `census.md` with `trace_parity.md`;
   - update `seam_review_three.md`'s third question and its report list.

   Then update the A3 row of the Amendment Log with revision 2, and validate.
2. Update `__reports__/diet/07-plan_integrity_level3_v0.md` to this plan and `08-briefing_integrity_level3_v0.md`: the Why section with rulings 1–6, the Next table and the Goal check.
3. Commit `rdm(api):` and `reports(api):` on `plan/integrity-level3`, push, update the PR #70 body (it speaks from outside), and send the coordinator a note.

## Verification

- `dirtree-rdm validate` passes, the Status graph matches the Nodes tables, and every link resolves.
- Every leaf has a gate seen failing on the base, a model, an estimate and a cap.
- The briefing quotes the README purpose paragraph verbatim.
- No node mentions `debug config`, `mcp.list`, `census.mcp_servers`, hooks or instruction files, except as non-goals.
- `git log origin/feat/integrity..HEAD` shows planning commits only, and `main` is untouched.
