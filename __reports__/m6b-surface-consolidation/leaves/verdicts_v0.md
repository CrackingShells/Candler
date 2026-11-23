# Verdicts leaf (v0)

Leaf `verdicts` of M6b (L1): folds the six L0 reports into `__reports__/m6b-surface-consolidation/01-findings_verdicts_v0.md`. Code was read through colgrep for the seams only; see the report's `## Seams` for the outcomes.

## Verifier

A fresh Sonnet verifier (model passed explicitly) attacked the report with the leaf's Verifier brief. Round 1 returned 12 findings; every one was accepted and fixed in the report (none disputed). Each disposition below names where the fix lives. Claims the verifier made from code (F2, F5, F9, and the path claim of F1) were accepted on its evidence and are cited as its findings in the report; F3 and F4 were re-checked by the implementer (`architecture.md:1806`, `crates/candler-schemas/src/trace.rs:1216-1309`) and are true.

| Finding | Severity | Disposition |
|:--|:--|:--|
| F1: G8's "path and side stay" is unproven and the scope is understated (39 shadowed rows, fix covers 14; `read_value` validates for every JSON read) | high | Fixed: G8 verdict row, Blast radius release 1 and the Codes review now say `path` stays only if the post-parse validator supplies a pointer, that 14 of 39 are covered and 25 `format.*` and `canonical.*` rows stay behind `document_invalid`, and that the `try_from` invariant loosens; the size may be exceeded. |
| F2: false claim that the `EXIT_CODES` constant is readable by the page generator (`crates/candler` has no `lib.rs`) | medium | Fixed in seam 3: stated as open, needs a path include, about 2 calls more. |
| F3: X8's "add the missing paired-design Future row" is false; the row exists at `architecture.md:1806` | medium | Fixed in seam 6: add nothing; the leaf's error is named. |
| F4: the trace is a schema type (`TraceFile`), so no new record and no third-release addition | low-medium | Fixed in seam 4, D1's table and the release table. |
| F5: `resolve` needs Docker (`preflight.rs:317-322`); X5's quick start and C1 depend on it | medium | Fixed: seam 5, the C1 and C5 rows, D2's quick start, Q5's repo facts. |
| F6: C3 and X7's one table is a new item and cannot replace `PROJECT_TABLES` and `USER_TABLES` | medium | Fixed in seam 3: "binding" removed; the key names have one owner; a fallback keeps one owner per fact; the sizes' risk is stated. |
| F7: the accepted set leaves the offline rule unmet for C3, X6 and X7; not in the questions | medium | Fixed: named in the responsibility record, H3's row reworded, new question Q9. |
| F8: `defer` triggers that are states or already met (C2, G6, G21) | medium | Fixed: C2's trigger counts only occurrences after `1e6dc91a`; G6's probe waits for a second failure and the cause-in-the-message ask is Q6; G21's identity-row decision is a decision, not the trigger; the Observations row is corrected. |
| F9: X1's blast radius omits the guards that parse `ps`, `ls` and `resolve` text | medium | Fixed in the guards table (`guard_cli_observe.rs` ranges, `guard_cli_surface.rs:904`). |
| F10: X1's guard needs the analysis record cut at L2; `analysis_core` does not own `analyze.rs` and `inspect.rs`; C1 split | low-medium | Fixed: the guard row is added per verb as its record lands; `analysis_core` owns the two files for additive keys; the C1 split is stated as deliberate. |
| F11: hand-kept counts in a hand-written header; the record's rule (1) against X11's README copy | low | Fixed: counts live only in the generated `index.md`; the record names the exception for examples a test imports or parses. |
| F12: `done` overstated (G16, G9, G10, G14); one file with two owners | low | Fixed: G14's unverified residuals are in its row; the `python/candler-evaluator/README.md` residual is X11's alone; the `store` field of the `ps` and `ls` records that G9 relies on is in `output_contract`'s ids. G16 stays `done` on its leaf's reading (residuals routed to X5, X8, X11). |

### Round 2 (re-check through SendMessage to the same verifier)

Fixed: F2, F3, F4, F5, F7, F9, F10. Partly fixed, with the remainder then fixed by the implementer (not re-checked a third time):

| Finding | What was missing | Disposition |
|:--|:--|:--|
| F1 | the triad line ignored the path loss; no fallback if the validator cannot supply a pointer | Fixed: the compromise row names the loss; the codes review gives the fallback (inner code only where a pointer exists, else `document_invalid` with the inner code in `causes[]`). |
| F6 | `is_setting` is a third owner of the four config keys, so "two items do not own one fact" was not shown | Fixed in seam 3: `is_setting` must read the one table; if it cannot, the item stops and returns to Eliott. |
| F8 | Q6 option A still said a second trigger counts as met | Fixed in round 1's edit (the text now reads "the question returns at the next incident"); confirmed by grep. |
| F11 | the record's exception wording not re-read | Left as written (consequence (4) names the exception for tested examples); not disputed. |
| F12 | `done` rested on unverified residuals | Fixed: G10 and G14 carry an explicit caveat in the verdict column that `done` covers what was read. G16 stays `done`; its residuals are routed to X5, X8 and X11. |
| new | round 2 not recorded | Recorded here. |

Disputed findings for the coordinator: none.
