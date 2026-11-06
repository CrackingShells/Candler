# Skeleton: leaf report (v0)

The coordinator did this leaf itself, as R04 assigns it: there was no implementer, and an API review ran instead.

## Commits
| sha | subject |
|---|---|
| `94972bf` | `feat(evaluation): scaffold the evaluation modules and the evaluate seam` |
| `5549a37` | `feat(cli): scaffold the import, evaluate, show and diff commands` |
| `d68e943` | `test(evaluation): extend the layering scan to the binary and the helper` |
| `a0f5aaa` | `build(deps): add globset, tokio signal and tempfile for m2a leaves` |
| `f40a6b7` | `feat(evaluation): pass the supervisor, cancel and env root to evaluate` |
| `0b805ac` | `feat(cli): take a suite file, select an evaluator and share helpers` |
| `eb8c17e` | `fix(evaluation): gate the unix-only surface anchors and doc links` |

The last four apply the API review (Step 4 and the re-review). The leaf text and reports it changed are in `fa9c0cc`, `902832f`, `0b7d539` and `2ea8298`.

## Gates
| Gate | Result | Evidence |
|---|---|---|
| STD (after SPAWN, uid 501) | PASS | `266 tests run: 266 passed, 3 skipped`, then `cargo doc` clean |
| SUBJ | PASS | All subjects ≤ 72 characters. `cog check 921765c..HEAD`: `No errored commits` |
| OWNED | PASS | `git diff --name-only 921765c..HEAD` holds nothing outside the leaf regex |
| SCAFFOLD | PASS | No `mod scaffold` in the owned files. `guard_layering_i1.rs` has 3 tests and 3 `Guards:` lines. `public_surface.rs` is M1's usage-only file and carries no `Guards:` lines, as in M1 |
| NEUT | PASS | `guard_layering_i1` passes, with the bite below |
| Stub exits | PASS | `evaluate --help` exits 0. `diff --from a --to b --format json` exits 2 with `candler diff: not built` on stderr |
| Step checks | PASS | `cargo check --workspace --all-targets`; `cargo clippy -p candler-core --all-targets -D warnings`; `cargo clippy -p candler --all-targets -D warnings`; `show --help` |
| API review | see below | 12 findings and 4 notes; every finding fixed or moved into its leaf |

## Guards
- **NEUT** (`guard_layering_i1`, first test).
  - Guards line: `Guards: I1 — none of the deny-listed domain names appears in the core's sources, the harness crate's sources, the published schemas, the candler binary's sources or the Python evaluator helper's sources`.
  - Oracle: the deny-list literal.
  - The bite was run in both new folders:

    | Planted `rikyu` in | Exit | First failure line | Restored |
    |---|---|---|---|
    | `crates/candler/src/commands/mod.rs` | 100 | `"rikyu" found under …/candler/src: [".../commands/mod.rs"]` | `git checkout --`, clean |
    | `python/candler-evaluator/src/candler_evaluator/__init__.py` | 100 | `"rikyu" found under …/python/candler-evaluator/src: [".../__init__.py"]` | `git checkout --`, clean |

    After both restores the binary passes again (3/3).
  - `assert_clean` also asserts that each folder holds at least one file, so a wrong path cannot pass vacuously.
- The development test `engine::scaffold::the_stub_is_not_built` passed in Step 1 and was deleted in Step 3.

## Findings
1. **Spec defect in this leaf's gate, fixed by the coordinator.** The gate ran `diff --from a --to b; test $? -eq 2`. `--format` is required (R03 §Skeleton seams), so that command exits 2 on clap's usage error before the stub ever runs, and the gate passes vacuously. The gate now passes `--format json` and checks the `not built` line.
2. **Deviation: `run` is async.** The leaf says `commands::<name>::run(&cli, args) -> ExitCode`. The stubs are `async fn run(&Cli, &Args) -> ExitCode`: `main` builds the current-thread runtime once and `block_on`s `dispatch`, which awaits one arm per line. This lets `evaluate` await `engine::evaluate` without a second runtime. Each stub carries `#[allow(clippy::unused_async, reason = …)]`, which `surfaces` removes.
3. **`--plugin` is repeatable** (`Vec<PathBuf>`, required), to match `EvaluateRequest.plugins: Vec<PathBuf>`. `--concurrency` is `Option<NonZeroUsize>`. The default belongs to `surfaces` (R01 "Deliberately open": the slot count default).
4. `evaluate` is `pub async fn` with no await yet. It carries `#[allow(clippy::unused_async)]`, not `#[expect]`, because the lint does not fire under `cfg(test)`, where the scaffold awaited it. `engine` removes it.

## API review
A Sonnet reviewer read the seams against R03 §Skeleton seams, `engine.md`, `surfaces.md`, `selection_rates.md` and `m2b_execution_parity.md`. It ran `cargo check` and `clippy` and no tests. Q4 (module tree), Q5 (NEUT is not vacuous) and Q6 (lints) were fine. Each finding was resolved either in the skeleton itself, in `build(deps)` `a0f5aaa`, `f40a6b7` and `0b805ac`, or as behaviour in the leaf that owns it (`fa9c0cc`, `902832f`, `0b7d539`).

| # | Severity | Finding | Resolution |
|---|---|---|---|
| 1 | BLOCKER | `EvaluateRequest` has no env root, so the engine cannot test the `ready` marker | `env_root: PathBuf`. The rule `<state home>/candler/environments` is in R03 §Environments, and `registry::default_env_root` in registry.md |
| 2 | BLOCKER | `register_evaluation_steps(&[PluginManifest])` loses the plug-in directory | registry keeps `Plugin { dir, manifest }`; the step takes `&Registry` |
| 3 | MAJOR | Nothing maps `--suite <id>` to a file | `--suite <file>`; `EvaluateRequest.suite: PathBuf` |
| 4 | MAJOR | `evaluate` has no supervisor or cancel, so Ctrl-C orphans helper groups (I22) | `evaluate(request, Arc<Supervisor>, watch::Receiver<bool>)`. Caller cancel goes into engine, protocol_client and surfaces (`ctrl_c`). Tokio `signal` is enabled; the binary receives SIGINT and never sends a signal |
| 5 | MAJOR | `rates` wants a suite (`RoleMap`), and `show` cannot pick an evaluator | `RoleMap` dropped; roles come from `record.role()` (I21). `show --evaluator`, required by `--rates`, is in the README and golden gates |
| 6 | MAJOR | "Absent from the `to` suite" cannot be computed; a fully failed regrade reads as no change; `&Equivalence` cannot say `null` | A version's expectations are read from its records; `compared == 0` exits 2 (CLI1 row); `Option<&Equivalence>` |
| 7 | MAJOR | No `tempfile` dev-dependency, no glob crate, and no leaf owns a manifest | `build(deps)` adds `globset`, tokio `signal` and `tempfile`; the skeleton's OWNED covers the manifests |
| 8 | MINOR | The flat `request_deadline` contradicts R01's "sum plus margin" | `Option<Duration>`: `None` is the sum of item deadlines plus `REQUEST_DEADLINE_MARGIN` |
| 9 | MINOR | `EvaluateError` derives `Clone`, `PartialEq` and `Eq`, so it cannot wrap io or store errors | Only `Debug` and `Error` |
| 10 | MAJOR | The store root and `Resolver` composition have no home; four commands and M2b would duplicate them | `commands/common.rs`, in surfaces' OWNED |
| 11 | MINOR | The current-thread runtime stalls on blocking store writes, and M2b wants multi-thread | Multi-thread runtime; engine uses `spawn_blocking` |
| Q4 | MINOR | `registry` and `engine` need `supervise`, which is Unix-only | Both are `#[cfg(unix)]`; so is `protocol_client_surface` |
| — | spec text | "four folders"; `--runs <filter>`; registry's `test(scaffold)` check has no tests | "five folders"; `<glob>`; registry Step 1 has its own `mod scaffold` |

Notes accepted without change:
- **12.** The enum and match are append-only only if M2b lands after M2a. M2b also inserts into `commands/mod.rs` alphabetically.
- **Q5.** A stale `__pycache__` could give a NEUT false hit. A `.pyc` holds only text its source held, so this needs the cache of a bite, and bites are restored before PYSTD runs.
- **Step 1.4.** The reviewer could not find `mod scaffold`: it existed in `94972bf` and was deleted in `d68e943`, as specified.

The M2 coordinator was told about the shared CLI changes (runtime, `common.rs`, the signal feature, the new flags) before they were made.

**Re-review** (the same reviewer, over `27be63b..0b7d539`). It ran `cargo check`, workspace clippy, `cargo doc -p candler-core` and Linux clippy, and no tests. All eleven numbered findings are closed and no new BLOCKER or MAJOR was found. Its residuals, each fixed in `eb8c17e` (code) and `2ea8298` (leaf text):

| Residual | Severity | Fix |
|---|---|---|
| Q4 partly open: `registry_surface`, `protocol_client_surface` and `engine_surface` were not `#[cfg(unix)]` | MINOR | The skeleton gates all three anchors; the three leaves fill gated anchors |
| Step 4's Linux clippy cannot see a `cfg(unix)` gap, since both installed targets are Unix | MINOR | skeleton.md Step 4 item 5 says the gate guards the Linux build only; the gating is reviewed by reading |
| A dropped `watch::Sender` makes `changed()` return `Err` | NOTE | engine.md: `regrade` keeps the sender alive, and `Err` reads as "not cancelled" |
| The module doc linked `crate::supervise` and the Unix-only modules, which breaks `cargo doc` off Unix | NOTE | Plain code text |
| An expectation with no `to` record in any run stays `uncompared` | NOTE, kept | The engine writes a `not_measured` row per planned item, so only an unplanned expectation is uncompared |

The review closes with no open finding.
