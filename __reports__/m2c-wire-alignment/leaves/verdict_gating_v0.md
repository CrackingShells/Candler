# verdict_gating: leaf report (v0)

Verdict: done. The stratum verdict gates on the build's identity and describes the rest. Base 2aa7afd, branch `m2c/verdict_gating`.

## Commits

| sha | subject |
|:----|:--------|
| 0bb8ce7 | fix(manifest): count evaluation_inputs among the nuisance paths |
| 716f079 | feat(manifest): gate a stratum on the build's commit, not its tag |
| this commit | reports(manifest): record the verdict_gating leaf |

## Gates

| gate | result | last line |
|:-----|:-------|:----------|
| Known-red `nuisance_set_is_the_hand_written_one` on base | red, then green | before: `left`/`right` differ by `evaluation_inputs`; after: `2 tests run: 2 passed` |
| V1 `binary(guard_comparability)` | PASS | `2 tests run: 2 passed, 0 skipped` |
| compare.rs names no planner or skill version | PASS | `git grep` exit 1 |
| STD, `-p candler-core`: fmt, clippy `-D warnings`, nextest, doc | PASS | `146 tests run: 146 passed, 12 skipped` |
| LINUX | PASS | `Finished dev profile` |
| SUBJ | PASS | `No errored commits`, no subject over 72 |
| OWNED | PASS | three paths, all OWNED |

DRIFT, G5a and GOLDEN do not apply: no schemas file or vector was touched. SPAWN was not run: no `src` file outside `compare.rs` changed and no test spawns.

One flake: in the first full `-p candler-core` run, `guard_container_record::stopped_run_has_its_agent_recorded_before_the_hook_sweeps` failed (timing, 16 s). It passed alone, and the whole set passed 146/146 on a rerun with `--no-fail-fast`. Not related to this leaf.

## Guards

**`stratum_verdict_table` (V1)**
- Guards: I7 and D8: arms compare only when they agree on the build identity.
  - `eval_infra.commit` recorded and equal; `eval_infra.dirty` false everywhere; `sandbox_contract` equal.
  - `tag`, `commits_ahead` and `packages.*` gate nothing.
  - An unrecorded commit or dirty flag is refused and named.
- Oracle: rows of `verdicts.json`, expected literals written by hand from D8.
- New rows: C53 (commit unrecorded in every arm), C54 (unrecorded in one), C55 (tag differs), C56 (`commits_ahead` differs), C57 (`packages.*` differ), C58 (commit and dirty unrecorded, both named, sorted), C59 (tag and commit differ, only the commit named).
- Reworded: C50 (tag recorded in one arm only is now comparable) and C51 (source text).
- Red before the code: C50, C53, C55, C56, C58, C59. C54 and C57 passed already, as controls.
- Existing rows C45 to C49 (dirty unrecorded, true, one dirty arm; differing commit; differing `sandbox_contract`) are unchanged and stay green.

**Bites**, each injected, seen red and restored (`git checkout`/copy of the good file):

| bite | red rows | message seen |
|:-----|:---------|:-------------|
| Every `provenance.eval_infra.*` gates again | C50, C55, C56, C59 | `C55 ...: expected "comparable", got Refused([... "provenance.eval_infra.tag"])` |
| Unrecorded commit counted as equal (`leaf_key(None)` equality only) | C53, C58 | `C53 ...: expected {"refused":["provenance.eval_infra.commit"]}, got Comparable` |

The `packages.*` bite (gate every `provenance.*`) was not run; C57 covers it by construction.

**`nuisance_set_is_the_hand_written_one`** (existing oracle, unchanged): red at base, green after 577646c.

## Probe answers

None in this leaf.

## Fix round 1

Three fixups of the feat commit, autosquashed onto `feat/m2c-wire-alignment` (ea4905e).

| item | change | new rows | bite and message seen |
|:-----|:-------|:---------|:----------------------|
| 1 sort unguarded | none (rows only) | C60 (`census.init_tools` refused with an unrecorded commit, sorted), C61 (commit differs and is unrecorded in a third arm, beside a dirty arm: commit then dirty, once) | `refused.sort()` removed: C60 `got Refused([... commit, ... census.init_tools])`, C61 shows the commit twice |
| 2 unrecorded never equal | `sandbox_contract` unrecorded in every arm is refused, like commit | C62 | contract-unrecorded counted equal: C62 `expected {"refused":["provenance.sandbox_contract"]}, got Comparable` |
| 3 commit is a non-empty string | an empty or non-string commit counts as unrecorded and names the commit path; the 40-hex format is the recorder's | C63 (empty string), C64 (number 123), identical across arms | empty commit accepted: C63 and C64 `got Comparable` |

The old verifier ids X07, X22 and X23 are not rows of the table; C62 to C64 are their guards. The doc of `provenance_refused` states the rule once: a gated path never counts as equal when unrecorded.

## Findings

- **Split in one place (D8).** `gates_verdict` and the three path constants carry the gated and descriptive split with its reason. The refusal causes are in the doc of `provenance_refused`. A veto of D8 means editing those two functions and the rows C45 to C59.
- **Deviation, none from the leaf's text.** The nuisance fix is its own first commit, as the dispatch said.
- **Closed in fix round 1.** `sandbox_contract` unrecorded now refuses like commit and dirty (the binary always records it).
- **Observation.** `from_manifest` keeps only the three gated paths, so a gated path outside the three cannot reach the verdict whatever `provenance_refused` does.
- **Sibling dependence.** None: fixtures are hand-written manifests.
