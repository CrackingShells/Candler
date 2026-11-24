# Seam review 3: findings and what was decided

A fresh Sonnet reviewer wrote none of this code. It read the level-3 plan (v1), the leaf file, the inputs note carried from
the leaves, and the level-3 range `bf1f03ea..92c0f3a3` (#71, #72, #74, #75, #77, #78, #79), at `659258c2`. It read code
through colgrep only, and it ran no build.

**Disclosure.** Before the coordinator's instruction not to, the reviewer had read the six leaf reports in full, including
their verdict, review and "What it leaves" sections. Every finding below rests on code it expanded itself (cited by
`file:line`); the leaf reports were used only to locate code. Where a point rests on a leaf report and not on code, it says so.

## The four questions

**1. Did any leaf add where it should have replaced?** No second producer of a catalog, MCP file or origin remains. What is
left is small:

- Two one-line passthroughs: `candler-harness-claude-code/src/prepare.rs:95` and `candler-harness-opencode/src/prepare.rs:489`
  each wrap `candler_harness::mcp::servers`, and `runner/exec/mod.rs:677` calls the library directly (F3).
- Effort and mode spelling has two readers: `EffortLevel::from_text` (`candler-harness/src/spec.rs:568`) and
  `PermissionMode::from_text` (`spec.rs:620`) are called by the refusing reader at resolve and again by `Cell::effort` and
  `Cell::permission_mode` (`spec.rs:1499`, `:1512`), which fall back silently (input 1 below).
- The census has two producers by design: the harness's `system/init` and the gateway (`isolation/gateway.rs:1268`,
  `runner/exec/mod.rs`, `census(observed.init.or(gateway_init))`). The plan's revision 3 meant to drop the gateway census;
  amendment A5 kept it. It is stated on the capability page (`docs/contract/harness-capabilities.md:57`). Not a defect.
- `comparable.snapshots` keeps one live writer, the importer, and one reader, `import/grading.rs:421-428`. Both have a reason.

**2. One answer per driver per setting, one statement on the pages?** Yes. `ClaudeCodeDriver::answer`
(`claude-code/src/driver.rs:202`) and `opencode::prepare::answer` (`opencode/src/prepare.rs:113`) are exhaustive matches with no
wildcard over `AgentSetting`, and the MCP rows, `Codemode(true)` and the SSE refusal reach `harness-capabilities.md:42-45`.
`DriverFact::UnsetPermissionMode` is answered by both drivers (`claude-code/src/driver.rs:301`, `opencode/src/driver.rs:179`).
One gap: four answers are the same sentence written in both drivers (F5).

**3. Same servers, same trace form on one cell?** Yes, with two caveats. Both drivers read servers through
`mcp::servers` and render from the typed server: Claude Code writes `.mcp.json` and passes `--mcp-config .mcp.json
--strict-mcp-config` (`claude-code/src/driver.rs`, `argv`), OpenCode writes the `mcp` entries in `render_config`
(`opencode/src/prepare.rs:495`). Both origins come from `ToolOrigin::mcp_tool(server, tool)`
(`claude-code/src/parse.rs:384-396`, `opencode/src/export.rs:160-178`). The caveats are F1 (an unset permission mode) and F2.

**4. The two silent `other` readings.**

- *Ingest without a cell.* `Registry::offline` (`crates/candler/src/drivers.rs:67`) hands the reader no server names, so every
  OpenCode MCP call reads `Other`. Neither the reader nor any contract page says so: `ToolOrigin::Other` is documented only as
  "Any other source" (`candler-schemas/src/trace.rs:640-662`), and no page under `docs/contract/` mentions an origin. A reader of
  an ingested trace cannot tell "not an MCP call" from "an MCP call whose server was never declared". Not blocking: the run path
  passes the cell's servers (`runner/exec/mod.rs:677-685`), and ingest has no cell to take them from. F2.
- *Undeclared server.* The difference is stated, once, in the doc comment of `ToolOrigin::Mcp` (`trace.rs`, "Claude Code's reader
  takes any `mcp__<server>__<tool>` ... OpenCode's takes only a declared server"), which becomes the description in
  `normalised-trace.schema.json`. The trace carries no marker, and no contract page repeats it. A third divergence is
  unstated: an unknown non-MCP tool is `Builtin` on Claude Code (`parse.rs:395`) and `Other` on OpenCode (`export.rs:171-175`).
  The reader of one trace is told only if they open the schema. F2.

## Judging the inputs

| Input | Belongs to level 3? | Decision |
|---|---|---|
| P8: `Cell` holds JSON; resume falls back silently | Yes, the cause is level 3's; the reach is narrow | Follow-up, non-blocking. `runner/plan.rs:851-884` (`answers`) lets a stored value the experiment does not declare replay without being checked, so a stored mode or effort that P8 does not know reads as `Default` or unset (`spec.rs:1499-1520`). Reaching it needs a run stored before P8 with text outside the enums. Fix: make `Cell::permission_mode` and `Cell::effort` fallible and let the replay path refuse. Typed fields in `Cell` are not needed. I did not run a reproduction |
| P8: `setting_unsupported` names the harness slug, not the version | Level 3, cosmetic | Follow-up, non-blocking |
| P8: `MissingValue` has no field path | Pre-existing | Follow-up, non-blocking |
| P9: a server key that is no field-path segment gets no `codemode` | Stale: it is refused | `fill_codemode` (`runner/plan.rs:582-611`) returns `PlanError::Driver(InvalidValue)` for such a key. It is refused at plan, not at resolve, but before any job. Closed |
| P10: two silent `other` readings | Yes | Question 4; F2, non-blocking |
| P10: `offline_reader`'s `servers` is unused by Claude Code | No action | The parameter exists for harnesses that spell `<server>_<tool>` (`claude-code/src/driver.rs:347`, `opencode/src/driver.rs:260`); one producer, no second reader |
| P10: server keys held to the name pattern by the reader only | Follow-up | Non-blocking; carry the const into `propertyNames` if the schema ever types the map |
| P12: `needs.snapshots` / `context.snapshots` removal | Follow-up, already decided | The input's premise is partly out of date: `python/candler-evaluator/.../plugin.py:119` now reads `context.get("snapshots", [])`. The protocol wire still requires the key (`docs/contract/evaluator-protocol.md:40,72,160,306`). The deferral is stated in `__design__/architecture.md:1044` and in the `evaluation.rs:1616` doc comment. It is not on the contract page, where an evaluator author meets the field with description "-". Non-blocking (F6) |
| P12: "always empty" wording and the import exception | Yes, wording | `import/grading.rs:421-428` fills `context.snapshots` from an imported run. Line 1044 says "for runs Candler makes", which is true, and `architecture.md:623` and `:1146` say imported only; no one place states both. Fold into F6 |

## Findings

| # | Finding | Cause | Leaves | Why no verifier saw it | Blocking | Fix |
|---|---|---|---|---|---|---|
| F1 | A cell with an MCP server and no `permission_mode` installs the same servers on both harnesses but does not call them alike: Claude Code runs `--permission-mode default` (`claude-code/src/driver.rs`, `argv`, `cell.permission_mode()` reads absent as `Default`); OpenCode runs `--auto` over an allow-all baseline (`opencode/src/driver.rs:179`). I did not check what Claude Code does with an MCP call under `default` in headless mode | `UnsetPermissionMode` facts at `claude-code/src/driver.rs:301`, `opencode/src/driver.rs:179` | P8, P13 | P8 gated that the fact is answered and shown on the page. P13's guard sets `bypassPermissions` so the harnesses compare, and so never exercises the unset case | No: the page states both answers. The user's case is an experiment that sets no mode and sees MCP calls only on OpenCode | Decide the default per harness, or let resolve refuse an MCP cell with no mode. The parity run's own "not covered" line already names it |
| F2 | The trace does not tell a reader why an MCP call reads `other` or `builtin` (ingest, undeclared server, unknown non-MCP tool); the reason lives in a type doc comment only | `trace.rs:640-662`; `drivers.rs:67`; `parse.rs:395` vs `export.rs:171-175` | P10, P11 | P10's gates named the two readings as "not covered" and deferred them to this review. P11 generated pages from the declaration, and the origin is no declaration | No | One row on `harness-capabilities.md` for how each harness reads an MCP call, with the ingest case; or a `Fact` for it |
| F3 | The `mcp_servers` wrappers of both drivers are pass-throughs to `mcp::servers`, and the core calls the library directly | `claude-code/src/prepare.rs:95`, `opencode/src/prepare.rs:489`, `runner/exec/mod.rs:677` | P9, P10 | Each leaf kept its own call site; no gate asks for a call count | No | Delete the two wrappers; call `mcp::servers` |
| F4 | A stored cell replays text P8 does not know as `Default` or unset | `runner/plan.rs:851-884`, `spec.rs:1499-1520` | P8 | The guards go through resolve, which refuses the text. Replay does not go through it | No (needs a pre-P8 run) | See the P8 input row |
| F5 | `ModelEndpoint`, `Env`, `Inputs` and `WallBudget` carry the same `Candler { how }` text in both drivers, and nothing ties the text to what the core does | `claude-code/src/driver.rs:202-282`, `opencode/src/prepare.rs:113-196` | P8, P11 | The walk checks that each driver answers; it does not check that a core fact is stated once | No | Put the four answers in one `const` in `candler-harness`, which each driver returns |
| F6 | The evaluator `snapshots` deferral is not where an evaluator author reads the field | `docs/contract/evaluator-protocol.md:40,72,160,306` | P12 | The leaf's gate was "the deferral is stated", and it is, in the design document and in a doc comment | No | Carry the doc comment into the page row, with the import exception |
| F7 | OpenCode's bare `tool` is the name after `<server>_`; I did not check whether OpenCode rewrites characters of a tool's own name when it builds the id, in which case `tool` would not be the server's name for it | `export.rs:160-166` | P10, P13 | The parity fixture's tool is `stamp` | No (unverified) | Run one tool whose name has a `.` or `-` on OpenCode |

## Not reviewed

- Each PR's oracle vectors, generated schemas and generated pages (by instruction).
- `tools.allowed` and `tools.disallowed`: whether an MCP tool name is spelled in each harness's own form in those lists.
- The stored Claude Code trace keeps the raw `system/init` line (the P10 condition): not checked in code.
- Container-e2e and the live runs, which no review of code can show.

## Consistency

Read-only. No code changed, no build run, nothing committed. Nothing found blocks level 3.

## What the lead decided

The lead read F1, F3 and F4's code at the cited lines, and they hold. F1's `UnsetPermissionMode` answer for Claude Code
says in so many words that a headless run under `default` denies every tool call that needs approval. So the gap is
stated on the page, not silent, and choosing a default for MCP cells is a design call for the next milestone.

Nothing blocks level 3. Two findings were against the level's own gate ("no second producer, second reader or passthrough
left by the level"), and both are fixed in #80:
- F3: the two `mcp_servers` passthroughs are deleted.
- F4: `Cell::effort_at` and `Cell::mode_at` in `candler-harness` are the one reader of effort and mode text. Resolve and
  both accessors call them, so a replayed cell with unknown text is refused with `InvalidValue` at its field instead of
  running as `Default`. The coordinator ruled F4 into the level: a silent default becoming a refusal is what Integrity is
  for, and Candler has no outside users to protect from it.

F1, F2 and F5–F7 are recorded, in order, under "Next" in the knowledge-transfer report. The review cost $1.22 against a
$3 estimate and a $6 cap (25 calls, measured from its transcript).
