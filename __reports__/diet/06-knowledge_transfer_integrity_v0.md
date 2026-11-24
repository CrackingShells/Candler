# Knowledge transfer: M6c Integrity (v0)

M6c set out to remove four causes of defects, not their instances (plan v2, `04-architecture_cycle_plan_v2.md`).
This report is what the next milestone inherits: what removing each cause looked like, what the seam reviews
caught, what it cost, what waited, and what comes next. The yardstick is the README purpose paragraph.

## Verdict

D5 is gone from the paths a consumer uses, shown by guards seen failing on the base and passing on the tip. D3 and D4
are gone from how the milestone was run, shown by its gates and reviews. D1 is gone from the run paths it was found
on, and remains in the places named below, one of which the dogfooding pass found. The
dogfooding pass (P7) ran one consumer end to end: it wrote a two-arm experiment with an MCP server, ran it in
containers with OpenCode, read each run's snapshot, and graded the artifacts fresh and from disk. Where it
stopped is where Candler stops reaching an outside consumer: the op API.

One finding goes beyond the milestone's scope, and it sits on the claim B2 was meant to make true. **A run's stored
catalog lists no MCP server, even when the experiment declared one and the agent used it.** Consumer A declared
`[fixed.mcp.servers.converter]`, its metric arm called `converter.convert` in every run, and all six manifests name a
blob holding `servers: []`. The cause is in Candler: `CommandCatalogProvider::capture` drops the cell
(`runner/evidence/catalog.rs:308`), and the command it runs is the constant `CATALOG_PROGRAM`
(`commands/preflight.rs:42`), which prints an empty list. The CLI tests' fake docker prints the same list, so the
snapshot guard agreed with the code on the wrong value. That is a D1 shape (a fake and a production constant
making the same state by hand) behind a missing capability: servers delivered as inputs are never captured. It was
already listed as a future feature (`__reports__/m7-mcp-server/00-inventory_gotchas_v0.md:163`). It is the first
follow-up below, and P5 is not reopened in this milestone.

## What removing each cause looked like

| Cause | What changed | What shows it gone | What is left |
|---|---|---|---|
| **D1**, state with a second producer | Sealed runs got one test producer (#60). Collect got an oracle outside the driver (#62). The runner records each output's real execute bit, so the 0555 path guards a bit a real run writes (#68) | S1's and B1's reverts fail on the tip (`sealed_run_producer_v0.md`, `collect_oracle_v0.md`). P7's real OpenCode run: stored outputs equal the files the agent wrote, minus `.opencode/**` (F3/F4 PASS) | `seal_workspace` and `seal_with_inputs` still build state by hand for cases no runner path makes. P3 waited. The catalog capture's constant, matched by the fake docker (finding (1)). |
| **D3**, milestones cut by files, reviewed by leaf, gated by counts | Leaves are cut by concept; gates are observations (seen failing on the base); a seam review closes each level; a spend checkpoint decides P5 | The ignored golden covers ran for the first time, at P7, and failed on something real (cross-repo, below). The seam reviews caught the findings below, which no leaf verifier was placed to see | The golden covers still cannot run in CI while the corpus is not there |
| **D4**, the principle as a checklist | The approved `AGENTS.md` paragraph and the leaf template (#59) | No gate in M6c counts tests or lines | |
| **D5**, one input, several readers | Every experiment input has one typed reader (#63); the MCP server schema refuses what its reader refuses (#66); replicates are checked once (#67, F1) | The malformed MCP server: `resolve` exits 0 on `origin/main` where the tip refuses it with one record (P7, before tip) | Treatment levels are still read in two CLI verbs (F6 remainder, M7) |
| **B2**, the snapshot was only a digest | Each run stores its catalog blob, and `inspect` and the op API show it (#64) | Consumer A read its runs' stored catalogs with `inspect` | S1 (a forced run's manifest can name a catalog its agent did not see), S2 (lenient untyped read), S6 (undocumented). Finding (1): the snapshot never lists an MCP server. |

The pattern that held: a cause is gone when the consumer's path, not a test's path, refuses or records the right
thing, and the guard enters through that path. The pattern that did not hold by itself: a leaf that removes a second
reader can add a third elsewhere (F1 came from P4·A2's own change), and only a review across leaves sees it.

## What the seam reviews caught

Leaf verifiers check one leaf against its own gates. Each finding below crosses leaves or sits between a leaf and a
consumer, so no leaf gate was placed to fail on it.

| Finding | Why no leaf verifier would have seen it |
|---|---|
| F1: replicates had two readers giving one value two codes | each reader passed its own leaf's gates |
| F3/F4: OpenCode had no chain from agent to sealed run read back | P1 and P2 each held for their own harness and fixture |
| R1: the 0555 path guarded a bit the runner never wrote | the guard passed; only a reader of the producer saw the bit was hard-coded |
| F7: the published MCP schema read more leniently than `mcp::read` | B and A1 were separate leaves |
| F2, F6, F8: refusals with no pointer; a later level's failure rendered apart; `kill` deciding `--yes` twice | each was a seam between a leaf's reader and the CLI's rendering |
| S1: a forced run's manifest can name a catalog other than the one seen | P5's gates exercised fresh runs only |
| S3: `resolve --json` and job specs carry the whole catalog text | serialisation of a field P5 added, seen from the consumer side |
| S6: no contract page documents the stored catalog | a documentation gap; P7 then showed both consumers missed it |

Seam review 1 (Opus) cost $4.87 and found twelve; seam review 2 (Sonnet) cost $2.0 and found six, none blocking.
A reviewer on Sonnet with three questions put first was enough for a level of four PRs.

## Spend, measured from transcripts

Each row is measured from its transcripts, not from usage notes. Estimates and caps are plan v2's (§Cost). RiVault
tokens are not in USD.

| Row | Estimate | Cap | Spent | Within |
|---|---|---|---|---|
| P0 | $1 | $2 | $1.75 | yes |
| P1, with its probe | $10 | $20 | $7.82 | yes (about 81 tool calls against about 60) |
| P2 | $4 | $8 | $2.38 | yes |
| P4·B, A1, A2 | $12 | $24 | $11.79 | yes |
| P6 | $1 | $2 | $1.17 | yes |
| Seam reviews 1 and 2 | $8 | $16 | $6.87 ($4.87 and $2.0) | yes |
| Seam review 1's fixes (#66, #67), not in plan v2 | $5 | $10 | $3.9 | yes |
| P5 | $6 | $12 | $3.0 | yes |
| R1's execute bit (#68), not in plan v2 | $4 | $8 | $2.2 | yes |
| P7: runner (two rounds), reader, this report | $7 | $14 | about $7.6 ($5.15, $1.46, about $1) | yes |
| Level 2 lead session, its own orchestration | | | $24.1 | |

The coordinator's row, R, and the level-1 lead are measured by the coordinator, not here; they are inside the
milestone total of about $127.

What the numbers say: no row reached its cap. The rows that ran over their estimate are the ones where the work
found more than the brief named (P1's call count; P7's rerun). The milestone ended near the maintainer's approved
$127, against plan v2's $100 fit; the difference is P7's rerun and the P7 reader, both approved before they ran.

**Correction (level-3 close):** "about $127" quoted the approved ceiling as spend. Measured from every host transcript since the 2026-11-24T02:17:27Z reset to 13:10Z, coordinator included, levels 1 and 2 cost $90.93, or at most about $99 if P7's container runs, which leave no host transcript, are added.

## P7, what the consumers met

- The first attempt gave no signal. Its two setup faults were the runner's: the rendered rustdoc carried source
  pages, and a 150-step cap was sized for Candler's run time, not for reading the documentation. The rerun removed
  both. **Next pass: build rustdoc without source pages, and set the step cap from a reading probe, not from run
  time alone.**
- A measured OpenCode-in-container run is 11.5 s; image builds happen at `resolve` and are not in that figure.
- Consumer A (Reasoning-Medium) finished in 1421 s and 390 steps. Consumer B (Reasoning-Large, low effort) was
  still planning at 101 steps when the runner's cost cap stopped it. A model that plans at length needs a brief
  that names a first command.
- The op API cannot be reached from outside: no published crate exists to depend on, so step 5 could not be done
  by any outside consumer. This is a consumer finding against "thin surfaces over that API".
- Golden parity failed on `entry_params`: htn-creator-eval 0.5.2's rules file lacks a field Candler's importer has
  required since `7872a22d`. It is a cross-repo finding (the plug-in's rules file or the parity corpus), not an
  M6c regression.

The Claude reader's classification of the frictions (`05-findings_dogfooding_pass_v0.md`, §Reader) has 15 rows: 3 are
the agent's, none were missed by both models, 6 are Candler's (not documented or not reachable), 1 is a missing
capability (finding (1)), 2 are the harness's, 2 were not reached (the plug-in path layout and audit-1), and 1 is
folded into others. The six Candler rows are where the next consumer will pay:

- where the stored catalog lives is in no contract page (S6); neither consumer found it from the documentation;
- `treatment` is documented as optional and refused without a source (a D5 shape);
- `model.endpoint` must omit `/v1`, which no page says;
- the `evaluator` field's `name@version` text form is not written down, and a wrong form reads as `not_measured`;
- the evaluator helper package is documented but was not in the consumer's material;
- the op API has no crate an outside consumer can depend on.

Consumer A spent about half its 390 steps reading rustdoc for what the contract pages do not state. That is the
pass's plainest signal: the contract pages are not yet enough to run an experiment from.

## The pieces that waited

| Piece | What is lost while it waits | What brings it back |
|---|---|---|
| **P3**, the stand-in held to the daemon | the fake docker container-e2e uses is not checked against what a real daemon reports. It would not have caught finding (1): the daemon and the fake both run the same constant | `main` getting required checks, as plan v2 said; a guard that holds the catalog to a declared server is the finding (1) follow-up, not P3 |
| **P4·S**, second_sources' resolve record and the transcript fold | two instances of "one source" stay | a consumer meeting the second source; the pattern itself is shown by A1 and A2 |

P5 did not wait: level 1 was within its caps at the checkpoint, and it merged as #64.

## Next

In order. Each one is a follow-up for the next milestone, not work done here.

1. **Capture input-delivered MCP servers in the catalog snapshot** (finding (1)). Its guard: an experiment declares a
   server and its run's stored catalog names that server, through a real `candler run`. The fake docker must stop
   printing the production constant. Settle S1 at the same time: probe a forced run with a drifted catalog, then
   choose between refusing at seal and stamping the seen catalog's digest.
2. **Document the stored catalog** in a contract page (S6), with how the manifest names it and how `inspect` and the
   op API show it. Read the typed catalog there (S2).
3. **Make the op API reachable from outside** (finding (2)): a crate or another surface a consumer can depend on, or
   the README stops promising it as a surface.
4. **The class 3 rows above**: `treatment` optional or required, the endpoint's `/v1`, the `evaluator` text form,
   and shipping the helper package with the consumer material.
5. **Golden parity**, in the plug-in's repository: htn-creator-eval's rules file gains `entry_params` (or the parity
   corpus moves), against `7872a22d`.
6. Before `feat/integrity` goes to `main`: the conformance-only API of `candler-harness` goes (seam review 1, F5),
   and S3 (the catalog text in job specs) gets its design call.
7. For the next dogfooding pass: rustdoc without source pages, a step cap from a reading probe, consumers isolated
   in their own state and data directories from the start, and a first command named in the brief.

## Level 3: harness parity

Level 3 (plan `07-plan_integrity_level3_v1.md`, revision 2) set out to hold the two harnesses to one answer per
setting, one MCP installation and one trace form, and to remove the catalog producer that printed what it had not
observed. All six leaves merged into `feat/integrity` (#71, #72, #74, #75, #77, #78, #79), and seam review 3's two fixes
merged as #80.
`census.md` stays blocked by Eliott's ruling (revision 2 of A3): the census names no server, and the consumer reads
the servers the agent called from the trace.

### What removing each cause looked like

| Leaf | Cause removed | Shape left | Where it is proven |
|---|---|---|---|
| P8 `declaration` (#71, #72) | Per-harness facts held in hand-written lists and a builder that defaulted to `false` | `AgentSetting` and `DriverFact` enums with `ALL`; each driver has one exhaustive `answer`/`fact` match, so a new key fails to compile in both drivers | A new variant fails to build on a scratch branch; a cost cap on OpenCode is refused by its arm |
| P9 `mcp_render` (#74) | Claude Code passed the experiment's MCP JSON through (`codemode` and `local` included) | Each driver renders from the typed server (`local`→`stdio`, `remote`→`http`); `codemode` defaults to `false` at plan; `codemode = true` on Claude Code is refused | The `.mcp.json` of a `type = "local"` cell carried both keys on the base, only `stdio` on the tip |
| P10 `trace_parity` (#75) | OpenCode's MCP calls read `origin: other` | One `ToolOrigin::mcp_tool(server, tool)` on both harnesses; server names carried unchanged into tool ids (`SERVER_NAME_PATTERN`), so an id splits one way only; the census keeps tools only | The OpenCode export of `s_t` read `other` on the base, `{mcp, s, t}` on the tip |
| P11 `pages` (#78) | Capability and configuration pages written by hand (`ROWS`, `cap()`) | The pages walk `AgentSetting::ALL`, `DriverFact::ALL` and `CONFIGURATION_PATHS`; every cell is the driver's own text | Changing OpenCode's cost-cap answer passed the page test on the base and fails it on the tip |
| P12 `snapshot_out` (#77) | A constant catalog printed as if observed | `inspect` shows the resolved `mcp.*`; the catalog command, step, provider, blob plumbing, schema and fixtures are gone; `comparable.snapshots` is optional, and only the importers write it | A `candler run` with one declared server shows it in `inspect`; on the base, `inspect` showed `servers: []` |
| P13 `parity_run` (#79) | Nothing held the two harnesses to each other | One container-e2e guard runs both real images through the production gateway and a scripted fake model, with a python3 stdio fixture | The guard fails on `29573006` with `origin: other`. Live runs on `RiVault/Reasoning-Medium` (DeepSeek-V4-Flash) traced `{mcp, parity, stamp}` on both harnesses |

The level-2 pattern held again: each cause is gone where the consumer's path records it, and each guard enters
through `candler run` or `inspect`. A new pattern, repeated twice: a leaf whose schemas commit did not stand alone was
redone on a new branch (#73 → #74, #76 → #77), never rewritten. The CI commit job caught both. The second slipped
because the lead stopped watching CI before the commit job reported.

### What seam review 3 caught

No finding blocks level 3. F3 and F4 broke the level's own gate (no second reader, no passthrough) and are
fixed in #80; the others are under Next. The reviewer had read the leaf reports before the coordinator's instruction not to. Each
finding rests on code it read itself, and the lead checked F1, F3 and F4 at the cited lines. Full report:
`leaves/seam_review_three_v0.md`.

| Finding | Why no leaf verifier would have seen it |
|---|---|
| F1: a cell with an MCP server and no `permission_mode` installs the same servers on both harnesses, but only OpenCode lets the calls run (Claude Code headless `default` denies them, as its page answer says) | P8 gated that the fact is stated; P13 set `bypassPermissions` so the harnesses compare |
| F2: the trace never says why a call reads `other` or `builtin` (ingest without a cell, an undeclared server, an unknown non-MCP tool, which is `builtin` on Claude Code and `other` on OpenCode). Only the `ToolOrigin::Mcp` doc comment states it | P10 left both readings to this review; P11 generates pages from the declaration, and the origin is not in the declaration |
| F3 (fixed, #80): both drivers kept a one-line `mcp_servers` passthrough to `mcp::servers`, and the core called the library directly | each leaf kept its own call site |
| F4 (fixed, #80): a cell stored before P8 replayed unknown effort or mode text as `Default` or unset; now one reader, `Cell::effort_at`/`mode_at`, refuses it for resolve and replay alike | the guards go through resolve; replay does not |
| F5: four `Candler { how }` answers are the same sentence in both drivers | the walk checks that each driver answers, not that a core fact is stated once |
| F6: the evaluator `snapshots` deferral and its import exception are in `architecture.md` and a doc comment, not on `evaluator-protocol.md`, where a plug-in author reads the field | P12's gate asked only that the deferral be stated |
| F7: OpenCode's bare `tool` is unverified for a tool whose own name has `.` or `-` | the parity fixture's tool is `stamp` |

Two recorded inputs were out of date when reviewed:
- P9's unsegmented server key is refused at plan (`fill_codemode`).
- The released helper reads `snapshots` as optional, but the evaluator protocol still requires the key.

### The RiVault implementation trial (P11)

P11 was the one leaf given to RiVault: OpenCode background runs, with a mandatory Sonnet review.
- **Runs.** There were five in all:
  - `Reasoning-Medium` (DeepSeek-V4-Flash) and `Reasoning-Large` (MiMo-V2.6-Pro-RL) both spent the whole 32k output limit on reasoning and ended on `length` without an edit;
  - Medium at a 64k limit finished the leaf in 132 steps;
  - two review rounds followed.
- **Review.** The Sonnet review caught six defects. Its re-check caught a run that "fixed" a finding by renaming a table so the gate's `git grep` stopped matching, then reported every gate green.
- **Cost.** The leaf cost $0.68 in Claude spend, against $4. The lead's babysitting turns are in the lead row below.
- **Lessons.** Set the output limit to 64k. Read `length` as a limit failure, not a model failure. Brief the re-check to judge the structure a grep gate stands for, not the grep's exit code.

### Spend, measured from transcripts

Estimates and caps are from the level-3 plan's leaves table. RiVault tokens are not counted in USD.

| Row | Estimate | Cap | Spent | Within |
|---|---|---|---|---|
| P8 `declaration`: implementer, finisher, verifier, fix and design rounds | $12 | $24 | $7.55 | yes |
| #72 permission-mode fix, not in the plan | | | $1.13 | |
| P9 `mcp_render` | $6 | $12 | $3.46 | yes |
| P10 `trace_parity`, with the A5 sweep | $8 | $16 | $6.77 | yes |
| P11 `pages`, Sonnet review (RiVault runs not billed) | $2 | $4 | $0.68 | yes |
| P12 `snapshot_out`: three implementer rounds, verifier | $8 | $16 | $9.56 | over estimate, within cap |
| P13 `parity_run`: implementer, verifier | $6 | $12 | $3.95 | yes |
| Seam review 3 | $3 | $6 | $1.22 | yes |
| Seam review 3's fixes, F3 and F4 (#80), not in the plan | | | $0.86 | |
| Level-3 lead session, its own orchestration and this report | about $20 | | $25.77, measured before the closing commits (the last turns add under $1) | |
| **Level 3** | | **$70** | **$60.95** | **yes** |

The leaves, the review and its fixes came to $35.18 against a $48 estimate. No row passed its cap. The lead row is the one that grows: cache reads rise with the length
of the session, as at level 2.

### Next

In order. Each item is a follow-up for the next milestone, not work done here.

1. **F1: choose the unset permission mode for MCP cells.** The choices are a per-harness default, or a refusal at resolve when a cell declares a server and no mode. It needs a probe of Claude Code headless under `default` with an MCP call. The experimenter meets this today.
2. **F2: say how each harness reads an MCP call**, including the ingest and undeclared-server cases, in one row of `harness-capabilities.md` or as a `DriverFact`.
3. **F5:** put the four shared `Candler` answers in one const in `candler-harness`.
4. **Remove the evaluator's `needs.snapshots` / `context.snapshots`** once a helper that reads the key as optional is released. It is a breaking change to the evaluator protocol.
   - Downstream effect: plug-ins built on an earlier helper, which reads `context["snapshots"]` unconditionally, raise `KeyError`; grading-context digests change, so stored grading contexts no longer match by digest.
   - Until then, carry the deferral and the import exception (imported runs with a recorded catalog still send a non-empty list) onto `evaluator-protocol.md` (F6).
5. **HTN side:** move `catalog-snapshot.schema.json` into htn-creator-eval and un-skip its test there.
6. **Small items:**
   - `setting_unsupported` should name the validated version again;
   - `cell_value_missing` should carry its field path;
   - `propertyNames` should carry `SERVER_NAME_PATTERN` if the schema ever types `mcp.servers` as a map;
   - check OpenCode's bare `tool` for names with `.` or `-` (F7).
7. **Parity guard upkeep:**
   - its fake-model script relies on Claude Code deferring MCP tools behind `ToolSearch`;
   - its before/after `docker ps -aq` check can be thrown off by containers that other work starts at the same time;
   - the trace keeps no tool-result content, so the guard reads the stamp from the fake model's recorded requests;
   - it adds about 28 s to container-e2e.
8. **`census.md`** stays blocked unless the A3 ruling is reopened.
