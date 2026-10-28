# Legacy golden fixtures

Raw rows and grading entries from the golden corpus, one per shape class. They are inputs to the
legacy-import mapping tests; nothing here is converted.

## Provenance

| | |
|---|---|
| Repository | `agenticai4hpc26-artifact` (`/Users/hacker/Documents/src/External/PCfVW/agenticai4hpc26-artifact`) |
| Tag | `candler-m2a-golden` |
| Commit | `7efa1b7794a52121b4b965e61373234aa94d3279` |
| Campaign path | `campaign/20260806-cont-train/` |
| Rows | `expectations.jsonl`, blob `5e829fbe3850fc5f2c66274bdc31bc7a911556f1`, 10,320 rows |
| Run provenance | `PROVENANCE.json`, blob `8b80c02a074393e10429251093a187fb0f7a154d` |
| Grading entries | `<cell>/eval-1-tier-a-domain-structure/<config>/<run>/<eval>`, read from the same tag |
| Regraded by | `htn-creator-eval@0.4.2` (mcp-python-ingestion `28be2c158fbc6ee786a789242c2d8fd47bfde9a8`) |
| Licence of the data | CC-BY-4.0 (`campaign/LICENSE` in the artifact repository) |

## Files

| File | Content |
|---|---|
| `select_golden.py` | Computes each row's shape class and writes the three data files |
| `golden_rows.jsonl` | The first row of each class in corpus order, byte for byte |
| `golden_grading.jsonl` | The row's entry from its grading file, `{cell, run, eval, id, entry}`, re-serialised compactly with the original key order and escapes |
| `golden_floats.json` | Every distinct float `value` in the corpus, sorted; each number is written with the same text as the corpus |

## Shape classes

A class is `(status, value kind, weight class, cause kind)`.

- Value kind: `none`, `int`, `float`, `text`.
- Weight class: `default` is the status default (pass 1/1, fail 0/1, blocked 0/1, not_measured 0/0);
  `0/0` is a zero weight on a status whose default differs; `other` is any other `score`/`max_score`.
- Cause kind: `none` for pass and fail; `artifact` for a blocked row with `blocked_by`;
  `marker:<code>` for a not_measured row whose evidence carries `TIER_D_REPLAY: ... reason=<code>`;
  `zero_mappings` for a not_measured row whose evidence says the mapping declares zero `mappings`
  entries; `unrecorded` when the grading entry gives no readable cause.

The corpus has 14 classes:

| Rows | Class |
|---:|---|
| 5 | blocked / float / default / artifact |
| 797 | blocked / none / default / artifact |
| 1 | fail / float / default / none |
| 22 | fail / float / other / none |
| 333 | fail / none / default / none |
| 129 | fail / none / other / none |
| 724 | not_measured / none / default / marker:no_replay_target |
| 18 | not_measured / none / default / zero_mappings |
| 241 | pass / float / default / none |
| 211 | pass / float / other / none |
| 1,920 | pass / int / default / none |
| 209 | pass / none / 0/0 / none |
| 4,262 | pass / none / default / none |
| 1,448 | pass / none / other / none |

No row has a text value, a `not_applicable` status or an `unrecorded` cause.

## Command

From the repository root:

```
python3 -I crates/candler-schemas/tests/fixtures/legacy/select_golden.py
```

`--repo PATH` names the artifact repository (default: the path above); `--out DIR` writes the data
files elsewhere. The output is deterministic, so a re-run leaves `git status` clean.
