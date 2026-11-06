# Protocol client over supervise

**Goal**: `evaluation::client` runs one evaluator process per request through `supervise::Supervisor` and exchanges `rpc` messages on its stdio. It enforces the request deadline and cancel, and turns every item the evaluator never reports into `not_measured {instrument_error}`. Every item is accounted for, no `deadline` row comes from an item-level fault, and nothing in `evaluation` signals a process (PC1, PC2).
**Pre-conditions**:
- [ ] [cmd] ENV, DEP(skeleton) and DEP(protocol_wire)
**Success Gates**:
- ⬜ [cmd] STD, SUBJ and SCAFFOLD pass, and OWNED passes with `^crates/candler-core/(src/evaluation/client/|tests/(guard_protocol_client\.rs|fixtures/client/|public_surface\.rs)$)` with only the `protocol_client_surface` block of `public_surface.rs` changed
- ⬜ [cmd] SPAWN, then `cargo nextest run -p candler-core -E 'binary(guard_protocol_client)'` passes (PC1, PC2)
- ⬜ [cmd] `test "$(find crates/candler-core/tests/fixtures/client -type f -exec cat {} + | wc -c)" -le 12288` (bytes, not disk blocks) (six stubs, ≤ 2 KB each)
- ⬜ [nl] The plain verifier holds no open BLOCKER or MAJOR: late-result and cancel races, pipe deadlock
**References**: [R01 §Evaluator protocol, §Jobs, runs and supervision](../../../__design__/architecture.md) — deadlines, cancel, I14, I22; [R03 §Protocol](../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md); `crates/candler-core/src/supervise/` — `Supervisor::new`, `with_signaller`, `StopHandle`, `Child::wait`, `Child::stop`; `candler_schemas::evaluation::rpc`; [JSON-RPC 2.0 §5.1](https://www.jsonrpc.org/specification); [R04 PC1, PC2](../../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md); [R09 M1 KT](../../../__reports__/m1-contracts/01-knowledge_transfer_v0.md) — I22 and the recording fake `Signaller`

## Step 1: Run a request through the supervisor
**Goal**: One call spawns the evaluator, sends one request, collects its notifications and result, and always reaps the process.
**Implementation Logic**:
1. `Client::new(supervisor: &Supervisor)`; `Client::describe(command) -> Result<DescribeResult, ClientError>` and `Client::evaluate(command, params: EvaluateParams, deadline: Duration, cancel: watch::Receiver<bool>) -> Result<ClientOutcome, ClientError>`, where `ClientOutcome` holds one outcome per requested item in item order.
2. The request id is `RequestId::new_v7()`. Stdin carries one `RpcRequest` line then EOF. Stdout is read line by line as `rpc::Message`. Stderr is drained concurrently into a bounded buffer kept as evidence, so a flood never blocks the child.
3. Every outcome notification is matched to a requested item by id; a duplicate or unknown item id is an `instrument_error` for the request.
**Deliverables**: `crates/candler-core/src/evaluation/client/{mod,session,framing}.rs` (`Client`, `ClientOutcome`, `ClientError`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(evaluator-protocol): run evaluator requests through the supervisor`

## Step 2: Enforce the request deadline, cancel and unreported items
**Goal**: A request ends within its deadline, and every item ends with exactly one outcome.
**Implementation Logic**:
1. The deadline is measured on an injected clock (`trait Clock`, a design seam that stays; its scaffold tests do not). At the deadline the client sends `cancel`, waits a short grace, then stops the process with `Child::stop` or `StopHandle`, which are the only stop paths (I22). Items already reported keep their outcomes. Each unreported item gets `not_measured {reason: deadline}` with evidence naming the request deadline; this is the only path that writes `deadline`.
2. When the process exits or closes stdout before reporting every item, each missing item gets `not_measured {reason: instrument_error}`. A partial frame at EOF and an unparsable line are `instrument_error` too.
3. A result or outcome that arrives after the deadline fired is discarded: the deadline wins.
4. A `-32800` error response is read as "cancelled", never as an evaluator fault.
5. A caller cancel (`cancel` turns `true`) takes the deadline's path (`cancel`, grace, stop) and returns `ClientError::Cancelled`: the engine commits nothing for that request (R03 §Protocol).
**Deliverables**: `crates/candler-core/src/evaluation/client/{deadline,accounting}.rs` (`Clock`, `SystemClock`, `account_items`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(evaluator-protocol): account for every item under the deadline`

## Step 3: Guard the client against scripted stubs
**Goal**: PC1 decides every failure path with six hand-written stubs, and PC2 keeps signals out of the evaluation modules.
**Implementation Logic**:
1. `tests/fixtures/client/`: six POSIX `sh` stubs (≤ 2 KB each, hand-written): a stub that reports only some items then exits; one that hangs; one that answers `cancel` with `-32800`; one that sends its result after a delay longer than the deadline; one that writes a partial frame then exits; one that floods stderr with 4 MB before answering.
2. `tests/guard_protocol_client.rs` (`#![cfg(test)]`), with a `Supervisor` built with the recording fake `Signaller` (feature `test-support`) wherever a stop is exercised:
   - PC1 (R6): a table, one row per stub. Expected per-item statuses and causes are hand literals: an unreported item gives `not_measured {instrument_error}`, a request deadline gives `deadline`, cancel is answered, the late result loses, a partial frame gives `instrument_error`, and the stderr flood completes. Oracle: JSON-RPC 2.0 §5.1 text for `-32800` and R01's deadline text. Bite: accept the late result.
   - PC2 (R6, I22): `support::assert_clean` over `crates/candler-core/src/evaluation/` finds no `kill(`, `killpg` or `Signal::`. Oracle: the planted-violation self-test of `assert_clean`. Bite: plant `libc::kill` in a comment of `client/mod.rs`, then restore it with `git checkout -- <file>`.
3. Fill `protocol_client_surface` (the skeleton gated it `#[cfg(unix)]`, as the module is); delete scaffold tests, record bites, dispatch the plain verifier.
**Deliverables**: `crates/candler-core/tests/fixtures/client/*.sh`; `crates/candler-core/tests/guard_protocol_client.rs` (PC1, PC2); the `protocol_client_surface` block
**Consistency Checks**: `git diff --quiet HEAD -- crates/candler-core/src && cargo clean -p candler-core && cargo nextest run -p candler-core -E 'binary(guard_signal)' && cargo nextest run -p candler-core -E 'binary(guard_protocol_client)'` (expected: PASS)
**Commit**: `test(evaluator-protocol): guard the client against scripted stubs`
