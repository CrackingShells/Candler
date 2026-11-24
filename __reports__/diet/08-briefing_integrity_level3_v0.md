# Briefing: M6c Integrity, level 3 planned (v0)

## Verdict

Candler's product goal, from the README purpose paragraph:

> A domain-agnostic core for evaluating LLM-based agents. Candler has two equal halves behind one operation API:
> - **Execution:** run agents under controlled conditions (experiment matrix, runners, isolation, per-run tool-catalog snapshots, provenance).

Levels 1 and 2 of M6c made each kind of stored state have one producer and each input have one reader, on the paths a consumer uses. The dogfooding pass then showed where Execution falls short. Under OpenCode, a run's record names no MCP server. The stored "catalog snapshot" comes from a constant that always says `servers: []`. And nothing declares, per harness, what happens to each setting an experiment makes. So "controlled conditions" holds for one harness at a time, not across two, and the "snapshot" is not an observation. Level 3, harness parity, is planned and approved. It is on the roadmap as amendment A3, with Claude Code as the reference harness. No code has changed yet.

## Done

| Piece | What it gave Candler | How we know |
|---|---|---|
| Level 1: P0, P1, P2, P4 chain, P6 (#59–#63) | One producer for sealed runs, one oracle for collect, one typed reader per experiment input, kill off a TTY, the `AGENTS.md` principle and the leaf template | Each guard was seen failing on the base and passing on the tip (`__reports__/diet/leaves/*_v0.md`). Spend per row stayed within its cap (P1 $7.82 against a $20 cap; P4 $11.79 against $24). |
| Seam review 1 and its fixes (#66–#68) | The MCP schema refuses what its reader refuses, replicates are checked once, and every stored output keeps its execute bit | 12 findings at $4.87, and the fixes at $6.1 |
| P5 catalog snapshot (#64) | A sealed run stores a catalog blob, and `inspect` shows it | Its guard passed, but the fake docker printed the same constant as production (finding (1)) |
| Seam review 2 and P7 dogfooding (#69) | An outside consumer ran a two-arm MCP experiment end to end on OpenCode and graded it | 6 manifests, each naming a blob that says `servers: []`, while the agent called `converter.convert` in every run (`05-findings_dogfooding_pass_v0.md`) |

## Why the plan changed

- **A3, level 3 (this amendment).** The dogfooding finding is a second producer (D1) sitting behind a capability Candler lacks. A record that is not observed, and settings that no harness declares, make every parity comparison an act of faith. Eliott approved the direction and made four rulings:
  - Claude Code is the reference harness;
  - a defect found on the Claude Code side is fixed in both drivers;
  - the README says **"per-run tool census"**;
  - **`codemode` defaults to `false`**, so by default the agent sees MCP tools directly on both harnesses.
- The amendment rewords the campaign goal, its done gate and its spend gate in the same change. Levels 1 and 2 stay within the $127 Eliott approved, and level 3 gets $70.

## Next

| Leaf | What it adds | What an experimenter can then do |
|---|---|---|
| P8 declaration | `AgentSetting`, one typed list of what reaches the agent. Every driver must answer every entry: mapped, provided by Candler, or refused with a reason. A new entry is a compile error until every driver answers it. | Know that no setting is silently dropped on either harness |
| P9 MCP render | Each driver writes its own MCP configuration from types. Claude Code stops receiving OpenCode keys. The `codemode` default becomes `false` and is written into the resolved cell. | Run one MCP cell on both harnesses and have each agent see the same tools |
| P10 census | The census names servers, their status and their tools on both harnesses, including OpenCode code mode | Read from the run record which servers and tools the agent had |
| P11 pages (RiVault, with a Sonnet review) | Capability and configuration pages generated from the declaration, plus a census page | Read one page that says per harness what happens to each setting |
| P12 snapshot out | The constant producer and its matching fake are removed. The cell digest compares the resolved servers. The README and `AGENTS.md` say "census". | Trust that the record of tools is an observation, and compare cells by the servers they declare |
| P13 parity run | A container-e2e guard runs one cell on both harnesses with a real server, plus one live RiVault run per harness | Rely on CI to catch a harness falling out of parity |
| Seam review 3 | A cross-leaf review that asks first what the level left behind | |

The estimates total $47, with $96 in caps, and lead orchestration is about $20. Two decisions are open before P9 and P12, and both go to the coordinator: the digest change from filling the `codemode` default, and dropping the snapshot from `comparable` under a new digest version.

## Goal check

| Part of the goal | Proven | Pending |
|---|---|---|
| "run agents under controlled conditions" | Per harness: isolation, budgets and permissions each have one reader | Across harnesses: one declaration answered by each driver (P8, P9), and a parity guard (P13) |
| "per-run tool-catalog snapshots", to become "per-run tool census" | Claude Code's census lists tools and servers from `system/init` | OpenCode's servers and tools (P10); the constant producer removed (P12) |
| "provenance" | Sealed runs, outputs with their execute bit, and the catalog digest in the manifest | A digest that compares the resolved servers instead of a constant snapshot (P12) |
| "domain-agnostic", "two equal halves behind one operation API" | Unchanged by level 3 | The op API still cannot be reached from outside (finding (2), outside level 3) |
