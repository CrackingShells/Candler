# L1: the coordinator's rulings on the leaf reports (v0)

Five L1 leaves reported spec defects or gaps: `selection_rates`, `registry` (second report), `reference_oracle`, `protocol_client` and `helper_runtime`. This report records each ruling and the leaf or report text it changed. Each leaf's own report keeps its commits, gates and bites.

## Cell naming (selection_rates, reference_oracle)
**Defect.** The rates gate is a byte `diff` between `candler show --rates` and `$ORACLE/rates_by_cell.toml`, and the oracle keyed cells by directory name. The store does not know that name: a `RunManifest` holds the cell digest in `run`, and its `import.sources` give cell-relative paths only.

**Ruling.** Cells are keyed by the 64 bare hex digits of the cell digest everywhere: `CellKey`, `show --rates`, `rates_by_cell.toml` and `na_pairs.txt`. This is also how the run keys of `equivalence.json` already print. The oracle computes that digest from the raw corpus by M1's rule, as it already did for `equivalence.json`. Its `--check` pins the two M1 vector cells, and the verifier recomputed three more.

A new `cells.tsv` maps each hex to its directory, for readers and for OR1. OR1 still derives by directory and relabels through `cells.tsv`, which must name each of the 12 directories once. A wrong hex cannot pass: GP1's byte comparison and GP2's `equivalence_missing` both fail loudly on it.

The byte comparison therefore stays as written, and golden_parity needs no join.

| Changed | Where |
|---|---|
| `[cell."<cell hex>"]`, `na_pairs.txt` keyed by hex, `cells.tsv` | `reference_oracle.md` Steps 1–3, R03 §Generic diff |
| `rates(selection, &NameVersion)`, `CellKey` prints as bare hex | `selection_rates.md` Step 3 |

## Oracle size (reference_oracle)
**Defect.** The 30 KB limit cannot hold the two 209-row files: each row carries a 64-hex run key or a cell key, and the oracle directory came to 65,218 bytes.

**Ruling.** The limit is 72 KB (73,728 bytes). The whole fixture budget is still met: 10 + 12 + 20 + 10 + 72 + 5 = 129 KB, against 150 KB. R04's fixture table follows.

**Formats accepted as the oracle chose them:**
- one blank line between tables;
- `total` at top level of `counts.toml`;
- one compact line per `equivalence.json` row;
- a generated README.

## selection_rates
- `&NameVersion` in place of `EvaluatorId` is accepted, as are the added public items.
- Interpretations (a)–(i) are accepted:
  - **(a)** Uncompared rows are listed in both directions.
  - **(b)** A row's class is its first differing field.
  - **(c)** An error replaced by a different error is flagged.
  - **(d)** The later record is current.
  - **(e)** The latest record wins silently.
  - **(f)–(h)** `-0.0` and `0.0` differ.
  - **(i)** `generator` is optional.
- **Fix round.** Record a bite for each of the two DF2 guards that has none: the refusal, and the uncompared list.

## registry (second report)
- **F1, F4, F6, F7: accepted.**
  - F1: `for_plugin` and `with_path`.
  - F6 and F7: a sync deadline of 30 minutes, with records under `<env root>/<digest>/records`.
  - `register_evaluation_steps` has no direct test. `surfaces`' CLI2 covers it end to end.
- **F2: accepted.** The digest is `sha256(lock ‖ <project>/pyproject.toml)`, and `project` is the uv project directory. R03 §Environments now says so.
- **F3.** `RelPath` refuses `.`. The HTN plug-in's uv project is `$PLUGIN` itself, so its manifest sits one level up, at `.claude/skills/htn-creator-workspace/candler-plugin.toml`, with `project = "candler-plugin"`. `htn_plugin.md` Step 1 says so. It also no longer puts `KindSpec`s in the manifest, which A3 had made wrong. The coordinator widens that leaf's OWNED before dispatch.
- **F5, fix round.** No step checked the described version against the suite entry's `name@range`. `check_suite` now refuses a described version outside the range, with a REG1 row and its bite.

## protocol_client
- **F1: fixed.** The `$` in the OWNED regex anchored `fixtures/client/`, so no stub could match. The regex now anchors each file and leaves the folder open.
- **F2: fixed in the text, as the code already does.** The leaf and R03 said stdin carries one request and then EOF. R01 §Lifetime keeps stdin open so that `cancel` can follow. R01 holds: the client closes stdin after the response or after `cancel`, and the helper exits on EOF. `helper_runtime` already reads `cancel` while an item runs.
- **F3: accepted.** `ItemOutcome::Unreported { expectation_id, reason, evidence }`. The engine writes the `not_measured` record from `reason.code()` and that evidence (`engine.md` Step 2).
- **F4–F8: accepted as chosen.**
  - F4: the types, a 30 s `describe` deadline, and graces of 2 s for cancel and 5 s for stop.
  - F5: the `StopSignal as Termination` alias. Its weakness: PC2 trips if a later edit names `StopSignal` plainly, which costs a false failure, never a missed signal.
  - F6: steps 1 and 2 in one commit.
  - F7: how dropped lines are read, and an unsolicited `-32800` as `instrument_error`.
  - F8: `stop_fault`.

## helper_runtime
- **1: accepted.** The helper cannot know `role`, `suite_entry_digest` or `context_digest`, so it sends `descriptive` and all-zero digests. The engine sets all three from its own plan and never keeps the wire values (`engine.md` Step 2). ENG1 and ENG2 see the result.
- **2: accepted.** The isolation mode goes in a `log` notification, `isolation: fork|inline`, sent before the `describe` response. `DescribeResult` is closed.
- **3: accepted.** An editable install reports the sha256 of its `direct_url.json` text with `dirty=true`. The schema forbids `dirty` on a `git:` digest, and PEP 610 never pairs `vcs_info` with `dir_info`.
- **4: accepted.** Kinds receive every prior record of the request in order, and pick their dependencies by expectation id. `Item` carries no `depends_on`.
- **5: fixed.** The OWNED gate used a look-ahead, which POSIX ERE lacks, so `grep -vE` failed and `test -z` passed vacuously. It is now a plain prefix regex plus `grep -F _models`.
- **6 and 7: accepted.**
  - 6: the `Plugin` dataclass, the `python -m candler_evaluator <module>` entry, and the wider `RunContext`.
  - 7: on cancel, the item's child is abandoned to its own alarm. The verifier's M3, that the child is reaped only at helper exit, is accepted: the client closes stdin after `cancel`, so the helper exits.
