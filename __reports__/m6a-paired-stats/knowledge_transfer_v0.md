# M6a paired statistics: knowledge transfer (v0)

Date: 2026-11-18. Author: the M6a milestone coordinator. Sources:
- the campaign `__roadmap__/m6a-paired-stats/` (its Amendment Log and Progress table);
- the level handovers L1 to L3;
- the leaf reports in `leaves/`;
- the session transcripts, priced with `__reports__/agent-token-costs/scripts/spend.py` at R05 rates, each API message counted once.

It follows the M5b report (`__reports__/m5b-egress-proxy/coordination_knowledge_transfer_v0.md`).

## Executive summary

- **What shipped.** `analyze` now picks each test from the experiment's design. Every declared factor's level pair is compared, blocked by the other declared factors, with:
  - Fisher mid-p for one block;
  - CMH with an exact fallback;
  - a stratified permutation test with a BCa interval for weighted outcomes.

  Holm runs per comparison per role. `request.treatment` is a declared factor like `model.id`, and the importer declares it. The HTN golden design (3 models × 4 skill versions × 20 runs) is one Candler experiment file. Each arm reports `instrument_error`. No golden or parity digest moved.
- **What it cost.** About **$84** in all:
  - planning: $11.7;
  - build, four levels: $54.3, against a $105 cap;
  - milestone coordination: $17.7.

  It is the first milestone under its build budget. M5b was 2.1× over.
- **What went wrong.** The plan missed one consumer of the changed concept: the importer never declared the treatment. Amendment A9 also left a gate guarding the old design. Both surfaced only at L3, and L3 was reshaped with Eliott's approval (A8, A10).

## Gates

| Gate | What it proves | Result |
|:--|:--|:--|
| G1 | every test equals an offline second engine (scipy 1.17.0, statsmodels 0.15.0) | pass |
| G2 | the golden design plans 12 cells. `analyze` gives 6 version pairs × 3 blocks and 3 model pairs × 4 blocks, equal to a second engine within 1e-9 | pass |
| G12 | an import declares the same factors as its native rerun (mini corpus, and the 240 golden runs) | pass |
| G3 | every validity source is cited in `__design__/architecture.md` §Analysis statistics | pass |
| G4 to G7 | per-arm `instrument_error`; treatment plan and wiring; I7 refusals; one-factor output | pass |
| G8 | under H0 the false-positive rate holds α for binary outcomes, one block and tie-free weights. With tied weights the rate is at most α (A15) | pass |
| G9, G10 | DIGEST, GOLDEN, STD (810 tests), LINUX, SUBJ; no schemas commit | pass |

The candler-v1 leaf gates, as reworded by A10, are each covered by a row above. PR CrackingShells/Candler#52 landed them on `main` as 8e09fcb4, with every CI check green.

## Where the money went

| Part | Cost | Plan |
|:--|--:|--:|
| Planning session (plan mode) | $11.67 | measured separately |
| L0: 7 leaves, the engines and the treatment setting | $16.70 | |
| L1: resolve, blocking | $6.60 | |
| L2: wiring, `analyze` output | $16.43 | |
| L3: golden design, import, known truth, design record | $14.57 | ≈ $20 (reshaped) |
| **Build, L0 to L3** | **$54.30** | **$105 cap, $84 stop** |
| Milestone coordinator: reviews, rulings, merges, close | $15.85 | $8 (close) |
| An aborted planning subagent (see lessons) | $1.89 | — |

The milestone coordinator averaged 188k tokens of context per call over 192 calls. It is the most expensive call in the system, and the next change should target it.

## Amendments

Fifteen amendments: A1 to A8 and A11 to A15 in the campaign log, A9 and A10 in the candler-v1 log. They fall into five classes:

| Class | Amendments | Pattern |
|:--|:--|:--|
| Plan facts wrong about the code | A1, A2, A5, A13 | the plan assumed a code path (an image step in `plan_runs`, setting paths in `fresh_cells`, a reachable `level_of` branch) that did not exist |
| Convention slips | A3, A4 | scope `plan` is not in `cog.toml`; a merge pre-condition tested a title |
| Design rulings | A6, A7, A11, A12 | a one-factor store keeps today's output; the table form of a treatment level; the importer's one-to-one rule, kept to all-imported strata |
| Design sweep missed | A8, A9 (gates), A10 | the importer was not planned as a reader of the treatment factor, and A9 did not reword the gates it made false |
| Statistics found by known truth | A14, A15 | with tied weights the permutation test is conservative (0.024 at 5 weight values, 0.056 on a grid of 1000) |

## Lessons

### Worked

- **Fresh Opus coordinator per level, Sonnet implementers, explicit call caps.** The build came in at 52 % of its cap. Call caps written as stop actions held, with one exception: the A14 agent made 32 calls against its cap of 25.
- **Second engines as evidence.** Every statistic is checked against an offline Python engine run with `--check`. The bites (pooled instead of blocked, continuity correction on, blocks taken from all factors) all turned the rows red.
- **Known-truth runs found a real property.** The tied-weights conservativeness came from G8, not from a review. The fix was to state the property, not to relax the gate. The implementer's relaxation was reverted.
- **Golden runs as a design, not numbers.** After the reshape, L3 proved the experiment's *shape* plans and analyses, and the import declares the same factors. It pinned no p-value from the 240 old outcomes (see memory `golden-runs-prove-the-design`).

### Did not work

- **The consumer sweep.** A7 made the treatment a factor, but nobody listed who reads factors. The importer kept declaring three aliases of one variable, and the version comparison came out empty. Rule now: an amendment lists every reader of the changed path and rewords every gate it makes false (memory `amendment-sweeps-its-gates`).
- **A planning subagent instead of a plan-mode session.** It was started, then stopped by Eliott ($1.89 lost); see memory `milestone-planning-is-a-plan-mode-session`.
- **Local branch refs shared across worktrees.** The local `feat/m6a-paired-stats` ref is checked out in the finished L0 worktree, so the coordinator cannot move it. L3 worked from a scratch base branch and pushed with `HEAD:feat/m6a-paired-stats`.
- **A red `main` that nobody owned.** From 9560cd3d on, 20 guards failed on `main`'s CI. The commit was roadmap-only, so nobody looked. #52's CI then failed the same 20 plus one M6a guard on the same cause.
  - Cause: rust-cache deletes every file outside the build profiles but keeps the directories. So the shared fixture `target/tmp/job_skill_repo` came back as an empty skeleton. The fixture tested `.git` for existence, and git climbed up to the checkout's own repository.
  - Fix: CrackingShells/Candler#53 tests `.git/HEAD`, prunes only empty directories, and sets `GIT_CEILING_DIRECTORIES`.
  - The level gates had missed it because they build with a target dir outside any repository.
  - Rule: a fixture that persists across processes tests that it is valid, not that it exists. And CI on `main` is read after every merge, roadmap-only ones included.
- **Tooling gaps.**
  - `dirtree-rdm` has no command for Amendment Log or Progress rows, so they were written by hand and checked with `validate`.
  - The `:!__roadmap__` pathspec fails in this git ("Unimplemented pathspec magic '_'"). Use `:(exclude)`.

### Change next cycle

1. Put a consumer table in the plan: every path or type a decision changes, with each reader and the leaf that updates it.
2. At each amendment, grep the leaves' gates for the old design and reword them in the same commit.
3. Budget the milestone coordinator as its own row. Its context grows over the whole milestone, so it hands its reads to level sessions and keeps only rulings and merges.

## Open follow-ups (for Eliott)

None of these blocks M6a. The first three were raised during L3. The last two come from Eliott's review of `analyze` on the real golden corpus.

| # | Finding | Effect | Proposal |
|:--|:--|:--|:--|
| F1 | `check_factors` refuses table-level factors, though the planner accepts them since M2c | an experiment with a table factor other than the treatment plans but cannot be analysed | let `level_of` read table levels for any declared factor |
| F2 | A and B follow the order levels were recorded in | the sign of B − A depends on recording order; `v0.2.3` sorts after `0.2.6` | orient by the experiment file's declared level order when it is given |
| F3 | tied weights make the weighted test conservative, at about half its size | real HTN weights are small fractions, so true effects are missed more often | a permutation mid-p (ties count half), matching the binary mid-p. This changes A9 |
| F4 | `reading` ("effect" / "inconclusive") is judged before Holm | a reading can say "effect" where Holm does not reject | judge the reading on the Holm-adjusted decision |
| F5 | `analyze` writes JSON only | the report is hard to read by hand | a text rendering of the comparisons |

Low severity, recorded in the L3 handover:
- `Report::build` and `compare_pairs` are public and skip `check_factors`.
- The importer keys levels with `Value::to_string`, while the verdict uses JCS.
- A treatment name may contain `@`.
- The weighted constant-p bite was not run.
