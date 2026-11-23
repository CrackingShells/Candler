# Candler gaps found in HTN Phase 6 run 2 (for the HTN coordinator)

Candler 5199afc7 (v0.13.0), built into a scratch target from `git archive`; the Candler repo was
not edited. The run store is kept at
`<worktree htn-6-run2-b5b5dd>/.claude/skills/htn-creator-workspace/results/phase6-run2-candler/candler-store`.

## 1. A native run cannot supply metadata that a plug-in suite requires (blocks `evaluate`)

- The htn-creator-eval plug-in's suite needs `plan.prompt_domain_name` (and `plan.config`) as run
  metadata.
- `crates/candler-core/src/evaluation/context/assemble.rs:143-158` resolves each needed metadata
  path by exact match in `manifest.comparable().values()`; when it is absent it fails with
  `ContextError::MissingMetadata { detail: "is not a value of the run manifest" }`.
- Declaring the path in the experiment is refused at resolve:
  "candler: refused: `plan.config` is not a path of the experiment surface; it is written in `[fixed]`".
- The prompt's own values are recorded as one map (`prompt.values`), never as separate paths, so
  `plan.prompt_domain_name` cannot be reached through the prompt either.
- Generic need: **a native run must be able to supply metadata that a plug-in suite requires**
  (a declared, recorded metadata surface, or path lookup into recorded maps).
- Consequence here: Tiers A–C were graded host-side with htn-creator-eval@0.4.2, not by
  `candler evaluate`; `candler analyze` was not run.

## 2. `--set replicates=1` is silently ignored

The job was started with `--set replicates=1` to run one pair first; it ran all 5 replicates per arm
(10 runs). No warning or refusal.

## 3. Turn cap enforced inconsistently

`budget.turns = 200`. 0.2.8 r2 ended at 201 turns, ended_by turn cap. 0.2.8 r4 completed at
250 turns (manifest `measured.run.num_turns`), not stopped.

## 4. `[treatment]` demands a source and pinned tag even when treatment is a factor

The experiment had to declare a `[treatment]` default although `request.treatment` is the factor.

## 5. `treatment:` is refused in a plug-in's `isolation.build.context`

`treatment:` is accepted only in list values of a plug-in fragment, so a plug-in cannot ship a
version-general "install the skill's own package" layer; each experiment must declare the
dockerfile + `treatment:` context pair itself.

## 6. A leading `/` in the prompt is refused at run time, not at resolve

Resolve accepted a prompt starting with `/htn-creator`; the run refused it. The experiment now
starts with "Use the htn-creator skill (/htn-creator)…".

## 7. Same `htn_creator_scripts` version string across arms

Both arms' recorded packages show the same scripts-package version, because the skill's pyproject
version was not bumped between 0.2.7 and 0.2.8. The treatment commit distinguishes them; the
package record does not. (Skill-side, not strictly Candler.)

## 8. `CANDLER_STORE` may not be honoured by `resolve`

Observed during setup; `--store` was used throughout. Unverified in code.

## 9. Observations (not defects yet)

- The stored workspace keeps 0-byte mount placeholders for the server files, so the store alone
  cannot re-run a gate against the same servers; the host gate copied them from the plug-in.
- No way found to set `CLAUDE_CODE_MAX_OUTPUT_TOKENS` for the harness: 0.2.7 r4 died with
  "Claude's response exceeded the 32000 output token maximum" while writing domain.py. Unverified
  whether Candler exposes it.
- The local Candler `main` checkout is 114 commits behind `origin/main` and its debug binary
  predates M6a.
