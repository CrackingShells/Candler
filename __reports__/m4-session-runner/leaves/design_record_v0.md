# design_record: R01 states what M4 built

Type: findings. Branch `m4/design_record`, base 4ce963f. Docs only: `__design__/architecture.md` and `README.md`.

## Commits

| sha | subject |
|:--|:--|
| eca195b | docs(design): record what a driver supplies and what opencode does |
| c4eba91 | docs(design): record session mode and the imported-strata rule |
| 7bbb080 | docs(readme): name the opencode driver crate |

Net `git diff --shortstat 4ce963f..HEAD`: 2 files changed, 245 insertions(+), 49 deletions(-) (plus this report).

## Gates

| Gate | Result |
|:--|:--|
| G7 (OpenCode section keywords, no "not yet confirmed") | PASS, prints nothing |
| G4 (`import.rules` and "imported" in §Run manifest) | PASS |
| `runner.mode`, `runner.idle_s`, `session.end`, `session.control` present; `runner.mode` a cell value in the configuration table | PASS |
| Workspace section names `candler-harness-opencode` | PASS |
| "run in every session experiment" gone | PASS |
| README names the new crate | PASS |
| SUBJ (`cog check 4ce963f..HEAD`, subjects at most 72) | PASS |
| Spec-verifier | the coordinator's step |

## Controls paragraph (from D4 and the coordinator's settled design, not the leaf's "resolve step")

Written in §Session runner as "Controls validate the instrument, never the model": C1 and C2 run the real harness in the cell's base image through the production gateway sidecar against the embedded scripted fake model (`candler-fakemodel:src-<digest>`); the check runs once when a job opens, before any run, if any fresh cell is a session cell (a job with none never runs one); control runs go to a scratch store, never `runs/`; the verdict is a pure function over measured `session.*` values (turns, `session.end`, the close, the close between `idle_s` and `idle_s + 1 s` after the turn end), proving the close rule at the control's own `idle_s` only; the record is keyed by the base image's content digest and a control digest, written once (I11), a failing verdict not written; failure refuses the job's session cells with an environment-side refusal naming harness version and control (no code name or file path in R01); every session run records `session.control` (path and digest) as a resolved setting outside the cell digest; a model's behaviour never refuses. This is not in the code at the branch point; adjust if the build differs.

## Findings

1. **Contradiction outside my sections (code doc).** `CollectSpec::Exec` in `crates/candler-harness/src/spec.rs` says the command inherits the run environment "including the variable that holds the gateway token". The core does not: `collect_args` (`candler-core/src/isolation/container/exec.rs`) passes `plan.env` (state variables), and the gateway names live in `plan.gateway`. R01 says "no gateway name". The doc should be corrected.
2. **R01 sentences I rewrote because the code contradicted them**: the end rule ("last Stop payload lists no task"; the code also needs the state `known`, and wake-ups and crons make it unknown after a Stop-less turn), "controls run in every session experiment", "OpenCode: HTTP pre-stop, overlay isolates HOME" (the stop is an in-container `Exec`, and state paths come from `state_paths()`), "Not yet confirmed" facts, `capabilities` without `session`, `classify_exit` as the core's call (it is `classify_end`), the ingest row ("Claude Code trace directories").
3. **Report vs code.** `session_runner` round 0 names `HOOK_SETTLE` (250 ms); the code has none, so R01 states no settle. `session.idle_s` is a measured path (that report's F), included.
4. **Stale items elsewhere, not changed.** `crates/candler-harness/src/spec.rs`: `PendingKind::Monitor` is never produced (Stop and the task list do not tell a monitor from a job); R01 lists "monitor" among pending kinds in prose only as what the close rule waits for. `runner/evidence/mod.rs` has a stale module doc line about event keys (harness_out_of_core finding 8).
5. **Unexercised, stated in R01.** Real-model interrupt and mid-tool SIGTERM, a code-mode `execute` call, `--thinking`, resume. Session mode on Claude Code was recorded only against the fake model; Linux hook-log latency is unmeasured (session_runner L).
6. **Version.** The leaf and D11 say "latest 2.1.x"; the driver is pinned to exactly 2.2.7, as R01 now says.
7. No R01 statement on `refactor(runner)!` `FrameMode` removal beyond "the runner frames the raw stream by the reader's `framing()`"; no `FrameMode` text existed in R01.

## Verification
The spec-verifier ran against the merged code (session controls included). Every `[cmd]` gate passed. It found four R01 sentences the code contradicted, which the coordinator corrected:
- the close rule never closes with anything pending, background tasks included;
- only the items added for the second harness have default bodies;
- a failed control refuses the whole job, with `jobs.session_controls_failed`;
- the record key includes Candler's version, and `session.control` is a measured value.

It also corrected the reader's `FrameIn` fields, the effort suffix of OpenCode's model argument, the `Edit`/`Write` pair rule, and which session the export is collected for.
