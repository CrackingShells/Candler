# Candler: gotchas and usage patterns for a consuming agent (input to M7)

Repo: Candler at detached origin/main 5199afc7 (core v0.13.0, schemas@0.11.3, candler-evaluator 0.1.0 per `docs/contract/index.md`).
Sources read: KT reports m1, m2a, m2b, m2c, m4 (x2), m5b, m6a, wire-schemas; v1-review findings and amendments A6-A8; interlude audit; m3 leaf and probe reports; roadmap Gotchas of m1..m6a; memory folder; HTN repo `__reports__/candler_core/` (cli_defaults, lane_overlap, architecture) and `candler-plugin/`.
"Live on main" is judged from `docs/contract/index.md`, `docs/contract/codes.toml`, `__design__/architecture.md` and a few single-file greps. I did not build or run anything. "Unknown" means I found no evidence either way.

Paths below are relative to the Candler repo unless prefixed by `HTN:` (= `/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion`).

---

## 1. Gotchas (ranked by harm to a consuming agent)

Class key: W = wrong result silently, S = spend or time burned, C = confusing refusal, D = doc gap.

### G1. A scored experiment can run against the wrong catalog or stratum and read as valid (W)
- Symptom: legacy HTN grading scored 13 of 26 campaigns against the wrong Tier D catalog; 181 of 181 rikyu runs showed D-E5 fail. An agent sees "fail", not "wrong instrument".
- Cause: default `servers/bio-opentrons` catalog, no `--servers-dir` on D-E entries. Source: `HTN:__reports__/candler_core/02-findings_cli_defaults_v0.md` (H1), `02-findings_lane_overlap_v0.md` (Contradictions).
- Live on main: partly. Candler's design derives the catalog per run (M2c declared inputs `[[inputs]] kind,path,at,audience`; `__reports__/m2c-wire-alignment/00-knowledge_transfer_v0.md`). Tier D (D-E1..D-E6, 1,440 rows) is still unimplemented: the registry refuses a `snapshots` or `inputs` need with "unsupported in this version" (`__reports__/m2a-evaluation-parity/02-knowledge_transfer_v0.md`, follow-ups; `__design__/architecture.md` Future features "snapshots[] and inputs[]").
- Prevention: a refusal code already exists for the need; M7 skill must say "Tier D evals cannot be authored yet". Tool: `resolve` output should list the catalog source per run.

### G2. Comparing two skill versions or arms: every stratum comparison depends on what is recorded (W)
- Symptom: `analyze` prints nothing between two treatments, or refuses a stratum; imported golden runs "refused as one stratum".
- Cause: A7/M6a made `request.treatment` a declared factor. A factor that varies a setting used to be refused (`experiment.setting_as_factor`), and the importer originally declared three aliases of one variable (M6a A8/A10). Source: `__reports__/candler-v1-review/03-findings_amendment_a7_v0.md`, `__reports__/m6a-paired-stats/knowledge_transfer_v0.md` (Amendments, Lessons).
- Live on main: fixed for treatment-as-factor and for imported strata (`crates/candler-core/src/manifest/compare.rs:577-619`, `imported_refused` at 472; M4 shipped "comparison verdicts for imported runs"). Residuals: a stratum whose arms differ in `eval_infra.commit`, `dirty` or `sandbox_contract` is still refused (`compare.rs:396` region, D8).
- Prevention: doc sentence in the "compare skill versions" skill: "one experiment file, treatment as a factor, same eval_infra; two files are two strata with no test between them". Computed default: `analyze` should print the refusal path (`analysis.*` codes in `docs/contract/index.md:47-49`).

### G3. `analyze` follows recording order, tied weights make the weighted test conservative, reading is judged before Holm (W)
- Symptom: sign of B - A flips with the order levels were recorded (`v0.2.3` sorts after `0.2.6`); a "reading: effect" where Holm does not reject; weighted outcomes under-detect real effects (rate about half of alpha with tied weights).
- Cause: M6a follow-ups F2, F3, F4. Source: `__reports__/m6a-paired-stats/knowledge_transfer_v0.md` "Open follow-ups".
- Live on main: probably live (M6a closed at 5199afc7, no later commit lists a fix; `analysis/report.rs:365-370` still has `effect`/`inconclusive` as the reading). F5 (JSON only) looks closed: `crates/candler/src/commands/analyze.rs:2,54-58` has `--format json|csv` with `--table`. F1 (table-level factors refused by `check_factors`, `analysis/report.rs:913`) unknown, likely live.
- Prevention: a consumer skill must tell the agent to read the Holm-adjusted decision, not `reading`, and to name level order; better a computed default (orient by declared level order in the experiment file).

### G4. RiVault serves identical requests from cache; replicates can be duplicates and `cost_usd` counts cache hits as paid (W, S)
- Symptom: replicate 2 equals replicate 1; cost non-zero on a cache hit.
- Cause: provider limit. Only the body field `"cache": {"no-cache": true}` bypasses it; `Cache-Control` is ignored. Eliott ruled Candler does nothing (M2b KT, For M4 item 2: `__reports__/m2b-execution-parity/04-knowledge_transfer_v0.md`).
- Live on main: yes, by ruling.
- Prevention: doc sentence in the "run on RiVault" skill ("at temperature 1 replicates may still be identical; check run distinctness"); a tool could flag runs with identical traces in one cell.

### G5. A treatment repository without `.claude/skills/<name>` fails every run in 0.1 s with `isolation_failed` and no message naming the path (C, S)
- Symptom: three `run.failed isolation_failed` within 0.1 s, `skill-archive` exits 128. Source: `__reports__/m4-session-runner/live_confirmation_v0.md` F3.
- Live on main: probably live: `docs/contract/index.md` and `codes.toml` have no code that names a missing skill path (grep for "skill" finds only HTN notes). Also a skill stored outside `.claude/skills/<name>` is unsupported (M2b KT, owned by M5).
- Prevention: a resolve-time refusal (new code, side `experiment`, path `/treatment/...`). Until then, a doc sentence.

### G6. Docker Desktop file share: a scratch root under `/private/tmp` is invisible to bind mounts; refusal text names no cause (C, S)
- Symptom: `jobs.session_controls_failed` seven jobs in a row; `docker run` exit 125, "bind source path does not exist: /host_mnt/private/tmp/...". About 25 minutes lost. Source: `__reports__/m4-session-runner/live_confirmation_v0.md` F4; KT `m4-session-runner/knowledge_transfer_v0.md`.
- Live on main: unknown. The code text in `docs/contract/index.md:170` still reads "No model's behaviour causes this" and does not name the control run's invalid reason or stderr tail (the open question F4 of the M4 KT).
- Prevention: refusal `causes[]` should carry the control run's invalid reason and stderr tail; doc sentence "use a store/scratch root under $HOME on macOS"; a `resolve` check that runs a `--mount type=bind` probe on the store root.

### G7. Plug-in author traps in `candler-evaluator` / the evaluator protocol (C, D)
Each is from the helper and contract pages; most are now documented or refused. Source: `__reports__/m3-lean-cli/leaves/docs_refusals_v0.md`, `__reports__/m2a-evaluation-parity/02-knowledge_transfer_v0.md` (follow-ups).
- Wrapping the plug-in command in `uv run` is wrong: Candler activates the environment itself (M2a docs follow-up).
- The environment kind `uv` needs both `project` and `lock` (`plugin.manifest_invalid`, path `/environment`); `plugin.toml` and `candler-plugin.toml` together give `plugin.manifest_ambiguous`.
- `command` runs in the plug-in directory.
- A bad outcome (return `7`, an unknown key, `blocked` with no cause, `pass` with weight 1/2) becomes `not_measured` with cause `instrument_error`, silently costing that item, not an error in the run. The note carries the reason.
- `print` goes to stderr; stdout carries only frames. A module with no `KINDS` exits 1 with a `PluginError` traceback and no frame.
- `INSTRUMENTS = ["pydantic"]` without `direct_url.json` fails `describe` (error -32000).
- Helper placeholders: `role` is `descriptive` and both digests are 64 zeros until the engine fills them.
- Items run with a per-item deadline (SIGALRM), so a hung kind costs one expectation.
- Method names and `-32800` are in `evaluator-request-side.schema.json`; the result-to-method table is in design R01 only.
- Live: documented in `docs/contract/*.md` for the cases above. The M7 skill should link the pages, not restate.
- Prevention: a tool `validate_plugin` that runs `describe` and the suite check without starting a job.

### G8. Refusal codes you cannot reach or that print a different code than the page lists (C, D)
- Symptom: a factor with no level, `replicates = 0`, a path overlap or an empty `any_of` print `experiment.document_invalid` or `suite.document_invalid`, never the named `factor_without_levels`, `count_zero`, `path_overlap`, `list_empty`; `format.*` codes never print for the three author documents; an unknown key under `[budget]` reports path `/foo`, not `/budget/foo`.
- Cause: the document check wraps the inner reason. Source: `__reports__/m3-lean-cli/leaves/docs_refusals_v0.md` ("Spec defects"); `docs/contract/experiment-file.md` Notes.
- Live on main: yes (the Notes document it).
- Prevention: an agent must match on `side` plus `path`, not on the specific inner code; the skill must say so. A real fix: let the document code carry the inner code in `causes[]`.

### G9. The store root: verbs without `--store` fall back to a default; `ps`/`kill` in another project see another store (C)
- Symptom: `candler ps` finds nothing, or finds another project's jobs. Source: M2b KT, "Owned elsewhere" N3 (`candler ps` needs `XDG_STATE_HOME`/`CANDLER_STORE_ROOT`). The cwd store becomes `<cwd>/.candler/store` once a `candler.toml` exists (`__reports__/m3-lean-cli/leaves/cli_surface_v0.md` spec defect 8).
- Live on main: partly fixed. R01 now says every verb reads one store root: `--store`, else `CANDLER_STORE`, else `store.root`, else default; "a verb that finds none refuses naming the three ways" (`__design__/architecture.md` after line 316). But `docs/contract/index.md:70` says `config.no_store` is "Not raised today": a missing store takes the default store, and a missing one prints `store.not_a_store`. So a typo'd project dir quietly uses the default.
- Prevention: pass `--store` explicitly from the MCP server, or make the tool return the store root it used in every result.

### G10. Fixed-value traps in the experiment file (C)
- A path in `[fixed]` that is a setting is out of the cell (A1 `settings_out_of_cell`); a factor that varies a setting is refused (`SettingAsFactor`); a stored experiment that wrote a setting in `[fixed]` got new cell digests (M2c KT, "Behaviour changes without a breaking marker").
- `isolation.image` sits in `comparable.values` as a machine-local docker id, so two machines never share a cell digest (M2c KT Decisions; M4 KT open question; Future features "isolation.image content identity").
- `runner`/`session` mode is opt-in; session close rule closes about 60 s after the last turn (M4 KT, live confirmation).
- A TOML date among values is refused (`experiment.value_unrepresentable`); `--set` accepts unknown `request.*` keys (M2c KT follow-ups); `candler run` reports a factor refusal as `Failed` after resolving the image and starting the sidecar, where other planning errors are `Refused` (M2c KT).
- Live on main: all unknown to likely live, except `--error-format json` (G11).

### G11. Output shape for agents: stderr text by default, JSON only with `--error-format json`; exit codes 0/1/2 only (D)
- Symptom: an agent behind a pty gets text, not a `refusal/1` record; usage errors (clap) print text unless `--error-format json`.
- Cause: design decision; `--error-format` is global and is neither derived nor a file setting (`__design__/architecture.md` after line 316; `__reports__/m3-lean-cli/leaves/cli_surface_v0.md`).
- Live on main: yes.
- Prevention: the MCP server always sets `--error-format json`; a skill sentence "always pass `--error-format json`".

### G12. `ingest` (alias `import`) takes a legacy folder only with `--rules`, and a run is recognised by root markers (C)
- Symptom: a raw trace directory with an unrelated `outputs/` was refused; a results folder is recognised only if `PROVENANCE.json` and `expectations.jsonl` sit at the path given. A rules file that does not parse is `config.import_rules_invalid` with no `path` (TOML spans). Source: `__reports__/m3-lean-cli/leaves/ingest_inspect_v0.md`.
- The importer needs `entry_params` in the rules after M2c; the legacy mapping needs `tier`, `scale`, `text`, and a domain without `tier` is refused (M1 KT; M2c KT).
- Imported and live digests never match: join on `(run, expectation_id, evaluator@version)` via `import.requests[]` (M1 KT, "For M2a").
- A legacy run with no catalog line counts as clean; `OutcomeWriter::commit` never replaces a final file (`OutcomeExists`): re-grading at the same evaluator version is refused, a new version gets its own directory (M1 KT).
- The importer holds the whole folder in memory (about 279 MB for the golden corpus; M1 KT follow-ups). Import is about 33 ms per run and repeats to the same store safely.
- Live on main: yes for all unless stated.

### G13. Re-grade surprises (W)
- `diff --pending` does not regrade a run after its suite entry's params change; `show` without `--rates` and `inspect` count every current record (two counts) (Future features "Two counts and a regrade gap").
- A dependent in the same request still costs its own item (M2a follow-ups).
- A kind's `prior[]` also holds the records of earlier siblings of the same request (M2a follow-ups, wire note).
- `evaluate` with no selection grades only runs lacking a complete outcome set for the suite's evaluator versions. Two concurrent evaluations can grade the same run twice (no per-run claim; evaluate_verb follow-ups).
- Live on main: yes (listed as Future features).

### G14. Jobs and processes: detached supervisor, no daemon (C)
- A job is a detached supervisor; `ps` reports `lost` when its lock is free and no `job.ended` exists. Ctrl-C is swallowed while `follow` awaits recovery or `detach`; SIGTERM or SIGHUP of the CLI leaves the job running; a failed `detach` leaves a stale job dir (evaluate_verb follow-ups; R01 §Job).
- A job cap crossed at setup ends with zero runs, correct. Tokens/cost caps only see ended runs (the job meter is fed after the seal), so a live run can overshoot: live metering is M4's, status unknown (M2b KT "For M4" item 1).
- A copied `jobs/` directory of a live job would read as Lost and sweep its containers (no store-id label) (M2b KT item 5).
- Never `kill`/`pkill` yourself; use `candler kill <id>|--all -y`. `kill` without `-y` refuses.
- A sweep refuses an orphan started in the same tick as the supervisor (CLK_TCK 100); `b33_setsid_descendant_dies` flaked once on Linux CI (M2c KT). M3 shipped `stray_sweep_tick`; status unknown.
- Live on main: yes except where noted.

### G15. Isolation facts an experiment author trips over (C, W)
- The agent container is read-only, `--cap-drop ALL`, env built from nothing; HOME is `/home/agent`, workspace `/work`. The sealed run dir keeps `workspace/` and `home/`. After a run the host copy of a mounted input is a 0-byte stub (N4).
- `sandbox_contract` is `candler-container@1`, not `v<n>`; `planner.runtime.*` is absent. `provenance_fields` in the legacy feature map fails by design (M2b KT).
- `isolation.egress` is enforced by an SNI proxy; a host behind a shared CDN front is reachable under another name (domain fronting); server-first protocols stall; the proxy IP is read once and cannot restart inside a job (R01 §egress, Future features).
- `isolation.ground_truth_exposed` refuses a build where evaluator-only bytes are also in what the agent gets (`docs/contract/index.md:139`). The refusal is digest-based: a transformed copy and a symlink escape are not caught (Future features).
- A second harness shares `isolation.build.*`; the env image tag was once keyed only by the lock (fixed 5982a0c).
- OpenCode: pinned to 2.2.7 exactly; `codemode` is a cell value (treatment factor); `bypassPermissions` is the only mode; `steps` caps; the live reader emits no events (tool calls come from the export).
- Live on main: yes.

### G16. Contract pages and refusal registry are the only agent-facing truth; R01 and README have lagged (D)
- Symptom: an agent trusting `README.md` or R01 "Nothing is implemented yet" (v1-review R row).
- Cause: doc drift (`__reports__/candler-v1-review/00-findings_v0.md` table C, R). Fixed in part: `docs/contract/index.md` is generated from schemas and `codes.toml` (append-only registry; released codes never renamed).
- Prevention: the MCP server should serve `docs/contract/*.md` as resources and tool errors as `refusal/1`; skills link, never restate.

### G17. Compatibility is pre-1.0 and has no users: breaking changes may land without a `!` marker (D)
- Source: memory `no-external-users-escalate-compat`; M2c KT (four unmarked behaviour changes); amendment A6 ("stays 0.x through long dogfooding").
- Prevention: the skill should pin core and schemas versions (`docs/contract/index.md` compatibility table) and check them at the start of a session.

---

## 2. Usage patterns (as observed)

### P1. Probe run, then scored run
1. `candler resolve <experiment>`: prints every resolved value with its source and runs the preflight (version probe, gateway reachability, image id); starts no job. The legacy HTN used `--dry-run` for this in 69 of 71 `run-config` calls (`HTN:.../02-findings_cli_defaults_v0.md`), and the legacy dry-run proved almost nothing (H2).
2. Run 1-2 cells at a model chosen by the agent (RiVault/Reasoning-Small is acceptable for reachability-only runs; memory `rivault-runs-no-ask`, `reasoning-small-not-golden`); name the question the run answers first.
3. `candler run <experiment> --set PATH=VALUE`; read job id; `candler watch <id>`; `candler inspect <job id>`.
4. Check `invalid` reasons (`input_leak`, `infrastructure`, `interrupted`); read `ended_by`.
5. Scored run: full experiment file, same shape, `analyze`.
Evidence: m2b equivalence smokes, M4 live confirmation (about 0.19 USD for 4 runs), M5b live smoke, M2c smoke v2 (2 runs, 0.31 USD).

### P2. Import the golden corpus (or any legacy results folder)
1. Pin the reference: `htn-creator-eval@0.4.2` (sha256 `ca7f12af...` of `eval.json`), commit `28be2c15`, `PROVENANCE.json` `pinned`. The artifact repo's `harness/evals/eval.json` is older (memory `golden-suite-source`).
2. `candler ingest <folder> --rules <rules.toml>` (rules need `entry_params`; the plug-in ships `HTN:.claude/skills/htn-creator-workspace/candler-plugin/import/htn-results-rules.toml`). The run completes in the caller; re-running is safe.
3. Expect 240 runs, 10,320 records, 348 usage diffs, 65 tree diffs, 25 skipped files, each explained; 56 `incomplete` runs are valid.
4. `candler evaluate <suite> --runs <glob>` with the live plug-in, then `candler diff` against the import: 8,671 unchanged, 209 excused, 0 changed, 1,440 uncompared (Tier D).
5. Treat golden as the shape of an experiment, not numbers to match (memory `golden-runs-prove-the-design`).

### P3. Author a plug-in (evaluator)
1. Write `candler-plugin.toml` (`schema = "plugin-manifest/1"`, `command`, `[protocol] major=1`, `[environment] kind="uv" project lock`) plus a Python package using `candler-evaluator` (kinds as pure functions `(params, context, prior) -> outcome`).
2. Write the suite (TOML or JSON; the suite is checked before the plug-in describes itself; `source` is free).
3. List plug-in dirs in `candler.toml` (`plugins.dirs`); `candler resolve <suite>`.
4. `candler evaluate <suite>`; read `evaluation.planned {outcomes}` and `evaluation.outcome` events.
5. Lock hygiene: before tagging, `uv lock --upgrade-package candler-evaluator` and verify the pinned commit; the lock must not point at a local checkout when published (M2a KT). Real consumer: `HTN:.claude/skills/htn-creator-workspace/candler-plugin/` (0.5.3), with `deltas.toml` documenting every deliberate divergence.

### P4. Compare skill versions across models (RiVault)
1. One experiment file with factors `model.id` x `request.treatment` (a table level `{name, tag, source}`), `replicates`, `[budget]`, `isolation.build.plugin = "<plugin>/<env>"`, `[[inputs]]`.
2. `resolve`, then `run`; each cell resolves its own treatment, lock and image.
3. `evaluate`, then `analyze --experiment <file> [--evaluator name@version] [--format json|csv]`: every declared factor's level pair, blocked by the other factors, Holm per comparison per role, `instrument_error` per arm.
4. Read the Holm-adjusted decision (G3); read `instrument_error` before believing a difference.

### P5. Check that MCP servers/tools are reachable by the agent (census, catalog)
- The census (`census.init_tools`) records the tool catalog the agent actually had; the stratum verdict refuses arms that differ in it (M2b KT "For M4" item 7). Catalog snapshots are `tool-catalog-snapshot/1` (MCP `tools/list` shape).
- Agent-side MCP servers come from the experiment (`mcp.servers.<key>.command/args`, one input per server file) or the plug-in's `agent_environments.<name>.values`; M5b live smoke found a Claude Code run that saw no server because `.mcp.json` was never written (fixed 591b2e78).
- Catalog capture of input-delivered servers is still a Future features item.

### P6. Run an evaluation of a raw trace (no run)
`candler ingest <trace dir>...` then `evaluate`, `show`, `inspect` (the registry's drivers recognise the stream). `ImportRecord::raw` makes `base_commit` and `legacy_integrity` optional (m3 P2 branch B).

### P7. Resume, stop, clean up
- `candler ps`, `candler kill <id> -y`, resume replays stored values and records deviations if forced; an attempt without `run.done` becomes `invalid(interrupted)`; nothing is deleted.
- Containers are destroyed only through the guard; an agent never runs `docker rm`, `kill`, `pkill`.

---

## 3. Repetitive chores that code should do (seen in reports)

1. Re-pin the plug-in's helper before a tag: `uv lock --upgrade-package candler-evaluator`, then `git merge-base --is-ancestor` check (M2a KT). Make a `candler plugin check` (or CI) do it.
2. Confirm the golden suite source by sha256 before translating it (M1 KT; memory `golden-suite-source`). A script check against `PROVENANCE.json`.
3. Run the SPAWN pre-check (`git diff --quiet ... && cargo clean -p candler-core && ... guard_signal`) before any process-spawning run (M1, M2a, M2b, M2c, M3 Gotchas). Developer-side but it leaks to agents that run tests.
4. Compute and compare digests: "a digest that moves is a defect to report, never a value to bless" - agents diffed main-vs-branch stores by hand (`__reports__/m2c-wire-alignment/digest_invariance_v1.md`, `m4-session-runner/digest_invariance_v0.md`). A `candler diff-stores` tool would do it.
5. Recompute provenance fields and join imported vs live outcomes on `(run, expectation_id, evaluator@version)` (M1 KT).
6. Cleanup of Docker objects by exact id, with before/after lists (M4 KT). A `candler gc --job <id>` or per-job label listing.
7. Check CI of `main` after every merge, roadmap-only included (M6a KT). Developer-side.
8. Priced spend per session from transcripts (`m5b-spend.py`, `categorize.py`; `__reports__/agent-token-costs/`). A tool.
9. `git fetch` before reading `main` for versions and ancestry; release-plz dry run for new crates (M4 KT). Developer-side.
10. Pick a store root under `$HOME` on macOS and run a `--mount type=bind` check on a fresh subdirectory before the first run (M4 KT). A `resolve` check.
11. Check refusal by `side` and `path`, not by prose; always pass `--error-format json`.
12. Hand-written Amendment Log and Progress rows in roadmaps (`dirtree-rdm` has no command) (M6a KT). Developer-side.

---

## 4. Findings I could not close

- Whether F3 (missing `.claude/skills/<name>`), F4 (control-gate refusal cause) and M6a F1/F2/F4 were fixed after the cited reports: I found no evidence of a fix, so I marked them "probably live".
- `__roadmap__/candler-v1/.../m7_mcp_server.md` says tool shapes are "open in R01"; the ids-not-dumps and budget rules are I13 (`__design__/architecture.md`, R01 §Operation API, §Budget).
- The HTN side's only direct Candler drive today is the plug-in plus the import rules and the planned Phase 8 (`HTN:__reports__/candler_core/04-architecture_v3.md`: Phase 8 waits for M5, now done). I found no HTN script that calls the Candler CLI by itself.
- Plan-mode and spend rules in the memory folder (`milestone-planning-is-a-plan-mode-session`, `measure-spend-from-transcripts`, `agents-read-code-by-colgrep-only`) concern Candler's developers, not a consumer, and were not turned into gotchas.
