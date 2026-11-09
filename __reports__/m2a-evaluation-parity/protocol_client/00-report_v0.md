# protocol_client: leaf report (v0)

The implementer's first report follows as it came back. The rulings on its F1–F8 are in [l1/00-rulings_v0.md](../l1/00-rulings_v0.md) §protocol_client.

## First report

### 1. Commits
- 186cb1a feat(evaluator-protocol): run evaluator requests through the supervisor  (steps 1 and 2 together, see findings F6)
- 15d8de2 test(evaluator-protocol): guard the client against scripted stubs

### 2. Gates (all PASS)
- STD: fmt, clippy --workspace --all-targets -D warnings, nextest --workspace (271 passed, 3 skipped), doc -D warnings: PASS. Linux target clippy --tests: PASS.
- SUBJ: `cog check f5c2533..HEAD` "No errored commits"; subjects <= 72: PASS.
- OWNED: PASS with the regex's `fixtures/client/` moved before the closing group (see F1); the literal regex fails on the six stubs.
- SCAFFOLD: no `mod scaffold`; 2 tests = 2 `Guards:` lines in guard_protocol_client.rs: PASS. Only the `protocol_client_surface` block of public_surface.rs changed (one hunk).
- Fixture size: 5,563 bytes total (<= 12288), each stub <= 1.2 KB: PASS.
- SPAWN (+ guard_signal 4/4) run on clean src before the final test runs and before the workspace run: PASS. The bite runs were made on dirty src (SPAWN's `git diff --quiet` would skip), directly after a SPAWN on the same commit; no bite touched stopping code.
- `nextest -E 'binary(guard_protocol_client)'`: PASS (2 tests, about 6 s).

### 3. Guards
- PC1 `every_failure_path_of_an_evaluator_is_accounted_for`. Guards: R6, six stubs, hand-literal statuses; JSON-RPC 2.0 §5.1 and R01 deadline text. Recording fake Signaller on every row; also asserts whether a stop was signalled.
  - Bite (late result): replace the deadline arm in session.rs with `std::future::pending()` -> late.sh row FAILS (A1..A3 Reported instead of deadline); hang.sh fails first.
  - Bite (EOF read as deadline) -> unreported.sh row fails (deadline vs instrument_error).
  - Bite (cancel line not sent) -> cancel.sh row fails ("a stop was not expected but ... signalled [15, 15, 9]"). The first version of cancel.sh did NOT discriminate (stdin EOF made it exit quietly); I changed the stub to hang on anything but a cancel line, then re-bit.
  - Bite (stderr not drained) -> flood.sh row fails (deadline, not Reported; ~37 s).
  - Bite (partial-frame note removed) -> partial_frame.sh row fails.
- PC2 `no_evaluation_module_signals_a_process`. Guards: R6, I22. Oracle: assert_clean's planted-violation self-test. Bite: `// libc::kill(pid, 9)` line planted in client/mod.rs -> FAILS naming the file; restored with git checkout.
All bitten files restored with `git checkout -- <file>`; tree clean.

### 4. One-off records
- Wall clock: PC1 about 6 s, PC2 under 0.1 s. Workspace nextest 26.6 s.
- Extra checks run in a throwaway test (deleted, not committed): `describe` against an inline sh evaluator returns the DescribeResult; a caller cancel returns `ClientError::Cancelled`; a dropped watch sender reads as "not cancelled". These have no committed guard (R04 assigns none).

### 5. Findings and deviations
- F1 (spec defect, gate): the leaf's OWNED regex ends `...|fixtures/client/|public_surface\.rs)$)`; the `$` anchors `fixtures/client/`, so no stub file can match. Fixed regex: `^crates/candler-core/(src/evaluation/client/|tests/(guard_protocol_client\.rs|public_surface\.rs|fixtures/client/))`.
- F2 (spec defect, protocol): Step 1 says "Stdin carries one RpcRequest line then EOF", but Step 2 sends `cancel` on stdin, and R03 repeats "one RpcRequest then EOF". Both cannot hold. I followed R01 §Lifetime: stdin stays open after the request line; it is closed after the result, or after `cancel`. helper_runtime must read `cancel` while evaluating and exit on EOF. Please fix the leaf and R03 wording.
- F3 (spec gap): `ClientOutcome` "one outcome per item" cannot hold an `OutcomeRecord` for an unreported item: the client lacks role, evaluator and suite_entry_digest (`Item` has none). I return `ItemOutcome::{Reported(Box<OutcomeRecord>), Unreported(Unreported{expectation_id, reason: Deadline|InstrumentError, evidence})}`; the engine writes the `not_measured` record from `reason.code()` and the evidence. Engine leaf must be told.
- F4 (spec gap): types the leaf did not name are mine: `EvaluatorCommand` (argv, env, cwd, marker, run, record_dir), `Client<'a>` with `with_clock`, `with_graces(cancel, stop)`, `with_describe_deadline`; `ClientOutcome{request_id, items, notes, stderr, stop_fault}`. `describe` takes a 30 s default deadline (the signature has none). Cancel grace default 2 s, stop-ladder grace 5 s.
- F5 (PC2 vs harness spelling): the needle `Signal::` also matches `StopSignal::Term`, which `StopSpec::new` needs. I import it as `StopSignal as Termination` in client/mod.rs, with a comment that avoids the needle. A future client edit that names the enum plainly would trip PC2 without signalling anything.
- F6 (commit shape): steps 1 and 2 are one commit under step 1's subject; the session needs `Clock` and `account_items`, so a step-1-only commit would not compile with the spec'd API. No separate step 2 commit.
- F7 (ambiguity, decided): duplicate or unknown item id, a JSON line that is no protocol message, a partial frame, an oversize line: the offending record is dropped, noted in `ClientOutcome.notes`, and the request continues; items left unreported get `instrument_error` whose evidence names the violations. A non-JSON line is only counted (R01 §Stdout guard), where the leaf says it is `instrument_error`; every item it leaves unreported is `instrument_error` anyway. An unsolicited `-32800` (client did not cancel) gives `instrument_error` with evidence, not `Cancelled`. Say if you want another reading.
- F8: a process that survives the stop ladder (possible only with the fake Signaller) is reported in `stop_fault`; the item outcomes are still returned.
- No dependency or manifest change. `progress`/`log` notifications are read and dropped (forwarding into job events is not in the leaf).
- The plain verifier (the [nl] gate) is not yet dispatched; it is the coordinator's call.

## Plain verifier: findings and their fate
| Finding | Fate |
|---|---|
| Deadline ordering unguarded: putting the frame arm first survives | Fixed: `a_deadline_beats_an_answer_that_is_already_waiting` |
| Caller cancel unguarded | Fixed: a new PC1 row |
| `late.sh` flake margin of about 100 ms | Fixed: stubs sleep 10 s, and the hang and cancel deadlines are 2,000 ms |
| PC2's needles are weak | Fixed: `kill_process`, `Command::new("kill")`, and a planted self-test per needle |
| Duplicate or unknown expectation ids pass silently when no item is unreported | Moved to the engine (`engine.md` Step 2 item 2): such a request fails and commits nothing |
| The record's `request_id` is not checked | Moved to the engine: it sets `request_id` from its plan |
| A cancel during `settle` returns `Ok`, and dropping a `Child` does not stop it (notes) | Moved to the engine (`engine.md` Step 2 item 4) |

## Fix round (rebased onto 33189b3; `src/` unchanged)
- **Deadline ordering.** The suggested stub, a result at 0.9 s against a 0.4 s deadline, does not bite: the two select arms race only when both are ready at the same poll.
  - The new test runs on a current_thread runtime. `ready.sh` answers at 0.3 s against a 0.4 s deadline, while a task blocks the only thread from 0.1 s to 1.3 s.
  - Every item must be `deadline`, with no signal sent.
  - Bite: frame arm first. That fails this test only.
- **Caller cancel.**
  - The row flips the cancel watch at 300 ms against `hang.sh` and expects `Err(ClientError::Cancelled)`, a stop, and a `cancel` line received.
  - Every recorded signal must target the stub's pid.
  - Bite: the caller-cancel arm becomes `pending()`. The row fails.
  - Bite: a cancel line sent only on deadline. The row fails.
- **Flake.**
  - The stubs now sleep 10 s.
  - Under workspace load, `hang.sh` missed its 400 ms deadline in 7 of 10 parallel runs. The hang and cancel deadlines are now 2,000 ms, and 10 of 10 runs passed.
- **PC2.** Both new needles bite in a planted `client/mod.rs`.
- **Not bitten:** a `settle` that keeps late frames, for the in-grace `late.sh` row. That bite needs a source refactor.

## Landing (coordinator, rebased onto d659de5)
Rebased as `dd8551a`, `42f99ac`.

| Gate | Result |
|---|---|
| OWNED | PASS against `d659de5`. `public_surface.rs` changes only the `protocol_client_surface` block |
| SUBJ | `No errored commits`, subjects of 65 and 71 characters |
| SCAFFOLD | PASS: 3 tests, 3 `Guards:` lines, no scaffold marker |
| SPAWN | `guard_signal` 4/4, after `cargo clean -p candler-core`, uid 501 |
| STD | `289 tests run: 289 passed, 4 skipped`, with fmt, clippy and doc clean |
| Linux clippy | PASS |
| `guard_protocol_client` | 3/3, then 3/3 in each of three repeats (about 8.8 s each) |
| Fixtures | 6,844 bytes in all, largest stub `hang.sh` at 1,175 bytes (limit 2 KB each) |

The leaf landed as `e7b7dd6` `merge(evaluator-protocol): land protocol_client`.
