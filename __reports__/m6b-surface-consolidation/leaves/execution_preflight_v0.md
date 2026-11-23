# Execution and preflight verdicts (v0)

Leaf `execution_preflight` of M6b. Judged on the tip (`a0df4d48`), code read through colgrep; all `path:line` cites are relative to the repo root. Reading limits are stated where a claim rests on a module doc or a name and not on the body.

## Live on the tip

| Id | Live | Evidence |
|:--|:--|:--|
| C1 (+H6 estimate) | partly | `plan_runs` has a no-lock `PlanMode::Check` and returns `plans` (new), `sealed` (reused) and `fresh` cells (`crates/candler-core/src/runner/plan.rs:83-92`, `:121-138`); `run` plans this way before it detaches (`crates/candler/src/commands/run.rs:1-14`). No verb prints it; `resolve` stops before planning (`crates/candler/src/commands/resolve.rs:1-12`). |
| C2 (+H1) | the probe verb, no; the ties, yes | I8 canary guard (`crates/candler/tests/it/guard_cli_e2e/i8_e2e.rs:236-529`, `crates/candler-core/tests/it/guard_declared_inputs.rs:772-932`); env image tag is a content key and its harness label is checked against the binary after a build (`crates/candler-core/src/isolation/image.rs:347-424`); harness version checked against driver and request (`crates/candler-core/src/resolve/steps/identity.rs:92-130`); `.mcp.json` rendered by the driver (`crates/candler-harness-claude-code/src/prepare.rs:85-86`); server names resolved once for both drivers (`crates/candler-harness/src/mcp.rs:1-17`); census of tools and servers the agent saw, as evidence per run (`crates/candler-core/src/runner/evidence/census.rs:1-7`, `:58-99`). |
| C4 (+H2) | yes | `resolve` prints `<path>\t<value as JSON>\t<source>` per value (`crates/candler/src/commands/resolve.rs:150-179`); sources are `explicit_argument`, `experiment_file`, `derived {step}`, `imported` and the rest (`__design__/architecture.md:Operation API`); the step that derived a value is its source (treatment `identity.rs:133-198`, image `crates/candler-core/src/resolve/steps/image.rs:98-159`, harness `identity.rs:92-130`). |
| C5 | mostly done | The file is parsed, and `check_factors` and `request_settings` run in the foreground before the Docker steps (`crates/candler/src/commands/resolve.rs:19-28`, `crates/candler/src/commands/preflight.rs:1-32`); only the image step needs Docker (`steps/image.rs:98-159`, `ensure`). Order inferred from module docs and imports, not from the function bodies. |
| G2 | fixed except by design | Stratum gate and imported strata (`crates/candler-core/src/manifest/compare.rs:567-639`, `:414-458`, `:460-515`); eval_infra commit and dirty gate the verdict (`compare.rs:25-31`, `:98-106`). |
| G5 | partly | A missing `.claude/skills/<name>` makes `git archive` fail and surfaces as "the skill tree could not be archived" with no path: at run time as `IsolationError::Plan` (`crates/candler-core/src/runner/exec/mod.rs:210-250`), and in resolve only when ground truth is configured (`crates/candler-core/src/resolve/steps/ground_truth.rs:299-360`). The treatment step resolves tag to commit and never looks for the skill path (`identity.rs:133-198`). Source root is the driver's (`crates/candler-harness-claude-code/src/driver.rs:170-172`). |
| G6 | cause still unnamed | `jobs.session_controls_failed` exists (`docs/contract/codes.toml:979-982`) and its contract text says "No model's behaviour causes this" (`docs/contract/index.md:170`); `isolation.mount_path_unsupported` covers only comma, newline or non-UTF-8 paths (`docs/contract/index.md:149`). No bind-mount check on the store root found. |
| G10 | traps refused, two residuals unverified | Settings are out of the cell (`plan.rs:508-661`); `isolation.image` is the local image id (`steps/image.rs:98-159`), already a Future row (`__design__/architecture.md:1773`). `--set` unknown `request.*` keys and the order of the factor refusal were not read to the body. |
| G14 | designed | One detached supervisor, no daemon, `ps` reads `lost` from the lock (`__design__/architecture.md:355-365`); `run` prints the id and returns (`run.rs:1-14`); `evaluate` waits (`crates/candler/src/commands/evaluate.rs:1-8`). Cap metering, sweep flake and copied `jobs/` dirs not verified. |
| G15 | designed | Facts are R01's (`architecture.md:Isolation`); ground-truth and egress residuals are Future rows (`architecture.md:Future features, Execution and isolation`). |
| G21 (+C15, H8) | no `gc`, no docker lock | The machine-wide flock exists for run slots only (`crates/candler-core/src/jobs/slots.rs:1-25`, `:164-175`) and for job and experiment locks (`crates/candler-core/src/jobs/lock.rs:17-65`); the env and extension tags are content keys (`image.rs:347-424`); orphaned env images wait on the identity row (`architecture.md:1773`). Whether two simultaneous builds of one tag are serialized inside `ensure_layer` was not read. |
| X2 | designed | `run` starts the supervisor and returns; `evaluate` follows because its report is the call's one stdout document (`evaluate.rs:1-8`, `:186-255`); R01 states it (`architecture.md:Evaluate job`). |

## The asks questioned

**C2 and H1, the agent-world probe.** Eliott ruled the container follows from the experiment files; a differing box is a Candler defect for Candler's guards. What ties box to files today: the image tag is keyed by lock, prune, base and context digests (`image.rs:347-424`), a built image whose label disagrees with its binary is refused (`image.rs:347-424`), the harness version is checked at resolve (`identity.rs:92-130`), evaluator-only bytes are guarded end to end (I8 tests above), and the census records what the agent saw (`census.rs:58-99`). What nothing catches: the census records the servers and tools the agent saw, but no code compares them with the cell's declared `mcp.servers`; I found no such comparison, and the Future row for a snapshot-versus-live check on input-delivered servers says it is not automated (`architecture.md:Future features`, "Catalog capture of input-delivered servers"). The M2b-era "no servers" defect would be seen only by reading a census. The remedy is a Candler guard on the existing census, not a verb an agent runs; the probe verb duplicates the preflight and the harness census and invites the inspection pattern the ruling rejects.

**C1 and H6, estimate half.** Cells, new and reused runs are already computed (`plan.rs:121-138`); the cap inputs are job budget values (I13) and the per-run wall bound is the entrypoint `timeout <wall_s + 60 s>` (`architecture.md:462`). So the estimate is a derivation of planning output, not a new verb; the experiment stays an experiment (R01). The "n too wide an interval" warning needs a variance prior that is knowable only from stored outcomes or a user-behaviour baseline (rejected), so it is not judged here; a cost-from-history estimate depends on C8's spend record.

**C4, H2.** `resolve` already is the explain: every value has its source, and the deriving step is part of `derived {step}`. The HTN ask for "which build layers exist" is the image and treatment steps' `derived` entries. The remaining gap is machine-readable output (X1, `cli_contract`).

**C5.** The premise is mostly wrong: shape, unknown fields and factor rules run before any Docker step. A "skip Docker" switch is a second layer of validation (AGENTS.md fail list) and an option that can be derived (R01 Risks).

**G5, G6.** G5 is a missing check in the treatment step, which already reads the pinned commit and has `tree_at`; the failure is a preflight check id (`<step>.<check>`, not a registry code: `resolve.rs:1-12`), so no `codes.toml` row is needed to refuse it earlier. Rows in the blast radius: the run-time `IsolationError::Plan` row in `crates/candler-core/src/isolation/refusal.rs:53-340`, and the row for a failed step; `codes_review` decides whether either changes. G6 is a property of the Docker host's file sharing, not of Candler's files; the real defect is a refusal that hides the cause (`jobs.session_controls_failed`, `codes.toml:979`), which is `codes_review`'s and `cli_contract`'s, and a workaround sentence is not a Candler fact.

**G21, C15, H8.** No lock around Docker is needed by Candler's own runs (slots bound concurrency, I16 forbids a daemon); the `lockf` is HTN's wrapper, and its need is unproven. `gc` of images is blocked by the identity row; containers are swept by the job's sweep and expire by their own `timeout` (`architecture.md:462-477`).

**X2.** Designed, and the reason is stated: one stdout document for `evaluate`; `run` returns an id for later `watch`.

**G2, G10, G14, G15.** Candler's behaviour is as designed and refused loudly; what remains are facts for a consumer, which `docs_and_skills` owns. Unverified residuals are listed in the Live table.

## Verdicts

Triad tables: rows sustainable (S), scalable (Sc), efficient (E); columns development (Dev), codebase (Code), consumption (Use).

### C1: Pre-run estimate (cells, reuse, cap bounds)
- **Verdict:** accept
- **Covers:** H6 (estimate half only; the spend half is `analysis_outputs`' C8)
- **Live on the tip:** partly: `plan.rs:83-92`, `plan.rs:121-138`, `run.rs:1-14` compute it; nothing prints it.
- **The ask questioned:** the premise "a plan operation" is wrong in name (the experiment is never called a plan, R01); the real problem is that planning's output is invisible before spend. The interval warning is not derivable without a variance prior and is left out.
- **Responsibility:** Candler's: it is a derivation of what the core already decides (R01 Operation API, "values that can be known are derived").
- **Blast radius:** `candler-core::runner::plan` (read-only use of `PlanOutcome`), `candler` `commands/resolve.rs` or `run.rs` output; no schemas@ release if printed as text, one if the output type moves to `candler-schemas` (D1); no codes.toml rows; no guard or digest at risk.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | + one source: planning | + no second computation | + the number matches what `run` will start |
  | Sc | 0 | + grows with `PlanOutcome` | + holds for any cell count |
  | E | + one fewer manual multiply | 0 | + no job started to learn it |
- **Compromise:** none, if it prints from `PlanMode::Check` and adds no option; an option would need the R01 Risks justification.
- **Idioms:** follows: one operation API, derived values, never called a plan; avoids a new verb.
- **Reason or owner:** Candler owns the derivation; the cost-from-history figure waits on C8 (seam).
- **Design suggestion:** operation in `candler-core`: `PlanOutcome` summary (`plans.len()`, `sealed.len()`, cells, run count times the per-run `wall_s` and the job caps) returned by a Check-mode plan, surfaced as extra lines of `resolve` output. Files: `crates/candler-core/src/runner/plan.rs` (a summary method), `crates/candler/src/commands/resolve.rs`. Test, role confirm: "does the printed count equal the runs `run` would start, and the reused count the sealed runs?"; no existing test compares printed numbers with `PlanOutcome`; stays sustainable by reading `PlanOutcome` in one fixture-based test that reuses the existing plan fixtures, no new infrastructure. Size: 8 Sonnet calls.
- **Future row:** n/a

### C2: Agent-world probe (a box that differs from its files)
- **Verdict:** defer
- **Covers:** H1
- **Live on the tip:** the probe verb no; ties yes (`i8_e2e.rs:236-529`, `image.rs:347-424`, `identity.rs:92-130`, `census.rs:58-99`).
- **The ask questioned:** the verb is rejected on Eliott's ruling (an agent inspecting the container is a bad pattern). The real problem is the gap he named: nothing compares the cell's declared servers and tools with the census, so a box lacking a declared server is seen only by reading evidence.
- **Responsibility:** Candler's, as a guard and a preflight/census check, not a consumer verb.
- **Blast radius:** `candler-core::runner::evidence::census` and the runner's run-start path; harness drivers (`candler-harness-claude-code`, `candler-harness`); a run whose census disagrees becomes invalid or flagged, which touches the integrity record, so `codes_review` and `analysis_outputs` see it; no schemas@ release if the existing census values suffice; digests at risk: none if the check does not enter `comparable`.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | + defects found by a guard, not by hand | 0 | + boxes true to files |
  | Sc | + | 0 | 0 |
  | E | 0 | 0 | + no 3-run model probe |
- **Compromise:** correctness of the box wins over a leaner runner; but no evidence of a live defect today, so it waits.
- **Idioms:** follows I8-style ground-truth guarding and "a defect is caught by Candler's own guards"; breaks nothing; refuses the verb because it would be a second layer of validation.
- **Reason or owner:** Candler owns the gap; trigger not met: no run on the tip is known to have a census that disagrees with its files.
- **Design suggestion:** n/a (defer)
- **Future row:** | Declared-versus-census check | The run compares the cell's declared MCP servers and tools with the census the harness reports at start, and flags a run whose box lacks one | A box can differ from its files and only a census read shows it | A run, or a model-free test, whose census differs from the cell's declared servers or tools |

### C4: `resolve --explain`
- **Verdict:** done
- **Covers:** H2
- **Live on the tip:** yes: `resolve.rs:150-179`, `steps/image.rs:98-159`, `identity.rs:92-130`, `identity.rs:133-198`.
- **The ask questioned:** the premise that `resolve` does not say which layer set a field is wrong on the tip: every value carries a source, `derived {step}` names the step (I12). What it lacks is machine-readable output, which is X1.
- **Responsibility:** Candler's, already met.
- **Blast radius:** none.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | 0 | + no second flag | 0 |
  | Sc | 0 | + | 0 |
  | E | 0 | + | 0 |
- **Compromise:** none
- **Idioms:** follows: values derived, each records its source (I12); an `--explain` flag would break "every new option must say why it cannot be derived".
- **Reason or owner:** already true; JSON form is X1's (`cli_contract`).
- **Design suggestion:** n/a
- **Future row:** n/a

### C5: Validate without Docker
- **Verdict:** reject
- **Covers:** none
- **Live on the tip:** the need is mostly met: parse, `check_factors` and `request_settings` precede the Docker steps (`resolve.rs:19-28`, `commands/preflight.rs:1-32`; order inferred, bodies unread).
- **The ask questioned:** "needing Docker to learn whether a file is well formed" is wrong for shape, unknown fields and factor rules; only the image step builds. A switch that skips the Docker steps would validate less than `resolve` and make two meanings of "valid".
- **Responsibility:** Candler's, already met by order of steps.
- **Blast radius:** would add an option to `resolve` and branches in the step list (`steps/mod.rs:43-81`).
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | 0 | - second validation path | - two meanings of valid |
  | Sc | 0 | - | 0 |
  | E | 0 | 0 | + only where Docker is absent |
- **Compromise:** the consumer without Docker loses; judged smaller than a second validation layer (AGENTS.md fail list). Revisit only if a consumer with no Docker appears.
- **Idioms:** breaks "a second layer of validation" and "every new option must say why it cannot be derived".
- **Reason or owner:** the foreground checks already fail first; the failures that remain are the Docker ones, and they are the preflight.
- **Design suggestion:** n/a
- **Future row:** n/a

### G2: Comparing arms depends on what is recorded
- **Verdict:** done
- **Covers:** none
- **Live on the tip:** yes: `compare.rs:567-639`, `compare.rs:414-458`, `compare.rs:460-515`, `compare.rs:25-31`.
- **The ask questioned:** the cause (treatment as a factor refused) is fixed. The residual (arms differing in `eval_infra.commit`, `dirty`, `sandbox_contract` refused) is by design: it says two builds of the contract are not one stratum.
- **Responsibility:** Candler's, met; the sentence "two files are two strata" and printing the refusal path belong to `docs_and_skills` and `analysis_outputs`.
- **Blast radius:** none.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | 0 | + | + loud refusal |
  | Sc | 0 | 0 | 0 |
  | E | 0 | 0 | 0 |
- **Compromise:** none
- **Idioms:** follows refusal-as-code; facts for agents generated under drift tests.
- **Reason or owner:** already true on the tip.
- **Design suggestion:** n/a
- **Future row:** n/a

### G5: Missing skill path in the treatment repository
- **Verdict:** accept
- **Covers:** none
- **Live on the tip:** partly: `exec/mod.rs:210-250`, `ground_truth.rs:299-360`, `identity.rs:133-198`.
- **The ask questioned:** "probably live, needs a new code" is half wrong: the check is a preflight check id, which needs no codes.toml row; and with ground truth configured, resolve already fails, though without the path. The real problem: the treatment step pins a commit and never asks whether `<source_root>/<name>` exists in it.
- **Responsibility:** Candler's (I7: the treatment is the tagged subtree).
- **Blast radius:** `candler-core::resolve::steps::identity` (the treatment step), `candler-harness::skills` (`SkillTree`, `crates/candler-harness/src/skills.rs:67-104`, to name the path); codes.toml: the run-time `IsolationError::Plan` row (`isolation/refusal.rs:53-340`) stays, `codes_review` decides whether the message row changes; no schemas@ release; no digest at risk.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | + no 0.1 s failures to diagnose | + one check beside the pin | + path named |
  | Sc | 0 | + driver-supplied root, any harness | + |
  | E | + | 0 | + refused before any job |
- **Compromise:** none
- **Idioms:** follows: preflight is the real check, no tokens spent (R01); a refusal is a code, a side and a path; avoids a defensive check against own code since the repository is user input.
- **Reason or owner:** Candler owns the pin, so owns the existence of what it pins.
- **Design suggestion:** operation: the treatment step asks `SkillTree` for the tree at the pinned commit (`git cat-file -e <commit>:<root>/<name>` or the existing archive) and fails the step with the path named. Files: `crates/candler-core/src/resolve/steps/identity.rs`, `crates/candler-harness/src/skills.rs`. Test, role guard (I7): "does resolve refuse a treatment whose pinned commit lacks the skill subtree, naming the path?"; `b22_skill_tree_is_the_tag` (`guard_claude_code_driver.rs:179-308`) covers a tag that has the tree, not one without it; add one case to its fixture, not a new test file, so it costs one tag in an existing git fixture and scales with no per-harness code. Size: 6 Sonnet calls.
- **Future row:** n/a

### G6: Store root invisible to Docker bind mounts
- **Verdict:** defer
- **Covers:** none
- **Live on the tip:** cause unnamed: `codes.toml:979-982`, `docs/contract/index.md:149`, `index.md:170`.
- **The ask questioned:** two asks hide in one: a resolve-time bind probe, and the cause in the refusal. The cause belongs to `codes_review`/`cli_contract` (what `causes[]` carries). The probe is a Docker-host property check, and one incident does not yet say it recurs.
- **Responsibility:** Candler's for naming the cause of its own refusal; the host's file-sharing is the consumer's environment.
- **Blast radius:** `candler-core::resolve::steps` (a new step), the sidecar and run path, `jobs/orchestrate/session.rs:234-250`; codes.toml: `jobs.session_controls_failed` and `isolation.docker_failed` rows, named, not decided here; no schemas@ release.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | + | - a Docker-host special case | + |
  | Sc | 0 | - per platform | 0 |
  | E | + 25 min saved | - one `docker run` per resolve | + |
- **Compromise:** correctness now loses to a platform special case on one incident.
- **Idioms:** follows "built only when its evidence trigger is met"; a probe is a defensive check unless recurrence is shown.
- **Reason or owner:** no recurrence on record since the one incident.
- **Design suggestion:** n/a (defer)
- **Future row:** | Bind-mount probe on the store root | A resolve step that mounts the store root into a throwaway container and refuses with the Docker error named | A store on a path the Docker host cannot share fails every run late | A second bind-mount failure of a run or control, or the cause of `jobs.session_controls_failed` still unnamed after `codes_review` |

### G10: Fixed-value traps in the experiment file
- **Verdict:** done
- **Covers:** none
- **Live on the tip:** yes for the refusals: `plan.rs:508-661`, `steps/image.rs:98-159`; the image identity row is `architecture.md:1773`.
- **The ask questioned:** the traps are refused or recorded loudly; the image-id trap has a Future row already. Two residuals (`--set` unknown `request.*` keys, the order of the factor refusal) were not read to the body and are not claimed.
- **Responsibility:** Candler's, met; teaching the traps is `docs_and_skills`.
- **Blast radius:** none.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | 0 | 0 | + |
  | Sc | 0 | 0 | 0 |
  | E | 0 | 0 | 0 |
- **Compromise:** none
- **Idioms:** follows refusal-as-code; settings out of the cell.
- **Reason or owner:** behaviour is as designed; the sampling of residuals goes to the build session's first read if Eliott wants it.
- **Design suggestion:** n/a
- **Future row:** n/a

### G14: Detached supervisor, no daemon
- **Verdict:** done
- **Covers:** none
- **Live on the tip:** yes: `architecture.md:355-365`, `run.rs:1-14`, `evaluate.rs:1-8`.
- **The ask questioned:** the behaviours listed (Ctrl-C, SIGTERM leaves the job, `lost` from the lock) are R01's design, not defects. Not verified: live cap metering, the sweep flake, copied `jobs/` directories.
- **Responsibility:** Candler's design (I16); what a consumer must be told ("never `kill`, use `candler kill`") is `docs_and_skills`.
- **Blast radius:** none.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | 0 | + | 0 |
  | Sc | 0 | + | 0 |
  | E | 0 | + | 0 |
- **Compromise:** none
- **Idioms:** follows I16; changing it would break "nothing runs when no job runs".
- **Reason or owner:** as designed.
- **Design suggestion:** n/a
- **Future row:** n/a

### G15: Isolation facts an author trips over
- **Verdict:** done
- **Covers:** none
- **Live on the tip:** yes: `architecture.md` isolation facts; residuals are Future rows (`architecture.md:Future features, Execution and isolation`).
- **The ask questioned:** these are facts, and the open ones (domain fronting, transformed copy, symlink) already carry rows with triggers; nothing is to change in Candler.
- **Responsibility:** Candler's design; the telling is `docs_and_skills`.
- **Blast radius:** none.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | 0 | 0 | 0 |
  | Sc | 0 | 0 | 0 |
  | E | 0 | 0 | 0 |
- **Compromise:** none
- **Idioms:** follows Future-features discipline (built when the trigger is met).
- **Reason or owner:** as designed and tracked.
- **Design suggestion:** n/a
- **Future row:** n/a

### G21: Docker lock and `gc`
- **Verdict:** defer
- **Covers:** H8, C15
- **Live on the tip:** no `gc` and no docker lock; slots and job locks exist (`slots.rs:1-25`, `jobs/lock.rs:17-65`); content-keyed tags (`image.rs:347-424`).
- **The ask questioned:** "the run owns the docker lock" assumes a race Candler's own runs do not have (slots bound concurrency; no daemon, I16); whether two builds of one tag race inside `ensure_layer` was not read. `gc` of env images is blocked by the existing identity row (`architecture.md:1773`); containers expire by their own `timeout` and the sweep (`architecture.md:462-477`).
- **Responsibility:** Candler's, for the store's own images and containers; the wrapper the caller uses is the caller's.
- **Blast radius:** `candler-core::isolation` (image labels, the lock), `candler-core::jobs`, a new verb in `candler`; schemas@: none; codes.toml: rows for refusal of `gc` on a running job (new, named only); I16 limits any holder to the call's lifetime.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | + | - a verb with a deletion path | + |
  | Sc | + | - tag identity not settled | + |
  | E | + disk freed | 0 | + |
- **Compromise:** disk reclaim waits on the identity decision, so a gc written now would delete by a key still to change.
- **Idioms:** follows Future-features discipline and I16; honours "never deleted" for runs by touching images and containers only.
- **Reason or owner:** trigger not met.
- **Design suggestion:** n/a (defer)
- **Future row:** | Docker lock and `candler gc` | A lock the call holds around image builds, and a verb that lists, with a dry run, then removes a store's own images, containers and networks | Concurrent builds of one tag, and env images that pile up per store | Two concurrent resolves or runs that fail or build twice on one tag, or the identity row above decided |

### X2: One way to wait
- **Verdict:** reject
- **Covers:** none
- **Live on the tip:** yes, as designed: `run.rs:1-14`, `evaluate.rs:1-8`, `evaluate.rs:186-255`.
- **The ask questioned:** the difference is not accidental: `evaluate` waits so its report is the one stdout document, and `run` returns an id because a run job is long and `watch` follows it (`architecture.md:Evaluate job`). A uniform wait would block `run` for hours or detach `evaluate` from its report.
- **Responsibility:** Candler's, decided in R01.
- **Blast radius:** `candler` `commands/run.rs`, `commands/evaluate.rs`, and consumers' call patterns.
- **Triad:**

  | | Dev | Code | Use |
  |:--|:--|:--|:--|
  | S | 0 | - two paths to keep equal | - a flag to learn |
  | Sc | 0 | 0 | 0 |
  | E | 0 | 0 | - blocking `run` |
- **Compromise:** none
- **Idioms:** breaks "every new option must say why it cannot be derived": the wait is derived from what the verb returns.
- **Reason or owner:** designed; the consumer-facing rule ("`run` returns, then `watch`") is a docs fact.
- **Design suggestion:** n/a
- **Future row:** n/a

## Seams

- G5 and G6 on `codes_review`: the run-time `IsolationError::Plan` row, `jobs.session_controls_failed` and its `causes[]`; named here, not decided.
- C1 on `analysis_outputs`: C8's spend record is the only source for a cost estimate from history; H6's spend half is theirs.
- C4 on `cli_contract`: the `resolve` output has no JSON form (X1, D1); C1's printed summary joins that contract.
- C2 on `analysis_outputs` and `codes_review`: a run flagged by a census mismatch changes the integrity record and needs a code.
- G2, G10, G14, G15, X2 on `docs_and_skills`: the sentences a consumer must be told (two files are two strata, never `kill`, `run` returns then `watch`) are docs and skills; Candler changes nothing.
- G2 on `analysis_outputs`: whether `analyze` prints the refusal path.
- G21 on the `isolation.image` identity row: `gc` waits for that decision.
