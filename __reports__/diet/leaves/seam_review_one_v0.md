# Seam review 1: findings, fixes and the spend checkpoint

A fresh Opus reviewer wrote none of level 1. It read plan v2, the README purpose paragraph, `git diff
origin/main...origin/feat/integrity` at `4bc0d03b` and the level-1 leaf reports. Its report, kept as it returned,
is `integrity-tools/seam_review_one_reviewer.md`, outside the repository. It cost $4.87 against a $4 estimate and an
$8 cap.

## Findings

It found nothing that had to be fixed before level 2. It found eight should-fix findings and four to record. Each row
below gives the finding, the question it answers, and what was decided.

| # | Finding (question) | Decision |
|---|---|---|
| F1 | `replicates` has two readers, `check_replicates_input` and `Levels::replicates`, and they give one value two codes, one of them `experiment.cost_not_positive` (Q2) | Fixed (rename, every consumer in one commit, no alias) |
| F2 | An MCP server refusal names no field and carries no pointer (Q6) | Fixed |
| F3 | OpenCode has no chain from an agent to a sealed run whose outputs are read back (Q5) | Deferred to P7, with the observation that closes it (A1) |
| F4 | P1's hand-built `.claude` case holds for Claude Code only; a real OpenCode run stores `.claude/x.py` (Q1) | Deferred to P7 with F3 (A1) |
| F5 | The conformance collector is one of the three filters real runs apply, behind a function-pointer `Collector` seam (Q3, Q5) | Doc line fixed; the seam recorded, with its follow-up named below |
| F6 | Treatment levels are read in two CLI verbs, and failures of the second and later levels bypass `render_failures` (Q2, Q6) | Rendering fixed; the second reader in the verbs deferred to M7 |
| F7 | The published MCP server schema is a more lenient second reading than `mcp::read` (Q2) | Fixed (RiVault, reviewed) |
| F8 | `candler kill` refuses off a TTY before it resolves the store, and `--yes` is decided in two places (Q2) | Fixed |
| R1 | The 0555 materialise path guards an executable bit the runner never writes (Q1) | Fixed the other way: the runner records each output's execute bit (maintainer's ruling) |
| R2 | `at_plan_fixed` splices a resolution by hand; the CLI guard covers the real producer | Recorded: redundant, not wrong. It goes when the row's file is next touched |
| R3 | `harness_wrote` is typed by hand per driver; the nested `.opencode` exclusion is held by a second oracle | Recorded: the conformance oracle runs the core's collector; the typed list is the oracle's expectation, not a reader |
| R4 | b41's dry run lost its "only the test's own jobs" scope assertion | Recorded: b40's line count holds the same property on the live path |

F3 and F4 closing condition. P7's leaf, `dogfooding_pass.md`, gained this gate (amendment A1). After one real
OpenCode run of the pass, its stored outputs are compared with the files the agent wrote in its workspace, minus what
the driver's collect rule excludes. Reverting the conformance case alone does not give this producer-side observation.

R1. The maintainer ruled that every stored output keeps its execute bit, whatever produced it. An evaluation plugin
that runs an output as part of its own evaluation must be able to, because results are meant to be easy to consume.
So the 0555 materialise path stays, and runs Candler executes now record the real bit, as import already did. P1's
hand-sealed executable case gives way to two guards from real producers: a run whose agent writes an executable
output, and an import of a folder that holds one. The tree digest of a new run with an executable output changes,
which the ruling accepts. No existing fixture run has an executable output, so no fixture digest changed.

F5 follow-up. Before `feat/integrity` goes to `main`, the conformance-only public API of `candler-harness` goes,
together with the dev-dependency cycle it needs. That API is `Collector`, `Setup::with_collector`, `with_workspace`
and `with_collect_commands`. The cycle runs from `candler-harness` and both driver crates to `candler-core`, and it
leaves three copies of the `stored_paths` adapter. The oracle then reads the collection from a real run instead.
F3/F4's run under P7 is that observation for OpenCode.

## Fixes

Each guard was seen failing on its base before it passed on the tip.

| Finding | Where | Guard | PR |
|---|---|---|---|
| F8 | `candler/src/commands/kill.rs` | `kill_off_a_terminal_reports_the_store_before_the_confirmation` | #67 |
| F6 (rendering) | `candler/src/commands/resolve.rs` | `o1_resolve_reports_a_later_level_failure_like_a_first_one` | #67 |
| F2 | `candler-harness/src/mcp.rs`, `refusal.rs` | the `mcp` rows of `guard_silent_acceptance` ROWS pin `/mcp/servers/<key>/<field>` | #67 |
| F1 | `candler-schemas/src/execution.rs`, `resolve/layers.rs`, `runner/plan.rs` | `replicates_are_checked_once`; the replicates rows of `guard_silent_acceptance`; `o1_resolve_refuses_what_is_no_replicate_count_as_the_plan_does` | #67 |
| F5 (doc) | `candler-harness/src/conformance.rs`, the two driver conformance tests | none (comments) | #67 |
| F7 | `candler-schemas/src/servers.rs`, `mcp-server.schema.json` | `the_mcp_server_schema_refuses_what_the_reader_refuses` | #66 |
| R1 | `runner/evidence/tree.rs`, `runner/exec/gather.rs` | `an_executable_output_materialises_as_0555_and_a_plain_one_as_0444` (a real `candler run`); `an_imported_executable_materialises_as_a_private_0555_copy`, which also passes on the base because import already kept the bit | #68 |

The fixes left these points behind. Each is recorded, and none needs an action.

- F2 gives every `experiment.cell_value_invalid` the cell field path as its pointer, such as `/model/effort`.
  `experiment.setting_unsupported` points at the `[fixed]` key instead, such as `/fixed/model.effort`. So the two
  codes point at the same setting in two different shapes.
- A fault inside `args` points at `args`, because a field path cannot hold an index.
- F1 leaves `check_replicates_input` taking a `JsonValue` it no longer reads.
- The F7 schema cannot express the refusals `mcp::by_name` makes across a whole map: a duplicate rendered name,
  and the `__` rule.
- The F7 schema refuses `null` for `command` and `url`, which serde reads as absent. This predates the change.

Consistency check: `cargo nextest run --workspace` on the `feat/integrity` tip after the merges (`5b850c56`): 754
run, 754 passed, 31 skipped.

## Checkpoint

The spend below is measured from transcripts, not from usage notes. Each cap is the leaf's own.

| Level-1 row | USD | Cap | Within |
|---|---|---|---|
| P0 | 1.75 | 2 | yes |
| P1 | 7.82 | 20 | yes (about 81 tool calls against about 60: a call overrun, not a dollar one) |
| P2 | 2.38 | 8 | yes |
| P4 | 11.79 | 24 | yes |
| P6 | 1.17 | 2 | yes |
| Seam reviewer | 4.87 | 8 | yes |

The RiVault runs of level 1 are unmeasured on the RiVault side, because their logs were plain text.

**Decision on P5.** Level 1 is within its caps, so P5 runs, as the maintainer had also ruled. It is merged as #64.

Milestone spend so far is about $108.5 against a cap of about $120. That is about $82 at the level-1 handoff, this
level's lead session at $24.0, and the coordinator's spend since handoff. The lead session's $24.0 splits as the lead
$14.6, `seam_fixes` $3.9 against a $5 estimate, P5 $3.0 against $6, `exec_bit` $2.2 against $4, and reviews $0.3.
