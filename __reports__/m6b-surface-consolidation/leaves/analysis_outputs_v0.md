# Analysis outputs verdicts (v0)

Leaf `analysis_outputs` of M6b. Eight primary ids: F2, F4, C6, C8, C9, C10, G13, X8. Covered ids: G3 (F2, F4), H5 (F2, F4, C6; F5 is `cli_contract`'s), H6 (C8, spend half; the pre-run estimate is `execution_preflight`'s C1), H7 (C9), G4 (C10). Code claims cite `path:line` read through colgrep on the tip (`a0df4d48`); no baseline, no model run, no build.

## Live on the tip

| Id | Live | Evidence |
|:--|:--|:--|
| F2 | Yes, but the mechanism differs from the input | A and B are the first two levels met while walking the cells in ascending cell-digest order, not "recording order": `compare_pairs` builds `levels` from `place` over `strata_of` cells (`crates/candler-core/src/analysis/report.rs:955-1066`, `:664-689`); `CellKey` is a digest (`crates/candler-core/src/evaluation/selection/rates.rs:12-15`). R01 says the same ("recorded key order, not the declared order", `__design__/architecture.md:1347`). The JSON names the levels, A first (`crates/candler/src/commands/analyze.rs:291-304`). |
| F4 | Yes, as two separate fields | `Tested` carries `reject` (Holm-adjusted p below alpha) and `reading` (the interval's reading) (`crates/candler-core/src/analysis/report.rs:309-352`); `reading` comes from the unadjusted interval (`:706-744`, enum at `:354-362`); `reject` is set by `adjust` over one family per comparison and role (`:1068-1104`; `crates/candler-core/src/analysis/blocking.rs:493-514`). Both are printed (`analyze.rs:258-289`). R01 defines `reading` as the interval's (`__design__/architecture.md:1402-1406`). |
| C6 | Partly: every part exists, the headline does not | Per-arm valid runs, gate and quality rates, `instrument_error` (`analyze.rs:162-213`); per-pair `levels`, stratum verdict and causes, `reject`, `p_holm`, `omitted` reasons (`analyze.rs:258-304`). Nothing selects "the decisions"; job state is in `inspect <job>` (`crates/candler/src/commands/inspect.rs:311-337`). |
| C8 | Partly: recorded per run, never summed | `run.cost_usd`, `run.usage`, `run.num_turns` are measured values of each run (`crates/candler-core/src/runner/evidence/run.rs:69-99`), shown by `inspect <run>` under `measured` (`inspect.rs:205-275`). Harness totals are taken "each as written" (`crates/candler-harness-claude-code/src/lib.rs:175-183`). The only sum is the live job meter that enforces caps (`crates/candler-core/src/jobs/orchestrate/schedule.rs:128-136`, `caps.rs:105-133`); it is not stored as a report. `analyze` and `inspect <job>` print no spend (`analyze.rs:162-213`, `inspect.rs:311-337`). |
| C9 | Yes, but the premise names a wrong verb and a wrong source | `show` has no run argument and prints outcome counts or rates (`crates/candler/src/commands/show.rs:11-27`, `:38-110`). The raw stream is stored framed in `raw/<stream>.jsonl.gz` (`crates/candler-core/src/store/run.rs:166-215`), and a normalised trace is already written beside it through the harness's `TraceReader` (`crates/candler-harness/src/reader.rs:56-86`, `crates/candler-core/src/trace/mod.rs:106-178`, `store/run.rs:217-225`, `crates/candler-core/src/store/layout.rs:73-77`). No verb prints it. |
| C10 | Yes, by ruling | No field of an arm or a run says two replicates are alike (`analyze.rs:162-213`, `inspect.rs:205-275`). The ruling (2026-11-09: RiVault's limit, Candler does nothing, no workaround) is in `__reports__/m2b-execution-parity/04-knowledge_transfer_v0.md:78`. |
| G13 | Yes, and already a Future features row | `pending_runs` calls a run complete when each plan entry's id has a record for the evaluator identity; params are not compared (`crates/candler-core/src/evaluation/engine/pending.rs:10-54`). `show` without `--rates` counts every current record (`crates/candler/src/commands/show.rs:38-110`). Row: `__design__/architecture.md:1800`. |
| X8 | Yes | `docs/statistics.md:165` says paired tests are "not offered yet". No paired test exists in `crates/candler-core/src` (colgrep, pattern `McNemar\|paired`: no hit); M6a built blocked tests (`blocking.rs:236-257`, `:493-514`; CMH, exact strata and permutation as `Battery` in `report.rs:706-744`). R01 says the design is not paired and a paired test does not apply (`__design__/architecture.md:1332-1335`), and also that "M6a builds the paired tests" (`:1757`): the two disagree. The Future features table has no paired-design row, though `:1335` says one exists. |

## The asks questioned

**F2 and F4 (with G3).** Neither is a wrong number. F2: the difference is B minus A over the two levels as the JSON names them (`levels`, A first), so the sign is correct; what is arbitrary is which level is A, because A is whichever level has the lower cell digest. An unrelated setting that changes a digest flips every sign of the pair, and two reports of one pair can disagree. That is a predictability defect, not a falsehood. R10's remedy, "the experiment file's declared order", is not always available: `analyze` runs without the file (`analyze.rs:25-46`, `:77-127`; the file is only read to check factors, `:134-147`), so a rule that uses it would give a second orientation when the flag is absent. F4: `reading` answers "does this interval exclude 0", `reject` answers "does the Holm family reject". They answer different questions, so "judge the reading on Holm" would make `reading: effect` claim an interval fact from a p-value, while the Newcombe interval is not adjusted. The misreading is that a reader takes `reading` as the decision. Guards and risk: orientation touches `crates/candler/tests/it/guard_golden_design.rs:402-433` (6 + 3 comparisons by shape), `crates/candler-core/tests/it/guard_analysis_report.rs:1428-1514` ("the model comparison is the second") and `crates/candler-core/tests/it/guard_known_truth.rs:581-724` (planted outcomes on the golden shape); those pin which level is A, so they would be re-pinned. The 240-run golden parity compares outcome rows (`__design__/architecture.md:29`), and `analyze` never writes the store (`analyze.rs:1-20`), so no DIGEST moves; GOLDEN risk is limited to those three sign-pinning guards.

**C6 and H5.** The summary is a rendering of data `analyze` already computes, with one new derivation: the list of decisions, one per tested entry, each with level labels, the B minus A difference and its interval, `p_holm`, `reject`, the family size, and the reasons for what was not computed (`omitted`, refused strata). Job state is not part of it: `inspect <job>` has it, and joining the two would stitch two operations. Where it lives: core derives (`Report` gains the list), `analyze` prints it as one key, and F5's text rendering (judged in `cli_contract` under D1) is a rendering of the same list. Seam: this record names the data and its fields; `cli_contract` names the format and whether the list is in a schema.

**C8 and H6's spend half.** Candler's to sum: the per-run values it already records from sealed runs. The provider's to report correctly: the cost a harness reports ("each as written", `lib.rs:175-183`); G4 says RiVault counts cache hits as paid, so the sum is an upper bound there, and Candler does not correct a provider's number. Two limits keep the sum honest: a run whose harness gave no total is left out of the measured values (`run.rs:69-99`), so the sum must say how many runs it covers; and `run.usage` is the harness's own shape, so a token sum would need a harness-neutral shape first (not asked for here, see the Future row in C8). The pre-run estimate half of H6 is `execution_preflight`'s C1.

**C9 and H7.** Neither the harness driver nor the consumer needs to decode the raw stdout: the driver's decoding (`TraceReader`) already ran when the run was sealed, and its neutral output, the normalised trace, is stored. Printing that trace is a rendering over neutral events, so one renderer serves every harness. The consumer's home-made script exists only because no verb prints the trace. The verb is not `show` (no run argument); `inspect <run>` is the per-run verb, and the choice is `cli_contract`'s.

**C10 and G4.** The cache is the provider's behaviour and the ruling stands. The proposed flag also cannot be made true: a raw stream carries session ids and times, so identical streams will not match, and comparing the model's replies needs harness-specific rules; and an agent at temperature 0 repeats legitimately. A flag that cannot tell a cache hit from a deterministic agent would mislead. What Candler already says about independence is in `docs/statistics.md` ("Candler cannot check that for you"), which covers the validity of n as a documented non-claim.

**G13.** Mostly a fact to document, partly behaviour: the row exists with a trigger. Two bullets of R06 (`prior[]` holds earlier siblings' records; a dependent costs its own item) are facts for a skill, not defects; the other two (regrade gap, concurrent double grading) are the Future row's.

**X8.** The line is wrong in its tense: "yet" promises a feature that R01 rules out (runs are not paired, `__design__/architecture.md:1332-1335`). M6a blocked, and did not pair. `analyze`'s field names agree with R01 where R01 names them (`levels`, `reading` "effect" and "inconclusive"); R01 never names `p_holm`, `reject`, `battery`, `blocks`, `fallback`, `per_block`, `or_mh`, `homogeneity_p`, `omitted`, `instrument_error`, which the JSON carries (`analyze.rs:162-304`): a field list is X4's (`cli_contract`, D1). Three stale lines go with it: `__design__/architecture.md:1757`, the missing Future row promised at `:1335`, and `:328`, which says `analyze` finds the experiment file by digest while the code reads only `--experiment`.

## Verdicts

### F2: A and B orientation of a comparison
- **Verdict:** accept
- **Covers:** G3 (the F2 and F4 half; F3 is not an input), H5 (the level-label half)
- **Live on the tip:** yes, with a different mechanism: A is the lower cell digest, not the first recorded (`crates/candler-core/src/analysis/report.rs:955-1066`, `:664-689`; `crates/candler-core/src/evaluation/selection/rates.rs:12-15`; documented at `__design__/architecture.md:1347`).
- **The ask questioned:** the sign is right and labelled; what is arbitrary is the orientation, which an unrelated digest change flips. R10's "declared order" is not always at hand (`analyze.rs:25-46`).
- **Responsibility:** Candler's (R01: analysis in `candler-core`; a value is derived, never asked for, I12). It is a derived property of the levels, so it needs no option and no file.
- **Blast radius:** `candler-core/src/analysis/report.rs` only (`compare_pairs`); no `schemas@` release (analyze JSON is not a schema type, `analyze.rs:258-304`); no `codes.toml` row; guards to re-pin: `guard_golden_design.rs:402-433`, `guard_analysis_report.rs:1428-1514`, `guard_known_truth.rs:581-724`, `guard_analyze.rs:1252-1312`; DIGEST: none (`analyze` never writes the store, `analyze.rs:1-20`); GOLDEN: outcome-row parity is untouched, only sign-pinning guards move.
- **Triad:**

  | | Development | Codebase | Consumption |
  |:--|:--|:--|:--|
  | Sustainable | 0: one function, three guards re-pinned once | +: one rule replaces an accident | +: one report, one orientation, across stores |
  | Scalable | 0: no new cost | +: holds for any factor and level count | +: stable as factors are added |
  | Efficient | 0: no extra work | 0: a sort | +: no reader re-derives the sign |

- **Compromise:** none. The natural order is not the experimenter's declared order (tags such as `v0.10` sort before `v0.9`); the labels from C6 keep that harmless, and the declared order stays out because `analyze` can run without the file.
- **Idioms:** follows "values derived, never asked for" and "every new option must say why it cannot be derived" (no option); follows thin surfaces (the rule is in core). Breaks nothing.
- **Reason or owner:** the arbitrary orientation is a predictability defect, small to fix, and it must precede the text rendering and the summary so they print one order.
- **Design suggestion:** files: `crates/candler-core/src/analysis/report.rs` (the `levels` list in `compare_pairs` is sorted by a natural order before pairs are formed: scalars by value, a treatment by `(name, tag)` text then commit, other values by canonical JSON text); the guards above are re-pinned. Test per D3, one: **guard**, `orientation_ignores_cell_digests`: two stores with the same levels whose cell digests sort in opposite order (an unrelated setting differs) give the same `Comparison.levels` and the same sign of `diff`. Question: does A and B depend on anything but the levels? No existing test answers it: `guard_analysis_report.rs:1428-1514` and `guard_known_truth.rs:581-724` fix one digest order and assert signs, never vary it. Sustainable: one fixture that varies one setting, asserting a relation and not a number; scalable: independent of how many levels or factors exist. Size: about 8 Sonnet calls.
- **Future row:** n/a.

### F4: reading is judged before Holm
- **Verdict:** reject
- **Covers:** G3 (the F4 half), H5 (the Holm-headline half)
- **Live on the tip:** yes, as two fields: `reading` from the interval (`crates/candler-core/src/analysis/report.rs:706-744`, `:354-362`) and `reject` from Holm (`:309-352`, `:1068-1104`; `crates/candler-core/src/analysis/blocking.rs:493-514`), both printed (`crates/candler/src/commands/analyze.rs:258-289`).
- **The ask questioned:** the numbers are right and each field answers its own question; the reader's risk is taking `reading` for the decision. Re-judging `reading` on Holm would state an interval fact from a p-value, while the interval is not adjusted.
- **Responsibility:** Candler's output, but the proposal as worded is wrong; the real problem belongs to C6 (headline is `reject`) and to X4's field names (D1).
- **Blast radius:** none, since nothing changes here.
- **Triad:**

  | | Development | Codebase | Consumption |
  |:--|:--|:--|:--|
  | Sustainable | − : a merged field hides two questions | − : `Reading` would mean two things | − : an interval claim from a p-value |
  | Scalable | 0: n/a | − : each new test must say which meaning | − : readers cannot tell which one they have |
  | Efficient | 0: n/a | 0: n/a | 0: n/a |

- **Compromise:** the consumption cost (a misreadable pair of fields) is real; it is answered by C6's headline and by naming in D1, which cost no meaning.
- **Idioms:** a verdict states its statistical meaning (R01 §Readings: "no difference" is never stated); breaks "one fact, one place" if merged.
- **Reason or owner:** reject the re-judging; the misreading is handled by C6 (the list of decisions uses `reject`, shows `reading` as the interval's reading) and, if renamed (`reading` to an interval-named key), by `cli_contract` under D1.
- **Design suggestion:** n/a.
- **Future row:** n/a.

### C6: a summary of an analysis
- **Verdict:** accept
- **Covers:** H5 (the summary half; F2 gives the order, F5 the text)
- **Live on the tip:** partly: every field exists, no list of decisions (`crates/candler/src/commands/analyze.rs:162-304`); job state is in `inspect <job>` (`crates/candler/src/commands/inspect.rs:311-337`).
- **The ask questioned:** a summary is a rendering of `Report` plus one derivation, not a new operation. The job-state part is a different operation's data: joining them is a stitched feature, so only the analysis part is accepted.
- **Responsibility:** Candler's: the derivation (which entries decide what) is a fact about the statistics and sits in `candler-core`; the surfaces print it (R01: one operation API, thin surfaces).
- **Blast radius:** `candler-core/src/analysis` (a `Summary` built from `Report`), `crates/candler/src/commands/analyze.rs` (one key); `schemas@` release only if D1 moves the analyze JSON into `candler-schemas` (`cli_contract`); no `codes.toml` row; guards: `guard_analyze.rs:1252-1312` (names the test and blocks; additive key); DIGEST none; GOLDEN none.
- **Triad:**

  | | Development | Codebase | Consumption |
  |:--|:--|:--|:--|
  | Sustainable | 0: one more derived struct | +: one source for CLI, text and M7 | +: a reader cannot misread `reading` for `reject` |
  | Scalable | 0: n/a | +: grows with entries, no new logic per test | +: same shape for any design size |
  | Efficient | 0: n/a | 0: a pass over entries | +: replaces about 5 calls and a fold |

- **Compromise:** none; where it is printed (a key of `analyze` or a verb) is D1's, and this record asks for no new verb.
- **Idioms:** follows "operation API in core, thin surfaces", "values derived, never asked for"; follows no second layer of validation (it derives from the entries); text is a rendering, never a second source (D1).
- **Reason or owner:** the headline is the Holm decision; each line states the pair by level labels (B minus A), the interval, `p_holm`, `reject`, the family size, and, for what was not computed, the reason.
- **Design suggestion:** files: `crates/candler-core/src/analysis/summary.rs` (new module, in the analysis namespace), `crates/candler-core/src/analysis/report.rs` (`Report` holds it), `crates/candler/src/commands/analyze.rs`. Test per D3, one: **guard**, `summary_agrees_with_entries`: on the golden-shape fixture of `guard_golden_design.rs:402-433`, every tested entry has exactly one decision, whose `reject` equals `Tested.reject`, whose label pair equals `Comparison.levels`, and every omitted entry has its reason. Question: can the headline disagree with the entries it summarises? No existing test answers it: `guard_analyze.rs:1252-1312` checks that the JSON names the test and blocks, and nothing derives a decision list. Sustainable: it reuses a fixture that exists and asserts a relation, so retuning a statistic does not break it; scalable: it covers every entry whatever the design size. Size: about 12 Sonnet calls.
- **Future row:** n/a.

### C8: spend per run and per experiment
- **Verdict:** accept
- **Covers:** H6 (the spend half)
- **Live on the tip:** partly: per run recorded and shown (`crates/candler-core/src/runner/evidence/run.rs:69-99`; `crates/candler/src/commands/inspect.rs:205-275`); the sum is only the live cap meter (`crates/candler-core/src/jobs/orchestrate/schedule.rs:128-136`).
- **The ask questioned:** the sum is Candler's, the numbers are the provider's. The sum is of values the harness reported "each as written" (`crates/candler-harness-claude-code/src/lib.rs:175-183`); Candler does not correct them (G4: cache hits count as paid, so on RiVault the sum is an upper bound). The tokens half needs a harness-neutral `usage` shape first, so this record accepts the cost sum and the run counts.
- **Responsibility:** Candler's (I13: budget is Candler's; a report of what a job spent is the same fact the meter enforces). The provider's: the correctness of what it reports.
- **Blast radius:** `candler-core/src/analysis` (a spend sum over sealed runs), `analyze.rs` (a `spent` per arm), `inspect.rs` (`render_job`: a `spent` for the job); no `schemas@` release unless D1 moves outputs; no `codes.toml` row; guards: none pin the keys; DIGEST none (reads manifests); GOLDEN none.
- **Triad:**

  | | Development | Codebase | Consumption |
  |:--|:--|:--|:--|
  | Sustainable | 0: n/a | +: one sum, one place, no transcript scripts | +: no home-made decoding to learn spend |
  | Scalable | 0: n/a | 0: grows with runs, linear | +: per arm and per job |
  | Efficient | 0: n/a | 0: n/a | +: no pass over transcripts |

- **Compromise:** a stored sum and the live meter are two numbers for one fact; the sum is computed from sealed manifests only and says how many runs it covers (runs without a total, unsealed runs), so the two are not compared.
- **Idioms:** follows I13, "values derived", "each records its source": the sum names `run.cost_usd` as its source; breaks nothing.
- **Reason or owner:** it removes the one measurement the HTN lane did by script; the cost of misreading (a partial sum read as complete) is closed by the count.
- **Design suggestion:** files: `crates/candler-core/src/analysis/spend.rs` (new; sum of `run.cost_usd` over run views, the runs counted, the runs without a total), `crates/candler/src/commands/analyze.rs` (`arm_json`), `crates/candler/src/commands/inspect.rs` (`render_job`). Test per D3, one: **guard**, `spend_sums_recorded_costs_and_counts_the_rest`: three sealed runs, two with `run.cost_usd` and one without, plus one unsealed: the sum is the two costs, `runs_counted` is 2, `runs_without_total` is 1. Question: is the sum exactly the recorded values, never hiding a missing one? No existing test answers it: the caps tests exercise the live meter (`caps.rs:105-162`) and `run_values` only builds per-run values. Sustainable: a fake store, no model; scalable: asserts a relation on the recorded path, so a harness change does not touch it. Size: about 10 Sonnet calls. Tokens: a separate Future row below, not part of this build.
- **Future row:** (for the tokens half only) `| Token sums in reports | \`analyze\` and \`inspect <job>\` sum tokens beside cost | \`run.usage\` is the harness's own shape, so sums differ by harness | A second harness whose usage shape the first sum cannot read, or a report that needs tokens |`

### C9: show a run's transcript
- **Verdict:** accept
- **Covers:** H7
- **Live on the tip:** yes: no verb prints the trace; the normalised trace is stored (`crates/candler-core/src/store/layout.rs:73-77`, `crates/candler-core/src/trace/mod.rs:106-178`, `crates/candler-core/src/store/run.rs:217-225`); `show` has no run argument (`crates/candler/src/commands/show.rs:11-27`).
- **The ask questioned:** not "render a harness's raw stdout" (the driver's `TraceReader` did that decoding at seal time, `crates/candler-harness/src/reader.rs:56-86`), but "print the neutral trace Candler already stores". The consumer's script exists because no verb does.
- **Responsibility:** Candler's, in core: rendering neutral events is harness-independent. The driver's part (decoding) is done; adding transcript logic to a driver would repeat it per harness.
- **Blast radius:** `candler-core/src/trace` (a render of `TraceFile` events), `crates/candler/src/commands/inspect.rs` (a flag; the verb is `cli_contract`'s call since `show` takes no run); no `schemas@` release unless D1 moves outputs; no `codes.toml` row beyond a refusal for a run without a trace if the build finds one is needed (to be checked against the existing registry); guards: none pin it; DIGEST none (read only); GOLDEN none; I19 (secrets): the trace is already stored, so it holds what the store holds.
- **Triad:**

  | | Development | Codebase | Consumption |
  |:--|:--|:--|:--|
  | Sustainable | 0: n/a | +: one renderer for every harness | +: no per-agent decode script |
  | Scalable | 0: n/a | +: a new harness gets it free | +: any run length (stream the lines) |
  | Efficient | 0: n/a | 0: reads one file | +: no decoding of `jsonl.gz` by the consumer |

- **Compromise:** a transcript of a long run is large; it prints as one line per event so a consumer can stop reading; no paging option is added.
- **Idioms:** follows "operation in core, thin surface", "the harness reader owns the stream format, the core owns the neutral trace"; follows "every new option must say why it cannot be derived" (a flag chooses output, not a setting).
- **Reason or owner:** the work is a rendering of an existing, stored fact.
- **Design suggestion:** files: `crates/candler-core/src/trace/` (one `render` function over a trace's events), `crates/candler/src/commands/inspect.rs`. Test per D3, one: **guard**, `transcript_lists_every_event_in_order`: render the trace of the existing claude-code harness fixture; line count equals event count and the order equals the trace's. Question: does the transcript drop or reorder events? No existing test answers it: the trace tests prove raw to trace (`normalise`), nothing renders it. Sustainable: it runs on a stored fixture and is independent of the provider stream; scalable: the assertion is the relation, so a new event kind needs no edit. Size: about 8 Sonnet calls.
- **Future row:** n/a.

### C10: flag identical replicates
- **Verdict:** owner:provider
- **Covers:** G4
- **Live on the tip:** yes, by ruling: nothing flags it (`crates/candler/src/commands/analyze.rs:162-213`, `crates/candler/src/commands/inspect.rs:205-275`); the ruling is `__reports__/m2b-execution-parity/04-knowledge_transfer_v0.md:78`.
- **The ask questioned:** a flag cannot be true. Streams of identical replies differ (ids, times), so equality needs harness-specific reply rules; and a temperature-0 agent repeats legitimately, so the flag cannot tell a cache hit from determinism.
- **Responsibility:** the provider's (the cache and its cost counting); the experimenter's (n and independence, documented at `docs/statistics.md`, "Independence").
- **Blast radius:** none.
- **Triad:**

  | | Development | Codebase | Consumption |
  |:--|:--|:--|:--|
  | Sustainable | − : a rule per provider | − : provider knowledge in core | − : a flag that can mislead |
  | Scalable | − : each provider adds a rule | − : ditto | 0: n/a |
  | Efficient | 0: n/a | 0: n/a | + : one less field to read |

- **Compromise:** none: the three costs outweigh the one gain.
- **Idioms:** breaks "Candler must not over-specify for one provider" (the ruling), "domain-agnostic core" (I1).
- **Reason or owner:** the ruling applies; the provider reports cache hits or bypasses them (only the body field `"cache": {"no-cache": true}` bypasses it, per the same report). The skill sentence R06 proposes ("check run distinctness") is the docs leaves' call.
- **Design suggestion:** n/a.
- **Future row:** n/a.

### G13: re-grade surprises
- **Verdict:** defer
- **Covers:** none
- **Live on the tip:** yes (`crates/candler-core/src/evaluation/engine/pending.rs:10-54`; `crates/candler/src/commands/show.rs:38-110`); already a row at `__design__/architecture.md:1800`.
- **The ask questioned:** it names five behaviours; two are facts for a skill, three are the Future row's. No report of a user-visible mismatch exists yet; the row's trigger is not met.
- **Responsibility:** Candler's (evaluation), waiting on its trigger.
- **Blast radius:** if built: `candler-core/src/evaluation/engine` and `selection`; `show.rs`, `inspect.rs`; guards on pending selection; DIGEST: a regrade writes new records, not digests, but check `I4`; GOLDEN: regrade parity must stay.
- **Triad:**

  | | Development | Codebase | Consumption |
  |:--|:--|:--|:--|
  | Sustainable | − : changing pending selection touches parity guards | + : one count per fact | + : no surprise |
  | Scalable | 0: n/a | + : one rule | + : n/a |
  | Efficient | − : large change for no reported pain | 0: n/a | 0: n/a |

- **Compromise:** development cost wins today; consumption cost is held down by documenting the behaviour.
- **Idioms:** follows "Future features are built only when their evidence trigger is met".
- **Reason or owner:** the row exists; keep it and widen it with the concurrent double-grading case (no per-run claim).
- **Design suggestion:** n/a.
- **Future row:** `| Two counts, a regrade gap and double grading | \`show\` without \`--rates\` and \`inspect\` count every current record; \`--pending\` does not regrade a run after its suite entry's params change (\`pending.rs:10-54\`); two concurrent evaluations can grade one run twice | One count per fact, and a regrade that follows params | A user-visible mismatch in a report, or a double-graded run seen in a store |`

### X8: statistics.md against M6a, and the field names
- **Verdict:** accept
- **Covers:** none
- **Live on the tip:** yes: `docs/statistics.md:165` ("Paired tests are not offered yet") against `__design__/architecture.md:1332-1335`, and `:1757`; no paired test in `crates/candler-core/src` (colgrep); blocked tests at `crates/candler-core/src/analysis/blocking.rs:236-257`.
- **The ask questioned:** the line is wrong in its tense, not in substance: paired tests are not applicable to unpaired runs; "not offered yet" promises one. Field names: R01 names `levels` and the readings correctly; it does not name the other fields (see `## The asks questioned`).
- **Responsibility:** Candler's docs (`docs/statistics.md` is web-wide; R01 is development).
- **Blast radius:** `docs/statistics.md:165`; `__design__/architecture.md:328`, `:1335`, `:1757`; no code; no `schemas@`; no `codes.toml`; no guard (prose); DIGEST and GOLDEN none.
- **Triad:**

  | | Development | Codebase | Consumption |
  |:--|:--|:--|:--|
  | Sustainable | + : one fewer contradiction | + : docs and design agree | + : no promise of a missing test |
  | Scalable | 0: n/a | 0: n/a | 0: n/a |
  | Efficient | 0: about 4 calls | 0: n/a | + : no reader hunts for a paired test |

- **Compromise:** none.
- **Idioms:** follows "docs are the web-wide surface"; follows the fail list (no test per incident: the edit is one correction).
- **Reason or owner:** correct the tense, and settle the contradiction in R01.
- **Design suggestion:** files: `docs/statistics.md:165` (say that runs are not paired, so a paired test does not apply, and drop "yet"); `__design__/architecture.md:1757` (drop "M6a builds the paired tests"), `:1335` (either add the Future row it promises, with the trigger "a design that pairs runs", or drop the clause), `:328` (the file is named by `--experiment`; `analyze.rs:77-127` does not look it up). Tests per D3: none; the contract-page drift tests do not cover prose, and a test for a stale sentence would be a test per incident. The field list of `analyze` is X4's (`cli_contract`). Size: about 4 Sonnet calls.
- **Future row:** n/a.

## Seams

- **C6 and F2 with D1 (`cli_contract`).** This leaf names the data and order of the summary and the orientation rule; `cli_contract` decides the format, whether the list lives in `candler-schemas`, and F5's text as a rendering of the same list. Neither should add a second source for the decisions.
- **F4 with X4 and D1.** If `reading` is renamed to say it is the interval's, that is a schema-name decision for `cli_contract`; this leaf only rejects the re-judging.
- **C8 with `execution_preflight` (C1, H6 split).** The pre-run estimate and the post-run sum are one fact at two times; the sum here is read from sealed manifests and the live meter is untouched. `verdicts` writes H6's row from both.
- **C9 with `cli_contract`.** The verb: `show` takes no run, `inspect <run>` does; `cli_contract` rules which verb and whether the flag falls under D1's one output contract.
- **C10 and G4 with `docs_and_skills`.** The skill sentence about run distinctness belongs there; no code.
- **G13 with `docs_and_skills`.** The two fact bullets (`prior[]`, dependents) are for a skill.
- **X8 with `docs_and_skills` and `cli_contract`.** The doc edits (`docs/statistics.md:165`, R01 `:328`, `:1335`, `:1757`) touch the contract and docs leaves' files; the `analyze` field list is X4's.
- **F2 and F3.** F3 (tied weights) is not an input; it changes A9 and is M6a's follow-up.
