# engine: leaf report (v0)

The implementer's report follows as it came back. The shas were rewritten when the coordinator rebased the branch onto 8c01358: 56449d2, 52ff7ed and 97e56fd. The rulings on its findings are in [l1/00-rulings_v0.md](../l1/00-rulings_v0.md) §engine. The leaf is merged on report, with no verifier.

## Report

Branch `m2a/engine` in `/Users/hacker/Documents/tmp/claude-worktrees/Candler/m2a-engine`, on top of `feat/m2a-evaluation-parity` fcdf0aa. Nothing pushed. Three commits:

- d8c50cd `feat(evaluation): plan items, applies_to and dependencies per run`
- b34872c `feat(evaluation): run requests concurrently and commit outcome files`
- 588820d `test(evaluation): guard planning, the digest seam and i4`

### Gates
- SPAWN, then `guard_signal` (4 pass) and `guard_engine` (3 pass), after `cargo clean -p candler-core` with a clean `src/`: PASS, as the step 3 consistency check.
- STD: fmt, workspace clippy `-D warnings`, workspace nextest (293 passed, 4 skipped), `cargo doc -D warnings`: PASS.
- Linux-target clippy (`x86_64-unknown-linux-gnu`, candler-core all targets): PASS.
- `cargo check --all-targets` at each of the three commits: PASS.
- SUBJ (`cog check fcdf0aa..HEAD`, subjects at most 72): PASS. `cog verify --file` on each message: PASS.
- OWNED: PASS. `public_surface.rs` changed only in the `engine_surface` block.
- SCAFFOLD: no `mod scaffold`; 3 tests and 3 `Guards:` lines in `guard_engine.rs`.
- Coordinator's rebase onto fcdf0aa: done. ENG1 to ENG3 pass against the new helper. Its pydantic validation accepted the records of `ok`, `echo`, `fail` (no weight) and `counter` (a `count` value). No synthetic or echo breakage; `python/` untouched.
- The rust-conventions reviewer agent was not run. Clippy with `-D warnings` is clean.

### Bites (each temporary, restored with `git checkout -- <file>`)
- ENG1: the dependency check disabled in `blocker` (run.rs) makes only ENG1 fail.
- ENG2: the context digest taken over a different projection (`{"artifacts": []}`) makes only ENG2 fail.
- ENG3: skipping runs that already hold an outcome file of the evaluator (a cache) makes only ENG3 fail.

### Findings (defects and deviations for L1; none implemented as a fix)
1. Spec conflict, step 1 item 2. The leaf says a dependent of a failed dependency is "written by the core without sending it" and also that a dependency chain shares one request. These cannot both hold when the dependency fails inside the request: the core cannot know the verdict before sending. The engine sends the whole chain, then replaces the dependent's record with `blocked {expectation: <dep>}` and discards the plug-in's record. Only dependents whose dependency verdict is already known (earlier stage, or `not_applicable`) are held back and never sent. A plug-in therefore still runs the dependent of an in-request failure, with its side effects and cost. ENG1 asserts the replaced record, not non-delivery.
2. `PlanError::Interleaved` is an addition. Same-evaluator dependency chains are one batch, so a chain that goes A, then another evaluator, then A again cannot be split into requests in order. The leaf names only cycles. It is a planning error naming the entries.
3. The frozen `EvaluateError` lost `NotBuilt` and gained `EnvironmentNotReady`, `Suite`, `Io`, `Glob`, `Plan`, `Describe` and `Task` (the enum is `non_exhaustive`). `engine::regrade(store, plugin, suite, runs, env_root, supervisor)` is an addition (concurrency 2, no deadline override).
4. The engine overrides `role`, `suite_entry_digest`, `context_digest` and `request_id` and the evaluator and instruments from its own plan and `describe`; only the verdict (status, value, weight, probability, evidence, cause) is kept from the helper. A kept record that does not build becomes `not_measured` with `instrument_error`.
5. Request deadline: the sum of the items' deadlines plus `REQUEST_DEADLINE_MARGIN` (30 s), or the flat `request_deadline` when given.
6. Fixture deviations from the leaf text (the helper cannot be edited):
   - The helper's synthetic plug-in has no `fail` kind, and the echo plug-in declares no needs, so it echoes an empty context. One fixture plug-in `engine` (`tests/fixtures/engine/kinds/engine_fixture.py`) reuses the helper's `ok` and `echo` and adds a `fail` kind, a `counter` kind and the parity needs on `echo`. This is one manifest, not separate synthetic, echo and counter manifests.
   - ENG1 has 5 entries, not 4: N1 (applies_to miss), F1 (fail) with dependent F2, P1 (pass) with dependent P2.
   - The plug-in directory is built at test time with a symlink `env` to `python/candler-evaluator`, so the manifest's `uv` project is the helper. The guard runs `uv sync --frozen` into `<env_root>/<digest>/.venv` and writes `ready` itself; it does not use `EnvironmentStep`. Each guard builds its own environment (about 7 s per test; it needs the uv cache or network on a first run).
7. The imported mini-corpus runs already carry legacy outcome files, so the guard reads only the evaluator `engine`'s files.
8. Not guarded by an ENG test, by design of the leaf: the cancel paths of the engine (cancel before create and before commit, a client `Cancelled`). They are implemented per the L1 rulings and covered only through the client's `guard_signal`.
9. Housekeeping: a stray empty `rite.rs` from step 2 was removed (folded into b34872c).

### Cleanup done
Deleted `python/candler-evaluator/.venv`, the `__pycache__` directories I created, and the worktree `target/` (2.1 GB). The guards' scratch (store, plug-in dir, environments) lives in per-test temp dirs. The engine removes its own `work-*` directory under the environment root on every call.

## Landing (coordinator, m2a/engine rebased onto 8c01358)

| Gate | Result |
|---|---|
| Rebase | clean onto the surfaces merge and the uv CI step; `cargo check --workspace --all-targets` passes |
| OWNED | PASS: only `engine/`, `guard_engine.rs`, `fixtures/engine/` and `public_surface.rs` |
| SUBJ | `cog check`: no errored commits |
| SPAWN | `guard_signal` 4 passed |
| STD | fmt, clippy, doc clean; nextest 297 passed, 4 skipped (ENG1-ENG3 included) |
| Linux clippy | clean |
| DRIFT | `schema_drift` 2 passed, schemas unchanged |
| SCAFFOLD | no `mod scaffold`; 3 tests, 3 `Guards:` lines |
