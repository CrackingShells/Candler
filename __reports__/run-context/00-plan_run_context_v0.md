# Plan: the evaluator run context carries what a sealed run stored (v0, approved 2026-11-25)

## Context

The README's purpose paragraph is the yardstick, quoted verbatim:

> A domain-agnostic core for evaluating LLM-based agents. Candler has two equal halves behind one
> operation API:
>
> - **Execution:** run agents under controlled conditions (experiment matrix, runners, isolation,
>   per-run tool census, provenance).
> - **Evaluation:** grade traces and artifacts, fresh or already on disk, through pluggable
>   evaluators that emit one outcome record format.

Today an evaluator can only be handed the artifacts half of "grade traces and artifacts". The wire
already has `context.trace` and `context.raw[]`, and `needs.trace` is a declared field, but
`assemble` never fills them and silently ignores `needs.trace = true`
(`crates/candler-core/src/evaluation/context/assemble.rs`, doc line 99; ruling
`__reports__/m2a-evaluation-parity/l1/00-rulings_v0.md:75`). AGENTS.md calls that silent, more
lenient reading a defect.

"Sealed" means complete and protected from overwrite, nothing more: `RunWriter::done` writes
`run.done` last and "nothing in the run directory changes afterwards"
(`crates/candler-core/src/store/run.rs:239`), a finished run is refused by `RunWriter::create` and
by `seal_interrupted`, and evaluation reads only sealed runs so it never sees a half-written one.
The seal hides nothing from readers. Because a sealed run never changes, a digest taken at assembly
holds for the whole evaluation.

The feature's intent is general: an evaluator can be handed any part a sealed run stored (trace,
raw streams, manifest), the same way it is handed artifacts today. htn-creator-eval is the first
consumer to need it (its probe P0 grades one run with a kind that needs artifacts and the trace, and
it wants each shell call's command and exit status and each MCP call's `{server, tool}`), but the
design answers the intent, not that probe: every choice below reuses an idiom Candler already has,
and the one structural change is a refactor that leaves `materialise` simpler than it is.

Settled with the maintainer (2026-11-25):

| Question | Ruling |
|---|---|
| Exit status | Reuse `candler_schemas::process::ExitStatus` on the tool result |
| Releases | One release carries the whole feature (trace, exit status, raw, manifest, helper, consumer-path guard) |
| Blinding | No new concept: a kind's declared needs already state what it sees (see below) |

## What the trace already holds, and what it lacks

- **MCP origin:** held. `ToolCall.origin = Mcp { server, tool }` (`crates/candler-schemas/src/trace.rs:645`).
- **Command text:** held. `ToolCall.argv = [input.command]` for Claude Code's `Bash`
  (`crates/candler-harness-claude-code/src/parse.rs:359`) and OpenCode's `shell`
  (`crates/candler-harness-opencode/src/export.rs:180-219`).
- **Exit status:** lacking. `ToolResult` keeps only the harness's `is_error`. The raw streams hold it:

| Harness | Exit 0 | Nonzero | Interrupt | Denial | Timeout |
|---|---|---|---|---|---|
| Claude Code (real fixtures) | `is_error: false` | `is_error: true`, content starts `Exit code N` (1, 127 seen); no numeric field | `tool_use_result.interrupted`, never seen true | `tool_result_meta[].non_execution_kind` | no sample |
| OpenCode 2.2.7 (real binary, fake model) | `status: completed`, `metadata.exit: 0` | no fixture (expected `completed`, `metadata.exit: N`) | `status: error`, `error.type: aborted`, no exit | `status: error`, `error.type: permission.rejected` | no fixture |

## The exit-status field

`ToolResult` gains `exit: Option<ExitStatus>`, the same tagged object every supervised process
carries (`crates/candler-schemas/src/process.rs:284`: `{kind: code, code}` | `{kind: signal, signal}`).
It is absent when the call is not a shell call, was denied, or the harness gave no code (interrupt,
timeout). Candler never infers a signal from a code such as 137; the `signal` variant stays unused
until a harness reports one. `is_error` keeps meaning "the harness said so" and is not comparable
across harnesses for shell calls; the contract page says so.

Reader rules (the call's name comes from the call joined on `(session, span)`):

- Claude Code, `Bash` result: content prefix `Exit code N` gives `code N`; `is_error: false` gives
  `code 0`; `interrupted: true`, a denial, or any other error text gives no exit.
- OpenCode, `shell` result: `metadata.exit: N` gives `code N`; `error.*` gives no exit.

The optional field keeps the marker `normalised-trace/1` (old traces lack it and still read); the
schemas crate bumps its minor. The schema leaf confirms this against `__design__/architecture.md`
§Wire encodings before blessing.

## Each need

| Need | Today | After | Served as |
|---|---|---|---|
| `artifacts` | served | unchanged | `<ws>/files/...`, digest-checked copies |
| `inputs`, `metadata` | served | unchanged | as today |
| `snapshots` | refused `Unsupported` | unchanged | — |
| `trace: bool` | silently ignored | served | `<ws>/trace.jsonl` (0444), `context.trace` |
| `raw: bool` (new) | — | served | `<ws>/raw/<stream>.jsonl.gz`, `context.raw[]` (field exists) |
| `manifest: bool` (new) | — | served | `<ws>/manifest.json`, new optional `context.manifest` |

- The manifest carries usage, the tool census (`run.tool_calls`), `run.exit`, `run.ended_by`,
  factors and provenance, so "the rest of the stored run" is one file, not a family of needs.
- `tree.json` is not served: artifacts already give the files with digests.
- New `Needs` fields default to `false`, so an existing plug-in's `describe` stays valid. A run
  that lacks the requested part (an imported run with no trace) is refused with a new
  `ContextError` variant and refusal code, never served empty.
- **Refactor, one copy path:** `materialise.rs` has one copy routine, `copy_artifact` (create-new,
  0600, digest check, then 0444/0555), written for blobs. It becomes the single routine that copies
  any stored file into the workspace given a source path and an expected digest; artifacts, inputs,
  trace, raw streams and manifest all go through it, and `Assembled` carries one list of
  `(stored source, workspace path, digest)` instead of a field per part. Adding a part then costs a
  line in `assemble`, not a new copy path. The store records no trace or manifest digest, so
  `assemble` hashes those once and the copy is checked against that hash, as blobs are.

**Wire shape of a served file** (settled at roadmap authoring). `context_digest` is a pure function
of the wire `RunContext`, so each served file's digest travels on the wire, in the idiom
`ContextArtifact` already uses (`{path, abs, sha256}`: run-relative path, where the evaluator reads
it, digest). `context.trace` and `context.manifest` are that object (`{path: "trace.jsonl", abs,
sha256}`), and `ContextRaw` gains `sha256`. `context.trace` has never been filled, so changing it
from a path string to that object breaks no context Candler ever wrote.

**Context digest** (`RunContext::context_digest`, used in `I4Key` for outcome reuse): each served
part adds its key (`trace: <sha256>`, `raw: [{stream, sha256}]`, `manifest: <sha256>`) only when
served, so every existing digest (golden imports, oracle vectors) is unchanged.

## What a kind can see

Candler has no blinding concept, and this feature adds none. The idiom that already covers it: a
kind declares its needs in `describe`, Candler serves exactly those and refuses what it does not
understand, so what a kind saw is stated by the kind itself. The trace, raw streams and manifest name
the model and the treatment's effects; whether seeing them biases a grade is domain science, and the
core names no domain. The contract page states what each part reveals, once.

This agrees with the instruction-files feature (M6.5, plan v2): its v0 blinding gate G10 was
dropped, the maintainer ruled on 2026-11-25 that evaluator access to instructions is "deferred until a
consumer brings a reason", and G10 now names `experiment.instructions_collide` (the brief's
description of G10 is out of date). M6.5's new `context_start {system, first_message}` trace event
puts instruction texts in the trace; a kind that declares `needs.trace` sees them, as it declared.

## Protocol, schemas, helper

- `crates/candler-schemas/src/trace.rs`: `ToolResult.exit`.
- `crates/candler-schemas/src/evaluation.rs`: `Needs.raw`, `Needs.manifest`; `RunContext.manifest`;
  `context_digest` keys.
- Regenerate `crates/candler-schemas/schema/*.json` (`CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p
  candler-schemas drift`) and the contract pages `docs/contract/{evaluator-protocol,
  evaluator-helper}.md`; the outcome record is unchanged.
- `python/candler-evaluator`: `RunContext.manifest`; typed trace models generated from
  `normalised-trace.schema.json` by `scripts/gen_models.py`; a reader that yields typed events from
  `context.trace`; helper minor bump.
- `assemble.rs` doc and the M2a ruling: a new ruling note replaces "trace not served".

## Release and order

1. This feature lands on one integration branch cut from `origin/main`, merged to `main` through one
   PR after the maintainer's go; the CI release cuts it (expected workspace v0.17.0, `schemas@0.14.0`).
2. As soon as the plan is approved, the field names go to the HTN Phase coordinator (the consumer
   contact), so their probe kind can be written in parallel.
3. HTN pins the release and runs its probe P0; on pass, its redesign is unblocked.
4. M6.5 rebases its `trace_context` leaf onto this release: both edit `trace.rs` and bump the schemas
   minor, and this one unblocks the consumer. No external users, so no compatibility shims.

## Test plan

Roles: a **guard** stays in the suite and carries the claim; a **probe** answers a question that a
decision waits on, its answer is recorded and its code deleted; a **confirm** checks one build step
and is culled. Each guard below extends a guard the suite already has wherever one owns the
behaviour, keeps the doc shape `Guards: / Oracle: / Bite:`, and takes its expected values from a
source independent of the code under test.

### Behaviours, and who would be hurt if they broke

| # | Behaviour | Who relies on it | Kind of risk |
|---|---|---|---|
| B1 | A shell call's exit status in the trace is the one the harness reported, and absent when it reported none | every plug-in that grades agent behaviour | seam with harnesses Candler does not own |
| B2 | A kind that declares `trace`, `raw` or `manifest` gets exactly the stored bytes, in its private workspace, read-only | every plug-in author | design cornerstone (R9, I11) |
| B3 | The context digest covers what was served, and is unchanged for a context that serves none of the new parts | outcome reuse (`I4Key`), golden imports | cornerstone: when it breaks, reuse breaks silently |
| B4 | A need the run cannot meet is refused with a code, never served empty | plug-in authors; AGENTS.md's one-reader rule | silent acceptance |
| B5 | The wire refuses what it does not understand (`exit`, `Needs.raw`, `Needs.manifest`) | the one reader per input | lenient second reading |
| B6 | The Python helper hands the kind the paths and a typed trace | plug-ins written with the helper | consumer interface |
| B7 | A real agent run, graded, shows its plug-in each shell call's command and exit, and each MCP call's `{server, tool}` | the consumer path end to end | seam: the whole chain on real harnesses |

### Probe (its own step, before the readers)

**P-shell** (leaf `probe_shell_shapes`): what do real Claude Code and real OpenCode 2.2.7 write for
a shell call that exits nonzero, one that writes only to stderr and exits nonzero, and one ended by
the harness's own timeout? It runs the `parity_e2e.rs` rig with a scripted fake-model turn.
Decisions it feeds: the OpenCode reader rule for nonzero exits (no fixture exists today); whether
either harness gives a code on a timeout (if neither does, `exit` stays absent and no timeout guard is
written); whether Claude Code's `Exit code N` prefix holds when stdout is empty. Kept afterwards:
the recorded raw streams, as reader fixtures made by the real harnesses, and the answer in the leaf
report. Deleted: the probe's test code.

### Guards, by leaf

| Leaf | Guard (extends) | Behaviour | Scope, and why | Oracle | Bite (seen red) |
|---|---|---|---|---|---|
| `schema_exit_needs` | `exit` rows in the schemas crate's trace reader table | B5 | unit at the one reader that every trace consumer goes through | hand-written lines: valid `{kind: code, code: 3}`; refused `{code: 3}`, a string, `exit` on a call event; a result with no `exit` still reads | `exit` typed as a free JSON value |
| `schema_exit_needs` | `Needs` rows in the shared wire vectors that Rust and `test_guard_wire_vectors.py` both read | B5 | the wire, where both readers meet | hand-written vectors: `raw`/`manifest` absent means false; non-boolean and unknown keys refused | a default that accepts any value |
| `cc_reader_exit` | `guard_reader.rs::expected_facts`, new `exits` rows in each fixture's `expected.toml` | B1 | integration over the real recorded streams, the seam's faithful copy | rows written from `jq` over the raw lines, never from reader output; classes: exit 0, nonzero (1, 127), a denial, a non-`Bash` tool, a call in a child session, and the probe's new shapes | (a) drop the prefix parse; (b) give code 0 to every non-error result, `Read` included; (c) join on span without session |
| `oc_reader_exit` | the export reader table `opencode_export_reader_covers_trace_errors_cache_cost_and_child_sessions` | B1 | same seam, OpenCode's exports | `jq` over the exports: completed with exit 0, completed with exit N (probe fixture), aborted, permission rejected | (a) read `metadata.exit` only on errors; (b) take a missing `metadata` as 0 |
| `serve_context` | `guard_run_context.rs`: serving and isolation | B2 | integration at `assemble` + `materialise`, the function pair every surface calls | sha256 of the stored files taken in the test from the store paths; file modes read back | (a) restore the silent ignore; (b) a hard link instead of a copy |
| `serve_context` | `a_plug_in_cannot_write_through_to_the_store`, extended to the new parts | B2 | same | the store's trace, raw and manifest bytes before and after a careless plug-in writes | hard link instead of copy |
| `serve_context` | `the_live_context_digest_is_the_independent_vector`, plus new rows from the independent generator (`gen_live_context.py`, python `rfc8785`) | B3 | same | existing rows untouched and still green; new rows for contexts with trace, raw, manifest served | (a) add the trace key even when not served (old rows go red); (b) leave it out when served (new rows go red) |
| `serve_context` | one row in `needs_inputs_refused_when_the_run_cannot_serve_them`'s pattern, or in `guard_silent_acceptance.rs`'s table | B4 | same | the refusal code written by hand from the leaf | serve the context with no trace. If the leaf finds that every sealed run stores a trace by construction, there is nothing to refuse: no guard, and the leaf report says why |
| `helper_context` | `test_guard_context.py` | B6 | the helper's interface, where plug-in code meets it | hand-written wire context and trace lines | (a) a model that accepts unknown fields; (b) `manifest` dropped by `from_wire` |
| `graded_run_sees_trace` | new e2e guard in `guard_cli_e2e/`, on the `parity_e2e.rs` rig | B7 | e2e through `candler run` and `candler evaluate` on the real harnesses and the production gateway: B1's seam is only real there | the fake-model script's own commands (write `answer.txt` and exit 0, `exit 3`, call `parity/stamp`); the kind's verdict depends on the trace (pass only if it finds those `(argv, exit)` pairs and that origin), so a missing or wrong trace changes the outcome itself, not only an echo of it | (a) serve no trace; (b) OpenCode reader without `exit`: the OpenCode row goes red while Claude Code stays green |

The e2e guard is `#[ignore]` like the parity guard, runs in `.github/workflows/container-e2e.yml`,
and joins that workflow's expected-guard list, so a skip fails the job. It asks the same question
as HTN's P0, from inside the suite.

### Trust boundaries and fixtures

- Real: the harness binaries and the gateway (e2e and probe), the store and the copy (all
  `serve_context` guards), the Python helper's own reader.
- Recorded: harness streams and exports, only as written by the real harness on Candler's own rig.
  The one producer of a sealed run is Candler, so no guard builds a trace or a run by hand: runs come
  from the mini-corpus import already used by `guard_run_context.rs` or from a run under the scripted
  agent.
- Scripted: the fake model, whose script is the e2e oracle.

### Deliberately not tested

- `ExitStatus` serialisation: the type is reused, and its process tests already guard it.
- The `signal` variant in traces: no harness reports one and no reader writes one.
- Timeouts, unless the probe finds a harness that reports a code.
- Tampering with a stored trace before assembly: the seal records no trace digest, so Candler makes
  no such promise; the copy check covers only assembly to copy.
- Raw stream and manifest contents: Candler copies bytes. The manifest's producer already has its
  guards.
- The generated helper models field by field: CI's `gen_models.py` drift check covers them.
- The one-copy-routine refactor: the existing `guard_run_context.rs` guards (artifacts, inputs,
  corrupt blob, isolation, write-through) passing unchanged is its guard. It adds no test of its own.
- The golden runs: not served the new parts, so B3's untouched rows already guard their digests.

### Confirms

They are not listed here. Implementers write them freely while building, under the name prefix
`confirm_`. Each one is deleted before its leaf merges, or rewritten to guard standard (with the
`Guards / Oracle / Bite` doc) when it guards a behaviour no guard above covers. The leaf's verifier
checks that `git grep -n confirm_ -- crates python` is empty on the leaf tip.

## Roadmap node tree (sketch; authored with `dirtree-rdm` after approval)

Campaign `__roadmap__/run_context/` (milestone label left to the coordinator, e.g. M6d). Depth is
order; siblings run in parallel.

```
run_context/
├── schema_exit_needs.md          L1  ToolResult.exit, Needs.raw/manifest, RunContext.manifest,
│                                     digest keys, blessed schemas, contract pages
├── probe_shell_shapes.md         L1  probe P-shell: real Claude Code and OpenCode shell shapes
└── readers/
    ├── cc_reader_exit.md         L2  critical path
    ├── oc_reader_exit.md         L2  critical path
    ├── serve_context.md          L2  critical path: one copy routine, assemble/materialise,
    │                                 refusal, ruling note
    ├── helper_context.md         L2  non-critical (RiVault trial candidate)
    └── e2e/
        ├── graded_run_sees_trace.md   L3  e2e guard + CI list
        └── review/
            └── adversarial_review.md  L4  adversarial review of the whole diff against the README
                                          paragraph, the codebase's idioms, and waste
```

The critical path for HTN's pin is L1 schema → L2 readers + serve → L3 e2e guard; the helper leaf
ships in the same release but no gate on that path depends on it.

## Follow-ups (not in this release)

- Claude Code hook denials (`confine_hook`) read as `DeniedBy::Harness`; `user-rejected` too.
- OpenCode `permission.rejected` read as a plain error, not a denial.
- Timeout as a distinct shell end, if a consumer asks (no harness evidence yet).

## Done-criteria

- Every guard above was seen red on the base or under its named bite, then green on the tip. The leaf
  report records which bite was used and the failing message (leaf template convention 2).
- The probe is answered, its answer recorded in the leaf report, its fixtures committed, its code
  deleted.
- No `confirm_` test is left on any leaf tip.
- The e2e guard is listed as PASS in a `container-e2e` run on the integration PR (convention 3). A
  merge that touches the container path waits for it (convention 5).
- The project's checks (AGENTS.md §Checks, helper `pytest`/`ruff`/`gen_models.py`) pass as entry
  conditions; they are not evidence of the feature.
- Acceptance from outside: HTN's P0 on the pinned release.

## Execution model (coordinator-led team-of-subagents)

- The coordinator writes no implementation; every subagent runs Claude Sonnet 5 with the model
  passed explicitly. The `helper_context` leaf may run as a RiVault trial (Reasoning-Medium, then
  Reasoning-Large, then Sonnet), with a mandatory Sonnet review briefed without the run's summary.
- One implementer per leaf, each on its own worktree cut from the integration branch (itself from
  `origin/main`). Briefs carry: colgrep worktree init and absolute `paths` scoped to the worktree;
  the environment (local `uv` for the Python helper, Docker for the rig); the target dir the campaign
  README names, one per leaf, removed at its merge (leaf template convention 6), and the images the
  probe or e2e built; the estimate and its cap as a stop action; reporting spec defects instead of
  working around them; the leaf's guards with their bites, written first.
- A seam review of the whole diff so far follows each merge level (convention 4); the
  `adversarial_review` leaf is the last of them.
- The coordinator never verifies its own dispatches: `spec-verifier` per leaf, a Rust conventions
  review per Rust leaf, and the named `adversarial_review` leaf at L4. Agents are reused through
  SendMessage.
- Per level: integration review by the coordinator; each leaf rebased onto the integration tip, then
  `git merge --no-ff`; re-verify after any rebase or conflict; commit messages through `-F <file>`;
  confirm with `git log`.
- Push nothing without the maintainer's go; the PR description speaks from outside.

## Roadmap authoring (Phase 0, after approval)

Bootstrap `__roadmap__/run_context/` with `dirtree-rdm init`, add every node above with
`dirtree-rdm add`, write each leaf's Goal, Implementation Logic, Deliverables and Success Gates,
run `dirtree-rdm validate`, and commit the roadmap on the integration branch before any dispatch.
