# Registry: leaf report (v0)

The implementer's reports follow as they came back. The rulings on F1–F7 are in [l1/00-rulings_v0.md](../l1/00-rulings_v0.md). The registry is merged on report, without a verifier (R04).

## First report (after ruling A3)
1. Commits (m2a/registry on 40138a6):
 - 95c6a7a feat(registry): check suites against plug-in manifests
 - 35edff8 feat(registry): build plug-in environments once per lock digest
 - 85b5cb6 test(registry): guard the refusals and the environment reuse
 Fixups autosquashed; `cargo check --all-targets` passed on every commit.
2. Gates: SUBJ PASS (60/63/54 chars); OWNED PASS (only registry/, guard_registry.rs, fixtures/registry/, public_surface.rs; only registry_surface hunk changed); SCAFFOLD PASS (2 tests, 2 Guards lines, no scaffold); SPAWN PASS (guard_signal 4/4 after cargo clean, as uid 501); guard_registry 2/2 PASS; I22 PASS (guard_signal 4/4; git grep for Command|libc|kill|Signal under src/evaluation/registry: no hit; uv spawns only through Supervisor::run_captured); STD PASS (fmt OK; clippy -D warnings OK; nextest workspace 271 passed, 3 skipped; doc -D warnings OK); linux-target clippy --workspace --tests PASS; NOPUSH: origin has no feat/m2a-evaluation-parity.
3. Guards
 - REG1 the_registry_refuses_what_the_spec_refuses. Guards: R8 — valid base accepted; unknown kind, params failing the schema (pointer /pattern), glob outside outputs/, snapshots and inputs needs refused, the last two with R03's exact text. Oracle: spec text (R03). Bite: `if false && !needs.inputs.is_empty()` in needs.rs -> the inputs case panics ("inputs: CheckedSuite ..."); restored with git checkout.
 - REG2 a_second_preflight_under_one_digest_spawns_nothing. Guards: R6 — second preflight under one digest runs uv once total; env dir inode and mtime unchanged. Oracle: OS stat + the stub's counter file. Bite: `if false && try_exists(ready)` in environment.rs -> "the second preflight spawns nothing left: 2 right: 1"; restored with git checkout (SPAWN run before).
4. Records: none requested.
5. Findings / deviations
 - F1 (deviation, flagged): added `EnvironmentStep::for_plugin` and `EnvironmentStep::with_path(&str)`. REG2 needs a stub uv on PATH and the workspace forbids unsafe, so the test cannot set the process PATH; with_path overrides the forwarded PATH. REG2 therefore builds the step with for_plugin, not through register_evaluation_steps; that fn is only nameable-checked in registry_surface (it just maps for_plugin over the registry). Verifier may want a direct test.
 - F2: `--project` is `<plug-in dir>/<project>` and pyproject.toml is read from that directory, taking `project` as the uv project directory per the schema doc (the A3 text calls both "files"). Digest = sha256(lock bytes || pyproject bytes). A plug-in with environment kind none gets no step.
 - F3 (schema limitation, not fixed): RelPath refuses "." so a manifest cannot name the plug-in directory itself as the uv project; the pyproject must sit in a subdirectory (fixture uses env/). uv also looks for uv.lock inside the project dir, so `lock` must be <project>/uv.lock for --frozen to work; not enforced. htn_plugin should know.
 - F4: "unknown need" is unrepresentable (Needs is a closed struct); the unmeetable need is an artifact glob not under outputs/ (or with .. or an invalid glob). Metadata paths are always accepted (no manifest to check at suite time). Trace always accepted.
 - F5: check_suite does not check that the described version satisfies the entry's evaluator range; left to selection/engine.
 - F6: `#[allow(clippy::needless_pass_by_value)]` on register_evaluation_steps because the ruled signature takes Arc<Supervisor> by value.
 - F7: a failed `uv sync` yields a failed CheckResult (stderr up to 8 KiB), spawn errors yield PreflightError::Failed; sync has a 30 minute supervisor deadline (my choice). Child records go under <env root>/<digest>/records.

## Fix round (F5, after ruling A4)
- **Commits** on `a3b4f46`, then rebased by the coordinator onto `33189b3`: `6310998`, `982f816`, `835f928`. The subjects are unchanged.
- **Range check.** `RegistryError::VersionOutsideRange { entry, plugin, described, range }` is raised in `check_entry`, after the not-described check and before the kind lookup.
  - REG1 gained a row: the suite range `>=1.0.1` against the described version 1.0.0. The positive control keeps `^1`.
  - Bite: `if false && !range.matches(..)` in `suite.rs` makes REG1 fail at `guard_registry.rs:73`. Restored with `git checkout --`.
- **OWNED.** The leaf's regex anchored `fixtures/registry/` with `$`, so no fixture could match. The first report's PASS was wrong. The regex is fixed in `33189b3`.

## Landing (coordinator, on 33189b3)
| Gate | Result |
|---|---|
| OWNED (fixed regex) | PASS |
| SUBJ | `No errored commits`, with subjects of 54, 63 and 60 characters |
| SCAFFOLD | PASS: 2 tests, 2 `Guards:` lines |
| SPAWN | `guard_signal` 4/4, after `cargo clean -p candler-core`, uid 501 |
| STD | `271 tests run: 271 passed, 3 skipped`, with fmt, clippy and doc clean |
| Linux clippy | PASS |
| `guard_registry` | 2/2 |

The leaf landed as `3d31151` `merge(registry): land registry`.
