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

## run_context
- **`CheckedNeeds`: fixed in the text.** The leaf named a type defined nowhere. `assemble` takes the schemas' `Needs` and compiles the globs itself, so no checked form crosses the seam. The registry has already refused unsupported needs by the time the engine calls it (`run_context.md` Step 1).
- **Accepted as chosen:**
  - `needs.trace` is ignored. `snapshots` or `inputs` that are not empty give `ContextError::Unsupported`.
  - A missing metadata path, or one with no recorded value, gives `MissingMetadata`.
  - `materialise(&Assembled, &Path)` refuses a relative workspace, one inside or above the store (symlinks resolved), and an existing `outputs/`.
  - `AssembledArtifact`'s public fields, `FileRule` and `file_rule_keeps`.
  - A case or Unicode-normalisation collision on APFS is a hard `Io` error.
- **A failed `materialise` leaves a partial `outputs/`.** The engine removes the workspace before any retry (`engine.md` Step 2 item 3).
- **The dot rule's edges (verifier MINOR 3 and 4): kept, and written down.** The literal-segment match is position-free (`outputs/.a/**` keeps `.a/.a/f`), and escaped, bracketed or brace-spanning spellings do not count as literal. Both fail safe. R03 §File rule now says so.
- **The mini-corpus has no dot files under `outputs/`,** so RC1's Python vectors do not exercise the dot rule. RC2's hand table does, and that suffices.

## protocol_client verifier notes for the engine
A cancel that lands while the client settles returns `Ok`, and dropping a `Child` does not stop its process. `engine.md` Step 2 item 4 now has the engine re-check `cancel` before `commit` and never drop an in-flight `evaluate` future.

## htn_plugin, before dispatch
- **The manifest path.** OWNED now names `.claude/skills/htn-creator-workspace/candler-plugin.toml` (F3 above), and the Step 1 deliverable is that path.
- **The OWNED regex had a look-ahead `(?!kinds/)`.** This is the same defect as helper_runtime's item 5. It is replaced by `src/htn_creator_candler/[^/]+\.py$`, top-level modules only.

## htn_kinds
- **1: fixed.** The lock gate grepped for `editable`, but uv always writes the root project as `source = { editable = "." }`, so the gate could never pass. Both `htn_kinds` Step 1 and `htn_plugin`'s lock gate now require that line to be the only `editable` one, nothing from `$RW` or an unpinned `$R`, and exactly two instrument sources from `$R` at `28be2c15`. uv spells a `git+file` dependency `git = "file://…"`, so `htn_plugin`'s Candler-pin count now greps that spelling.
- **2: fixed.** OWNED omitted two listed deliverables, `src/htn_creator_candler/__init__.py` and `tests/conftest.py`. The regex now names both.
- **3: accepted.** The repo's `.gitignore` ignores `uv.lock`, so the lock was added with `git add -f`. `htn_plugin` re-locks the same way.
- **4: accepted.** `addopts = "-m 'not golden'"` keeps KP1 out of the default run. The KP1 gate selects it with `-m golden`.
- **5: accepted.** The pinned wheel omits the schema JSON that the grader reads, so the kinds ship a byte-identical copy under `kinds/_data`. `htn_plugin` must keep it in the package data.
- **6: for `htn_plugin`.** The kinds use `tomllib`, which needs Python 3.11, while `requires-python` says `>=3.10`. `htn_plugin` raises it to `>=3.11` when it re-locks.
- **7–11: accepted as chosen.**

## htn_kinds verifier (three MAJORs, one fix round)
- **MAJOR-1, fixed.** A 0/0 fail became `not_applicable`, and KP2's oracle reused KP1's mapping, so nothing guarded it. Eliott's ruling covers a 0/0 pass only. A 0/0 fail stays `fail` with weight 0/0, as M1's legacy mapping says. KP2 gains a fixture that reaches it, with a hand-literal expected outcome.
- **MAJOR-2, fixed.** On non-UTF-8 stdout the grader's strict decode fails and the item is `not_measured`, while the kinds decoded leniently and passed. The kinds now decode strictly too.
- **MAJOR-3, fixed.** A checker that closes `sys.stdout` made the first restore step raise, and the rest never ran. Every restore step now runs on its own.
- **MINOR, fixed:**
  - `on_missing` takes only `fail` or `blocked`.
  - `find_artifact` breaks ties by path.
  - The `.claude` clause has a test.
  - An out-of-range `SystemExit` gives what a real child gives.
- **MINOR, written into `deltas.toml`.** These are rows that KP2 cannot reach, and each is marked with the component that covers it:
  - The checker timeout. The kinds do not spawn, and the helper's item deadline ends a hang.
  - C6 after an assert fail of C1 or C5. Candler blocks it, while the grader computes it. No 0.4.2 spec has such an assert.
- **MINOR, accepted.** The in-process state leaks follow from the design, since the helper forks per item. KP2's docstring says so.
- **NITs: accepted, no change.**
- **SCAFFOLD count, clarified.** The rule "tests equal `Guards:` lines" counts collected tests. In the HTN repo, `addopts` deselects the golden test, so the count is taken with `-m "golden or not golden"`.

## helper_runtime verifier (no BLOCKER or MAJOR, one fix round)
- **M1, fixed.** HR2 had no `dir_info` case with `editable` false or absent, so the spec's bite ("ignore `editable`") left the test green. HR2 gains both cases, each expecting `IdentityError`, and the literal bite is recorded.
- **M2, fixed.** A wrongly typed `deadline_ms` or `kind` crashed the whole helper. `_parse_evaluate` now checks the types and answers -32602, and the helper stays up. The case is a row of HR1's existing table, not a new test.
- **M3, fixed.** A child abandoned after a cancel was never reaped. The helper now reaps abandoned pids with `waitpid(WNOHANG)` before each fork and at exit, and sends no signal (I22).
- **N1 and N2, accepted as notes.** `_early_cancels` can grow for cancels of requests that never arrive. The no-fork fallback has no isolation, as documented.

## helper_release (L2, merged on report)
- **1: noted for the KT.** The schemas' `(?![\s\S])` look-aheads work only under Python `re`. The generated models set `regex_engine="python-re"`; a consumer on pydantic's default Rust regex engine would refuse them.
- **2: accepted.** The leaf asked for the coordinator's OK before editing `ci.yml`, and OWNED named it. The `helper` job is added before `commits`. `release` still needs only `gates` and `commits`, which is right because the helper is not published.
- **3: accepted.** `pydantic>=2.7` becomes the helper's one runtime dependency; it was already locked as a dev dependency. The `exceptiongroup` marker narrowing to `<3.11` is harmless, since 3.11 ships it.
- **4: accepted, binding on L2.** The loop now validates through the models: a non-v7 request id is dropped and logged, a refused request gets -32602 (or -32601 for an unknown method), a non-SemVer version gets -32000 at `describe`, and an outcome that fits `normalise_outcome` but not `OutcomeRecord` becomes `not_measured {instrument_error}`. The Rust client sends `DescribeParams` and v7 ids, so it is unaffected. `engine` and `surfaces` rebase onto this merge before landing and re-run their plug-in tests against it. `htn_plugin` is re-pinned past it.
- **5, 6: accepted.** Cross-field rules stay in `normalise_outcome`. `INVALID_PARAMS` and `METHOD_NOT_FOUND` move to `frames.py`, and the public API is unchanged.

## surfaces (L2, one fix round)
- **1: spec defect, ruled.** The binary gets its import rules as data, from `candler import <corpus> --rules <file.toml> [--store …]`. `--instrument` is removed: `ImportRules.instrument` is a name, and the version and digest come from the folder's pinned evaluator, so the flag had nothing to carry. The legacy names (`htn-results@1`, `htn-creator-eval`, `TIER_D_REPLAY`) cannot be literals in `crates/candler/src`, which NEUT scans; a fixture under `crates/candler/tests/fixtures/` is outside that scan.
  - **The file**, parsed in `commands/import.rs` with serde, `deny_unknown_fields` at every level, one field per `ImportRules` field:
    `format`, `instrument`, `suite_kind`, `metadata = [paths]`; `[harness] name, version_pointer`; `[treatment] pointer, resolved`; `[[snapshot]] kind, pointer`; `[[marker]] pattern` plus exactly one of `code` (a fixed reason code) or `capture` (a group name).
    Every value goes through the primitive's own `FromStr` and `ImportRules::builder()`. A bad pattern, a bad primitive or a builder error is `CliError::Refused`, with the field named.
  - **The reader** is chosen by `harness.name` from a table in the binary. The table holds `claude-code` → `ClaudeCodeReader` only, and any other name is refused. Construct the reader once before importing, so the factory closure never sees a construction error.
  - **Dependencies.** `regex`, `serde`, `candler-harness` and `candler-harness-claude-code` were added to `crates/candler/Cargo.toml` by the coordinator in 14d9142. M2b adds the same harness crates, so the final rebase merges an append-only conflict.
  - **Fixture.** `crates/candler/tests/fixtures/cli/mini-corpus-rules.toml` mirrors `candler-core/tests/support/legacy_rules.rs::rules()` field for field. golden_parity (L3) writes its own rules file the same way.
- **2: CLI2 now, in two stages.** Stage one, in this fix round: import the M1 mini-corpus with the fixture rules into a temp store, then run `show` (the counts are hand literals from the mini-corpus grading files, never the importer's output) and `diff`, with the bite recorded. Stage two comes after engine lands, from the same agent: `evaluate` with the synthetic plug-in, `compared > 0`, and a planted store edit that makes `diff` exit 1.
- **3: accepted as chosen.** That covers the `show` shapes, the text `diff`, `evaluate`'s defaults and JSON, and the `#[non_exhaustive]` `CliError`/`Exit`. The coordinator reconciles `CliError` with M2b at the final rebase.
- **4: accepted.** `diff` compares selections, while I4 violations are `evaluate`'s and SEL1's concern. The KT notes that `diff` does not report them.
- **Coordinator's condition on `--rules` (2026-11-09).** The rules file decides what an import means, so it is provenance.
  - Its sha256 goes into `ImportRecord.rules`, next to the sources (surfaces Steps 4 and 5). A re-import of a finished run with another digest is refused with a `Conflict` that names `import.rules`.
  - **CLI2 gains two rows, without a new guard ID.** The manifest's `import.rules` equals a hand-literal sha256 of the fixture, and a re-import with a one-byte-different rules file exits 2 naming `import.rules`. Bite: the importer skips writing `rules`.
  - **The real HTN rules file** (`htn-results@1`) lives under version control with the HTN plug-in in `$R`, not in Candler. `htn_plugin` adds it in its fix or verifier round, and `golden_parity` pins its digest.
  - **The CLI2 fixture is a test double.** `crates/candler/tests/fixtures/cli/mini-corpus-rules.toml` is the vendored mini-corpus's rules for a test, not the provenance file of a real import.
  - **CliError.** The final rebase reconciles it under the coordinator's `Result<Exit, CliError>` shape and its 0/1/2 mapping.
- **Fix-round report (2026-11-09): landed as stage one; the leaf stays in progress until CLI2 stage two runs after engine.**
  - **1: accepted for now, KT follow-up.** The reader factory falls back to `process::exit(2)`. That path is unreachable after `check_reader`, because the core's `ReaderFactory` cannot fail and `ClaudeCodeReader` cannot be cloned. Follow-up: make `ReaderFactory` fallible, or make `ClaudeCodeReader::new` infallible.
  - **2: noted.** `CliError::Unavailable` is unused; it is reconciled at the final rebase.
  - **3, 4: accepted.** `import::digest_of` is re-exported, and the digest wiring sits in Step 1's commit.

## engine (L2, merged on report)
- **1: accepted; this is how a dependency in the same request behaves.** When a dependency's verdict comes back in the same request, the dependent is sent anyway, and its record is then replaced with `blocked {expectation: <dep>}`. Only a dependent whose dependency verdict is already known (an earlier stage, or `not_applicable`) is held back. The plug-in therefore runs, and pays for, the dependent of a failure inside the request. ENG1 asserts the replaced record. This matches the `deltas.toml` row "C6 after an assert fail of C1 or C5", where Candler blocks and the grader computes. The KT records the cost.
- **2: accepted.** `PlanError::Interleaved` refuses a chain that goes back to an evaluator after another one, and it names the entries.
- **3: accepted; the seam is amended.**
  - `EvaluateError` is `#[non_exhaustive]`. It drops `NotBuilt` and gains `EnvironmentNotReady`, `Suite`, `Io`, `Glob`, `Plan`, `Describe` and `Task`. `surfaces` maps it through `CliError::Failed`, and the workspace checks after the rebase.
  - `engine::regrade(store, plugin, suite, runs, env_root, supervisor)` is the regrade support function golden_parity calls. Its defaults are concurrency 2 and no deadline override.
- **4: accepted, and it closes helper_runtime finding 1.** The engine overwrites `role`, both digests, the request id, the evaluator and the instruments from its plan and `describe`, and keeps only the verdict. A kept record that does not build becomes `not_measured {instrument_error}`.
- **5: accepted.** The request deadline is the sum of the items' deadlines plus 30 s, or the flat override.
- **6: accepted.**
  - One fixture plug-in, `engine`, reuses the helper's `ok` and `echo` kinds and adds `fail` and `counter`. ENG1 has five entries.
  - The guards run `uv sync --frozen` themselves, about 7 s each, so CI's gates job now installs uv (8c01358, with the same pin as the helper job).
- **7, 9: accepted.**
- **8: accepted as scoped.** The cancel paths are covered through `guard_signal` and the protocol client only. golden_parity runs no cancel.
- **Integration review of the seams (coordinator).** After rebasing onto 8c01358 (surfaces landed), the workspace checks and the gates pass. CLI2 stage two is the end-to-end check of the engine's outcome-file layout against the `selection_rates` reader that `show` and `diff` use. It runs next, through surfaces.

## htn_plugin (L2, one fix round, then its adversarial verifier)
- **1: defect in htn_kinds, fixed at the source.** The helper passes `prior` as a sequence of `Prior`, while `kinds/ratio.py` read a mapping, so C6 became `not_measured {instrument_error}`. The fix (df4ec3ba) makes the ratio kind read the sequence and drops the leaf's `_by_id` adapter. Putting the mapping read back makes KP2 fail. The other kinds' annotations still say `Mapping` although they never read `prior`; that is a KT note, not a change.
- **OWNED, widened by the coordinator.** The ratio fix and its two test edits sit in `htn_kinds`'s files, and the import rules file is new. OWNED gains `src/htn_creator_candler/kinds/ratio\.py$`, `tests/test_guard_(deltas|kind_parity)\.py$` and `import/htn-results-rules\.toml$`. `htn_kinds` is landed, so no sibling races these files.
- **2: spec defect, ruled.** `eval.json` and `suite/1` carry no weights and no optional flag. The verifier's "weights" target becomes the roles (gate against quality), which decide the rates, and "optional flags" becomes the explicit `required` and `on_missing` params. HP1's bite "swap two weights" is replaced by the role and ratio swaps the report records.
- **3, 4, 5, 6: accepted.** Ruff stays on py310. The params schemas and the `<<domain_name>>`/`<<alias>>` token convention are the plug-in's to choose, and the registry checks them at preflight (golden_parity's live run is the first Rust-side check). The dropped fields are declared in HP1.
- **The HTN import rules file.** `candler-plugin/import/htn-results-rules.toml` has the CLI2 fixture's body plus one comment line. A mini-corpus import with it gives `{"records":136,"runs":6}`, as the fixture does. Its sha256 is `5d25aa60db68a8340bad2856a8c9fb1e1f99d13dce161d6f08539afb22e56952`, which golden_parity pins.
- **HTNSTD, root half.** pytest's summary line did not reach the captured file. Counted by progress characters: 386 passed, 10 F, 73 E, 1 skipped. The failures and errors are a subset of the baseline's, so the gate passes.
- **KP1** passed in 291.72 s on the fix-round code, before an annotation-only revert.
- **Adversarial verifier (2026-11-09): no BLOCKER or MAJOR.** All 37 entries match the pinned grader apart from the documented deltas. The roles match `_outcome.py`, and C6 takes C5 over C1 with every prior shape the verifier tried. Its own bites killed (B8 `on_missing`, A6 ratio, the ratio id match) and three of the implementer's bites reproduced.
  - **M1, fixed in a fixup of a0d4b866.** HP1 dropped `"required": true` without checking it, so removing it from C-N1 left HP1 green. HP1 now asserts it before dropping it.
  - **M2, covered by golden_parity, no change.** KP1's `CHECKS` table is hand-copied, not read from `suite.json`, so a suite param the kinds misread would escape KP1. GP1 runs `suite.json` itself through the engine on the 240 runs. The verifier's one-off differential (24 runs, `suite.json` params through the real kinds) found 0 differences.
  - **NITs, KT notes.** `DECLARED_MOVES` is documentation. HP2 mutates one fixed byte. The `specs.py` docstring mentions a deadline it does not set (the helper's 60 s default equals the grader's). A duplicate prior id takes the first. `deltas.toml`'s C6 row names only an assert fail, while Candler also blocks C6 after an unreadable C1 or C5. C6 is descriptive, so rates are unaffected.

## The `htn-creator-eval@0.5.0` tag (2026-11-09)
- **Order: tagged before the parity gates**, as the roadmap and the spec leaf say. Eliott recalled the identity ruling of 2026-11-06 ("Fixes after parity become 0.5.x/0.6.0 bumps"), so a fix found by golden_parity ships as 0.5.1. I4 forces that anyway, because a changed outcome under the same version is flagged. If it happens, the gates that name `0.5.0` are amended to name the version under test.
- **Where.** Eliott cleared the HTN main checkout ("`feat/skill-htn-creator` is good to use now"). The release commit 9f537216 `chore(htn-eval): bump htn-creator-eval to 0.5.0` adds the 0.5.0 entry to `infrastructure/CHANGELOG.md` on `feat/candler-htn-plugin`. e96257d9 `merge(htn-eval): land the candler plug-in (htn-creator-eval 0.5.0)` lands it `--no-ff` on `feat/skill-htn-creator`, and the annotated tag `htn-creator-eval@0.5.0` sits on that merge, as 0.4.2 sits on 28be2c15. Nothing is pushed.
- **Versions.** The plug-in reads 0.5.0. The legacy grader packages under `infrastructure/` stay at 0.4.2, since nothing in them changed and the plug-in installs them from 28be2c15.

## golden_parity pilot (2026-11-09): two seam defects, both fixed in Candler
The pilot imported the corpus (240 runs, 10,320 records, 2m15s, 207 MB store) and ran one full regrade at concurrency 4 (11m02s). It used two scratch workarounds. The counts equal `[live_expected]` exactly. `diff` gives compared 8,880, excused 209, changed 0, and D-E1…6 × 240 uncompared, and the rates equal `rates_by_cell.toml`.
- **Defect 1: the engine reads suites as TOML only.** `candler-schemas` documents a suite as TOML or JSON, and the HTN plug-in ships `suite.json`, so the engine and htn_plugin never met on the format. Ruled: the engine reads `.json` as JSON (`SuiteFile::from_json_str`) and anything else as TOML. That is golden_parity's new Step 1, with an ENG1 row.
- **Defect 2: the engine does not activate the plug-in's environment.** The engine only sets `UV_PROJECT_ENVIRONMENT`, so a bare `python -m …` command runs the first `python` on `PATH`. The engine's own fixture hid this behind `uv run --frozen`. Ruled: the engine activates the environment for every spawn (`VIRTUAL_ENV`, its `bin/` first on `PATH`). That is golden_parity's new Step 2, with an ENG1 row. Wrapping commands in `uv run` would re-sync a shared environment from concurrent requests and defeat REG2, so the HTN manifest stays as tagged, and `htn-creator-eval@0.5.0` stands. R03 §Environments and `__design__/architecture.md` (the `environment` row) are amended to match.
- **OWNED widened** to the engine, the registry, `guard_engine.rs` and its fixtures, and `candler-schemas/src/evaluation.rs`. The old Steps 1 and 2 become Steps 3 and 4, and the coordinator dispatches the verifier.

## Regrade throughput (2026-11-09): amendment A1
**Finding.** golden_parity's corpus guard took about 24 min per pass: an import of 135.7 s, then regrades of 642.5 s and 680.8 s. That is 13 items/s. Eliott judged this unsustainable for foundational code and asked for a profile with numbers: [`throughput/00-regrade_profile_v0.md`](../throughput/00-regrade_profile_v0.md). The profile found that the Rust core uses under 0.2 s of CPU per regrade, and that the costs are:
1. **One request, and one Python process, per independent item.** `engine/plan.rs` batches only entries linked by `depends_on`, so PERF12 makes 420 requests for 444 items, and each pays about 135 ms of boot and imports. This departs from R01, where the helper "imports the checker libraries once, then forks one child per item".
2. **The checkers' heavy libraries are re-imported in every fork,** because the parent never imported them.
3. **`F_FULLFSYNC` on every small file.** Rust's `sync_all` and `sync_data` issue it on macOS, at 4.15 ms against 0.06 ms for `fsync(2)`. It is 86% of the import's wall time, and about 36 ms of every request.
4. **Per-request materialising and helper start-up.**

The measured fixes on 12 runs: batching gives 3.6x at `--concurrency 4`, and preloading gives 2x on top of that.

**Ruling.**
- **Batching.** A request carries every applicable entry of one evaluator at one stage of one run, ordered by dependency.
- **Sync.** "fsynced" in R01 means `fsync(2)`, the guarantee `sync_all` gives on Linux. One sync module in `store/` serves every site. The materialised workspace is scratch and is never synced.
- **Helper.** It starts lazily and exits after its last frame.
- **The HTN plug-in.** It preloads the third-party libraries outside `_FRESH_PREFIXES`, behind a new guard KP4. It ships as 0.5.1, because fixes after the tag ship as 0.5.x.
- **The proof.** throughput_proof re-proves GP1–GP3 against 0.5.1. The gates are 100 items/s or more for a regrade and 30 s or less for the import.
- **Not done here.** A warm worker pool, or requests that span runs, would lift the ceiling of about 400 to 600 items/s that the per-item fork sets. That is a KT item, not M2a.

**The guard's cadence.** The corpus guards (GP1–GP3, KP1) leave the before-every-merge set. They are acceptance runs, required before landing a leaf that changes the engine, the store, the helper or the HTN plug-in, and before the PR.

**golden_parity.** It lands on its implementer's gates. Its adversarial verifier moves to throughput_proof, where a corpus pass costs minutes. Its finding 3 goes to that verifier: GP3's disjointness check is never seen failing on its own, because the engine refuses reused ids first. Its finding 4 is fixed by regrade_throughput Step 4.

| Changed | Where |
|---|---|
| A1: `throughput/` with `regrade_throughput`, `htn_preload`, then `proof/throughput_proof` | `__roadmap__/…/parity/throughput/` |
| The corpus guards leave the must-run set | R04 §Minimal Must-Run Regression Set |
| New guard KP4, and an ENG1 row for batching | `htn_preload.md`, `regrade_throughput.md` |

## A1 landings (2026-11-09)
- **regrade_throughput**, landed as 97a4a0f. Its implementer found an older race in the helper, in `isolate.py` `_await`. After a `select` timeout, the loop broke on `waitpid` and could drop an outcome the child had just written. At concurrency 8 that lost one row in 5 of 25 regrades. **Ruling:** fix it in the leaf, as `fix(evaluator-helper): read the item pipe to eof after the child exits`. The fix comes with a deterministic guard (an empty `select` forced in the test) and needs 0 of 25 regrades differing at concurrency 8. A lost outcome breaks parity, and the corpus guard now runs at concurrency 8.
- **htn_preload**, landed as 2f5dd387 on `feat/candler-htn-plugin` and 9dcbc94b on `feat/skill-htn-creator`. Its finding 2 was that the suite README still named 0.5.0, because `translate.py` held the version as a literal. **Ruling:** widen OWNED to admit `suite/README.md` (9f5c4a3), and take the version from the plug-in's single source.
- **The 0.5.1 tag.** It was first cut on 9dcbc94b and withdrawn within the hour, never pushed: the plug-in still pinned `candler-evaluator` at fcdf0aa5, which has the race. **Ruling:** re-pin at 97a4a0f9 (36188dea `build(htn-eval): pin candler-evaluator at the pipe race fix`, plus a CHANGELOG line), and cut the tag again on ae4eef09. From now on, a plug-in release re-pins the helper whenever the helper has changed.

## throughput_proof verifier (2026-11-09)
The adversarial verifier made one corpus pass (3/3 PASS, 161.5 s wall) and found no behaviour defect. Everything it tried to hide behind `equivalence.json` was caught: a wrong run, a nonexistent row, a reversed pair, a dropped row, a duplicate row, and an extra pair. The helper's `_await` lost no frame on any exit path it probed. The sync order holds at every store site, and no alias dodges the grep gate. The checkers' libraries all come in through the preload's four roots, and the forked children share no cache.

| Finding | Severity | Ruling |
|---|---|---|
| ENG1 does not guard three A1 behaviours: dependency-first order, the request deadline as the sum of the items' deadlines plus the margin, and the blocking of a `not_applicable` entry's dependents. A bite on each survives. | MAJOR | Fix in throughput_proof Step 3, with three ENG1 rows. OWNED is widened to admit `guard_engine.rs` and its fixtures. |
| The fixture marker `complete.json` does not hash the plug-in tree or the helper, so a kept store survives a plug-in edit. | MINOR | Fix in Step 3. |
| The "about half an hour" strings in the guard are stale. | NIT | Fix in Step 3. |
| KP4's second test (outcomes with the preload against without it) kills none of the five bites, because `_runner` drops the fresh modules for each run. The first test does all the work. | MINOR | KT follow-up, on the HTN side. The 0.5.1 tag stays as it is: the test asserts nothing false, and the first test guards the property. |
| GP3's request-id disjointness check reads the report. Real id reuse is refused earlier, by the engine's `OutcomeExists`. | MINOR | Keep it as written. A falsified report does reach it, and the per-row `request_id` membership check is its file-level partner. |
| The helper shows each item `prior[]` plus every earlier item of the same request, siblings included. | NIT | This is the helper's rule since helper_runtime finding 4, and kinds pick by expectation id. The KT records it as a wire-contract note. |
| The `.jsonl.tmp` stream file in `frames` and the environment `ready` marker are never synced, and a new index file's directory entry is not dir-synced. These predate A1. | NIT | KT follow-up, taken with the import's other durability and throughput items. |
| Hand literals 8880, 209 and 240 duplicate the oracle. The generator's `status == "pass"` clause cannot be observed on the corpus. The bare `htn_creator_scripts` name is missing from `FRESH`. | NIT | No change. Each one is either correct or harmless. |

**The deadline guard (fix round).** The first fix proved the request deadline by having a kind sleep 33 s, longer than the 30 s margin. That guard is too weak and too slow:
- It only bounds the deadline from below, so a sum-only bite survives.
- On the landing run it took 35.86 s of a 35.93 s workspace `nextest` run, which made it the critical path. The next slowest test took 8.8 s.

**Ruling:** make the deadline observable rather than wait for it.
- throughput_proof gains Step 3, `feat(evaluation): report the deadline each request was given`. It adds `EvaluateReport::request_deadlines` and widens OWNED to admit `engine/(mod|run).rs` and `public_surface.rs`.
- The ENG1 row then pins the exact hand sum, with no kind that sleeps.

A guard that waits on a timeout to prove a duration is not accepted when the duration can be read.

**The import gate (Eliott, 2026-11-09).** The guard's debug binary imports the corpus in 44.4 s. A release build takes 7.7–9.1 s. Eliott: "then it's completely fine."
- **Ruling:** the 30 s import gate measures the release `candler`. The debug figure is recorded but not gated.
- **Follow-up:** the import waste the probe measured (the gzip round trip, the sync after every blob, and one core) goes to the KT as a follow-up, not to an M2a amendment.

## Rebase onto main after PR #4 (2026-11-09): A36 and the CLI error
M2a lands second, so it owns A36 and the reconciliation with M2b's CLI. The coordinator assigned both.
- **A36, the store root.** M2b ruled "the store root is `--store`, else `Config`, through `store_root` in `ps.rs`", and the helper was to move to `commands/common.rs` once M2a arrived. Before the reconciliation the binary had two paths:
  - a global `--store` (with `CANDLER_STORE`), read by the M2a verbs through the resolver;
  - a `--store` of its own on `ps`, `kill` and `watch`, beside the global one, while `run` and `supervise` called `store_root(None)` and read the configuration only.

  clap copies a global value, `CANDLER_STORE` included, into a subcommand arg of the same id. So the job verbs' own flag did not shadow the global one: the first bite, which gave `kill` its flag back, survived. The real gap was `run` and `supervise`.

  **Ruling:** there is one store root, `common::store_root`. It takes `--store` from anywhere on the line, else `CANDLER_STORE`, else the configuration's `store.root`, else the core default, and the resolver layers all of it. The job verbs lose their own flag and take the global one. `supervise` still prefers the launcher's `CANDLER_STORE_ROOT`. `evaluate` reads the same setting from the resolution of its own preflight (`common::resolved_root`). A new row in `guard_cli_kill` checks it: a job verb finds the store through the env or a leading flag. Its bite is `kill` calling `store_root("kill", None)`, the pre-A36 shape of `run`, and it kills the row.
- **CliError.** M2b's `thiserror` enum is kept, and the M2a verbs return it. `main` prints `candler: {error}`, so the M2a reasons no longer carry their own `candler <verb>: ` prefix, which would have printed `candler: refused: candler import: …`.
- **The sandbox contract.** Main records `candler-container@1`. No M2a fixture or test asserts the value for a Candler-produced run. The legacy `v5` in the imported mini-corpus is the corpus's own field, so it stays.
- **NEUT: the planner package allowance (coordinator, 2026-11-09).** After the rebase, `guard_layering_i1` failed on one line of M2b's `commands/supervise.rs`, `const PLANNER_PACKAGE: &str = "gtpyhop";`. The M2a skeleton had extended the scan to `crates/candler/src`. This was the only failure in the workspace suite (338 of 339).

  **Ruling:** option B, one named allowance, stays in M2a. Option A, which takes the name out of the binary, goes to M2c. Eliott: "Gate in M2c as amendment to the roadmap." The coordinator records it as amendment A2 of `__roadmap__/candler-v1/contracts/parity/alignment/m2c_wire_alignment.md`, on its own rdm PR.
  - M2c then removes the planner axis (`PlannerAxis`, `PlannerProbe`, `planner_package`, `provenance.planner.*`). It records the environment packages the plug-in declares under a neutral path instead, and denies the whole word `planner` in the binary. M2a touches none of this.
  - The allowance is the exact file and the exact line (`ALLOWANCE` in the guard), never a pattern. The line is taken out once before the scan, so the rest of that file is still scanned.
  - The guard fails when the line is gone, so a stale allowance cannot stay. The comment beside it names the follow-up and says to delete it, together with its self-test, when the follow-up lands.
  - A planted self-test, `the_allowance_excuses_its_one_line_and_nothing_else`, checks four cases: the allowed line alone is clean; a second `gtpyhop` constant in the same file is found; `rikyu` in the same file is found; the allowed line in another file is found.

  | Bite | Killed by |
  |---|---|
  | A second `const PLANNER_TOO: &str = "gtpyhop";` in `supervise.rs` | The binary scan |
  | `// the rikyu MCP server` in `commands/run.rs` | The binary scan |
  | The filter excuses the whole allowed file | The planted self-test |
  | The allowed line changed to `"planlib"` (a stale allowance) | The binary scan's exactly-once check |
