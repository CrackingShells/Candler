# Briefing: M6c Integrity, level 3 planned (v1)

Supersedes `08-briefing_integrity_level3_v0.md`, after the maintainer's rulings on the level-3 plan.

## Verdict

Candler's product goal, from the README purpose paragraph:

> A domain-agnostic core for evaluating LLM-based agents. Candler has two equal halves behind one operation API:
> - **Execution:** run agents under controlled conditions (experiment matrix, runners, isolation, per-run tool-catalog snapshots, provenance).

Levels 1 and 2 of M6c gave each kind of stored state one producer, and each input one reader, on the paths a consumer uses. The dogfooding pass then showed where Execution still falls short:
- The stored "catalog snapshot" comes from a constant that always says `servers: []`.
- OpenCode's trace marks an MCP call `origin: other`, while Claude Code's marks the same call with its server, so the traces of one cell do not compare.
- Claude Code receives OpenCode's MCP keys verbatim.
- What each harness does with each setting is stated in five shapes, two of which let a driver say nothing.

So "controlled conditions" holds for one harness at a time, not across two. Level 3, harness parity, is planned and approved in its second revision (`07-plan_integrity_level3_v1.md`). It is on the roadmap as amendments A3 and A4, with Claude Code as the reference harness. No code has changed yet.

## Done

| Piece | What it gave Candler | How we know |
|---|---|---|
| Level 1: P0, P1, P2, P4 chain, P6 (#59–#63) | One producer for sealed runs; one oracle for collect; one typed reader per experiment input; kill off a TTY; the `AGENTS.md` principle and the leaf template | Each guard was seen failing on the base and passing on the tip (`__reports__/diet/leaves/*_v0.md`). Spend per row stayed within its cap: P1 $7.82 against $20, P4 $11.79 against $24. |
| Seam review 1 and its fixes (#66–#68) | The MCP schema refuses what its reader refuses; replicates are checked once; every stored output keeps its execute bit | 12 findings at $4.87, and the fixes at $6.1 |
| P5 catalog snapshot (#64) | A sealed run stores a catalog blob, and `inspect` shows it | Its guard passed, but the fake docker printed the same constant as production (finding (1)) |
| Seam review 2 and P7 dogfooding (#69) | An outside consumer ran a two-arm MCP experiment end to end on OpenCode and graded it | 6 manifests, each naming a blob that says `servers: []`, while the agent called `converter.convert` in every run (`05-findings_dogfooding_pass_v0.md`) |

## Why the plan changed

- **A3, level 3.** The dogfooding finding is a second producer (D1) standing in for a record Candler does not keep. Two rulings came with it: Claude Code is the reference harness, and a defect found on its side is fixed in both drivers. The README is to say "per-run tool census", and `codemode` defaults to `false`.
- **A4, revision 2 (this briefing).** The maintainer ruled on the first plan:
  - **One shape for every per-harness list.** The codebase's best list idiom is `CONFIGURATION_PATHS`'s const array of `Copy` rows. Its completeness check is the exhaustive match that `refusal_table!` uses. `SettingRow`, `Capabilities` and the capability page's hand-written rows fold into it. A missing answer is a compile error, not a default `false`.
  - **What the agent called is the consumer's to parse from the trace.** Candler owes a truthful record of what it installed and a trace that compares across harnesses. The census therefore keeps `census.tools` only, as the comparability verdict's input. The leaf that was to collect server status (`census.md`) is blocked, and `trace_parity.md` aligns both trace readers on one MCP origin instead.
  - **Code mode is declared, not parsed.** With code mode on, OpenCode's trace shows only `execute`. That is why the default is off, and OpenCode's answer says so.
  - **`tool-catalog-snapshot/1` stays.** The HTN consumer's capture script writes it and its grader reads it (htn-creator-eval@0.5.3). Only Candler's constant producer and the level-2 surface that showed its output go.
  - **Non-goals:** hooks an experiment can set, and CLAUDE.md/AGENTS.md. Both are left to a later feature round.
- The level's spend gate is unchanged at $70, and the leaves are now estimated at $45, with $90 in caps.

## Next

| Leaf | What it adds | What an experimenter can then do |
|---|---|---|
| P8 declaration | `Answer`, `AgentSetting` and `DriverFact`, each answered by every driver in an exhaustive match. Every configuration path says whether it reaches the agent. `SettingRow` and `Capabilities` are removed. | Know that no setting or harness behaviour goes unstated on either harness |
| P9 MCP render | Each driver writes its own MCP file from types. On Claude Code, `local` becomes `stdio` and `remote` becomes `http`, and Claude Code refuses code mode, with its reason. The `codemode` default is written into the cell. | Run one MCP cell on both harnesses and have each install the same server |
| P10 trace parity | OpenCode's trace attributes an MCP call to its server, as Claude Code's does, with an optional bare `tool` name that old traces can lack, and resolve refuses ambiguous server prefixes. The census stops naming servers, and the stored Claude Code trace keeps its raw `system/init` line. | Parse the same MCP call the same way from either harness's trace |
| P11 pages (RiVault, with a Sonnet review) | The capability and configuration pages are walks over the declaration and the drivers | Read one page that says, per harness, what happens to each setting |
| P12 snapshot out | The constant producer, its resolve step, `comparable.snapshots` and `InspectRun.catalog` are removed. `inspect` shows the resolved MCP config, once no `inspect` caller in htn-creator-eval@0.5.3 is found reading the catalog. The README, `AGENTS.md` and the live candler-v1 M6c Goal say "census". The old oracle vectors stay, for the maintainer's own stores. | See in `inspect` what Candler installed, and nothing it did not observe |
| P13 parity run | A container-e2e guard runs one cell on both harnesses against a real server. It asserts that the server is installed and that the call appears in the same trace form, and one live RiVault run per harness backs it. | Rely on CI to catch a harness falling out of parity |
| Seam review 3 | A cross-leaf review that asks first what the level left behind | |

The coordinator accepted the three compatibility items:
- P9: the digest changes once the `codemode` default is filled.
- P10: an optional `tool` field on the trace's MCP origin, holding the bare tool name.
- P12: a new digest version without snapshots, and `InspectRun.catalog` dropped, after P12 checks that no `inspect` caller in the HTN consumer reads it.

`/1` stays in `candler-schemas` for level 3. Seam review 3 records its long-term home as an open question.

## Goal check

| Part of the goal | Proven | Pending |
|---|---|---|
| "run agents under controlled conditions" | Per harness: isolation, budgets and permissions each have one reader | Across harnesses: one shape answered by each driver (P8, P9), and a parity guard (P13) |
| "per-run tool-catalog snapshots", to become "per-run tool census" | Claude Code's and OpenCode's census list the tools offered at session start, and the comparability verdict reads them | The constant producer removed (P12). Each MCP call traced in one form on both harnesses (P10) |
| "provenance" | Sealed runs, outputs with their execute bit, and the resolved values, `mcp.*` included, in the cell digest | `inspect` shows the installed MCP config instead of a constant snapshot (P12) |
| "domain-agnostic", "two equal halves behind one operation API" | Unchanged by level 3 | The op API still cannot be reached from outside (finding (2), outside level 3) |
