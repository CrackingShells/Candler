# Interlude audit: findings by slice

The auditors' findings, condensed, at aeb2a31. Line numbers are theirs; check them before acting. The ranking and the decisions are in [00-findings_audit_v0.md](00-findings_audit_v0.md).

## Slice A

```text
A slice (isolation, supervise, jobs) — condensed:
A1 M docker CLI fan-out: ps_by_label list + inspect-each serial (docker.rs:577-603); destroy = inspect,stop,inspect,rm (guard.rs:257-298) + more inspects (container/mod.rs:430,513); stop path lists job-wide 3x (stop.rs:109,173; sweeper.rs:197). ~10 spawns/run natural, ~37 per stopped run at N=9. Fix: one multi-id inspect filtered at daemon by job+run labels; pass inspect into guard.destroy; skip stop when not running.
A2 S-M child records never removed (supervise/record.rs:23-36, 2 fsyncs each); children() parses all on every verify/sweep (dir.rs:362-374, guard.rs:446, sweeper.rs:264); 240-run job ≈115k JSON reads; recovery reaps every historic record. Fix: delete record in Child::wait when group empty; check_record opens by id.
A3 M Linux /proc full scans every 250 ms (runtime/session.rs:42,218-240; sweep.rs:146-163) + marker sweep on every exit incl GroupOnly (stop.rs:403,654-681); ~7 reads/pid incl environ (signal.rs:181-200). ~14k reads/s at 500 procs. Fix: SIGCHLD-driven, direct children only, stat only, OnceLock boot_id, skip sweep when group empty.
A4 S liveness parses whole event log for every job at every job start (lost.rs:40-62, events.rs:151-174, runtime/session.rs:257-297); ps same; LostProof::ended twice per destroy; detach re-parses per poll. Fix: backward last-line read reusing last_complete_line (events.rs:200-232). (Overlaps C6.)
A5 L five docker-invocation owners (docker::Runner, gateway::DockerCli, run.rs docker_binary + CLIENT_ENV dup, image.rs, controls.rs) and six hardening lists with different limits (run.rs:89-109, 235-258; image.rs:1011-1050, 917-966; controls.rs:825-868; gateway.rs:608-660); CONTROL_ROLE twice. Fix: one client + one ControlContainer builder.
A6 S correction: gateway log capture keeps first 4 MiB, drops newest lines (stop.rs:946-956; gateway.rs:914-928); limit defined twice (gateway CAPTURE_LIMIT vs docker OUTPUT_LIMIT). Fix: fold read into read_since; one limit.
A7 S every control/collect container gets anonymous volume dst=/opt/env → full venv copy (run.rs:93-96; exec.rs:68; catalog.rs:249). Fix: volume only in run_argv.
A8 M cidfile 100 ms polling (runner/exec/mod.rs:355-377) + fallbacks; use docker create → id, then start -a.
A9 M sleep-polling: controls wait_for 200 ms exec spawns; gateway wait_ready up to 40 exec spawns; watch_caps 25 ms wake for job life (schedule.rs:169-179); slots 50 ms poll; JobLock std::thread::sleep in async (lock.rs:73-87) blocks tokio worker; detach polls full log.
A10 M duplicate validation: mounts() canonicalize x3; full-id check x2; env-merge loop x3; agent_record re-checks labels; ps_by_label rejects own selectors.
Sound: bounded output pumps; event-append last-line read; build contexts cleaned; JoinSet scheduler; Child::wait; image flock in spawn_blocking.
```

## Slice B

```text
B slice (evaluation, runner, resolve, import, analysis, store...) — condensed:
B1 M plan rescans whole store 3x (runner/plan.rs:462/878, :829/948, :951) + linear scan per cell×replicate (:955-960) + next_attempt fresh runs() per attempt (store/run.rs:273, 322-333); overflow re-check (:977-981) defensive. 10k-run store × 240 attempts ≈ 2.4M stats. Fix: list once into BTreeMap<(cell,rep),Vec<(attempt,done)>>; delete next_attempt.
B2 S every pending eval batch busy-polls slot files 50 ms (engine/mod.rs:467 join_all; engine/run.rs:86; jobs/slots.rs:196-205) sync flock on tokio workers; ~170k syscalls/s for 240-run regrade; no FIFO. Fix: tokio Semaphore(S) in front. (= A9 slots item)
B3 M store index write-only cache (store/index.rs 199 lines; fsync per seal/commit) read only by two divergent index_is_current (import/raw.rs:880, assemble.rs:559) that fail after any parallel job → every ingest rewrites index. Fix: delete index or make it runs() source.
B4 M three sealing pipelines disagree on reader failure (runner seal.rs:66; raw.rs:691-780 reseals invalid; assemble.rs:323-394 seals Valid w/o trace.json + outcomes); dup helpers discard_replaced, remove_tree, 3x store_error, io_error. Fix: one seal_ingested.
B5 M SHA-256→Digest hand-rolled 8x in prod (+~10 tests), fallible for impossible errors (dead variants e.g. RuntimeError::SpecDigest); re-hash in put_blob and RunWriter::raw. Fix: Digest::of_bytes/of_reader/From<Sha256> in schemas.
B6 M ~20 fsyncs/run, manifest written twice; ~11 removable (≈2 min per 10k-run import).
B7 S materialise copies via io::copy (no clonefile) then re-hashes copy per batch per run (context/materialise.rs:116-160). Fix: fs::copy + digest from tree entry.
B8 M-L import holds whole legacy folder in RAM (read.rs:24-31; documented 379 MB peak for 240 runs); streams decompressed fully; FrameWriter writes plain JSONL then re-reads+gzips. Fix: stream.
B9 M outcome-of-(run,expectation) defined 3 ways; tally may double count under different digests (analysis/tally.rs:306-327; engine/run.rs:225 empty_digest); pending_runs by id only. NEEDS PROBE. Records cloned 3x; Report::build O(strata×records).
B10 M two diff schemes for Comparable (manifest/compare.rs:259-339 vs runner/plan.rs:649-735).
B11 S resolve/layers.rs:20-55 redefines ~13 constants from experiment/paths.rs; plan.rs:36-39 more.
B12 S runner clones trace 3 copies (runner/exec/mod.rs:716,753); seal_run sync on tokio worker.
Sound: normalise streams; stats via statrs; digests cached; outcome writer; blob layout; engine stage logic.
```

## Slice C

```text
C slice (CLI, harness, drivers, schemas, python) — key findings, condensed:
C1 M harness surface split between trait and CLI DriverEntry (drivers.rs:56-114); Registry::builtin rebuilt per stream (ingest.rs:184-208, process::exit in factory); supervise builds registry twice. Fix: trait owns skills/base_context/offline_streams; detect returns version; ReaderFactory takes harness name.
C2 M ~230 dup lines: render_prompt/is_key/scalar_text/nothing in both drivers; prompt values x3; claude argv built twice (driver.rs:291-339, session.rs:122-159); cell paths in 4 sources (core experiment/paths.rs, schemas execution.rs:489-504, both drivers). Fix: paths into schemas; typed Cell accessors in harness.
C3 M two hand-rolled tar readers (claude prepare.rs:171-327, opencode skills.rs:109-231), already drifted (pax); plain-segment check x4. Fix: one SkillTree in harness, one plain_segment in schemas primitives. ~-250 lines.
C4 M fallible constructors for constants -> error variants + refusal rows + silent drops (.ok() on opencode interrupt driver.rs:299; live_reader None). Fix: const fn validated ctors, infallible new().
C5 S preflight starts a docker control container to print constant JSON (preflight.rs:40; run.rs:171, resolve.rs:249). Fix: StaticCatalogProvider.
C6 M liveness classify->recover->reclassify x4 CLI verbs (watch/evaluate/kill/ps); ps parses every job's whole event log from offset 0 (ps.rs:128) and re-digests spec; serial recoveries. Fix: core lost::settle, JobEvents::progress, ended counts from JobEnded.
C7 S-M harness versions in 4+ places; Dockerfile CC_VERSION not tied to driver SUPPORTED; SUPPORTED parsed at runtime x3. Fix: driver source of truth, --build-arg.
C8 L settings table not single source (opencode table() vs settings() parallel, guarded by test); settings() run 3x per cell. Fix: one plan(cell) method.
C9 S FrameData::to_bytes copies every frame (raw.rs:299-305). Fix: as_bytes(&self)->&[u8].
C10 M analyze/inspect JSON built by hand with json!, ingest rules format in CLI; no schemas.
C11 M ~47 uncoded CLI refusals -> candler.internal; codes.toml parsed by 5+ separate readers in tests.
C12 S-M python plugin.py re-implements outcome rules (outcome.rs:58-90); no shared outcome vectors.
Minor: duplicate Silent sinks; multi-thread runtime per verb; <store>/assets/<digest> never GC'd; ls reads every manifest.
```

## Slice D

```text
D slice (tests, CI, scripts) — condensed:
D1 S Gates job: no cache; fmt 42s, clippy 47s, build 51s, nextest build 106s + run 93s, doc 12s; 283-371 s, critical path. cargo build step wasted (test-support self-dep recompiles). Fix: rust-cache/sccache GHA, fmt parallel job, delete Build. ~150-200 s/run.
D2 S-M gates rerun on PR `edited` (ci.yml:11): #22 ran 3x ≈14 runner-min; no docs paths filter; main gates rerun after PR. Fix: if action != edited on gates/helper; changes filter; (main skip optional).
D3 M 106 test binaries (100 integration files), test build 106 s; 43 recompile mod support; container-e2e compiles 74 binaries to run 2. Fix: one integration binary per crate (tests/it/main.rs); container-e2e --test filters.
D4 S container-e2e serial max-threads=1 (.config/nextest.toml:3-9) 200 s; only global sweeps need serial (b40_kill_all, b12_sweep_scope?); finished probe step container-e2e.yml:45-77 (answer stable since 2026-11-06) → delete; hard-coded 14 test names :93-100.
D5 M fixture builders duplicated: manifest JSON ×13 files, fn candler() ×11, uv sync env ×4, wait_for ×3, ~12 fake HarnessDrivers. Fix: shared dev crate. ~1.5-2.5k lines.
D6 S-M ~25 tests each uv sync --frozen into fresh tempdir + re-import 1.3 MB corpus; CI uv enable-cache false (ci.yml:42,70). guard_cli_evaluate 72 s, guard_engine 44.5 s. Fix: env once per run keyed by EnvironmentKey; uv cache on.
D7 S guard_session rows sequential 19.3 s; primitives.rs:1954 walks 1.1M code points through 2 validators 7.5 s; flaky timing tests (b33_setsid_descendant_dies runs 42973775444, 42977476940; e1_ctrl_c... 43130165299) forced full reruns.
D8 S public_surface.rs compile-only tests (core 776 lines, harness 341) "usage only, no behaviour asserted" → delete 1,117 lines.
D9 M hand-written source scanners (guard_layering_i22 326, guard_platform_seam 202, guard_assert_clean 47, ...) → clippy disallowed-methods; candler crate has no lib → tests #[path]-include src. ~600 lines.
D10 S-M golden guards (~1,850 ignored lines) run by nothing but roadmap leaves w/ absolute paths; gate list in AGENTS.md:86-92 vs ci.yml disagree. Fix: scripts/check.py gates|e2e|golden used by CI and people.
D11 M every test a guard (521/592 /// Guards:), no confirm/probe; E1 checked at 3 layers.
D12 S no timeout-minutes; nextest only container group terminates slow tests.
Estimate: 4.5-6k test lines removable (7-9%); CI PR critical path ~6 min → 2-3 min.
```
