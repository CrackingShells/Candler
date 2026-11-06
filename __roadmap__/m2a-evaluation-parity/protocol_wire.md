# Protocol wire: JSON-RPC envelope

**Goal**: `candler_schemas::evaluation::rpc` frames the existing request-side and notification roots as JSON-RPC 2.0 messages, `RequestId::new_v7` mints request ids, and one set of hand-written vector files decides the envelope for Rust (WIRE1, WIRE2) and later for Python (PYM1).
**Pre-conditions**:
- [ ] [cmd] ENV, and `cargo tree -p candler-schemas -e features -i uuid | grep -q 'uuid feature "v7"'`
- [ ] [nl] The M2 coordinator OK'd the additive schema root and `RequestId::new_v7` (proxy: the reply quoted in Amendment Log A1)
**Success Gates**:
- ⬜ [cmd] STD, DRIFT, SUBJ and SCAFFOLD pass, and OWNED passes with `^crates/candler-schemas/(src/(evaluation\.rs|evaluation/rpc\.rs|primitives\.rs)|schema/evaluator-message\.schema\.json|tests/(guard_rpc_wire\.rs|fixtures/rpc/))`
- ⬜ [cmd] Additive ratchet: `test -z "$(git diff --name-only --diff-filter=MDR origin/main -- crates/candler-schemas/schema)"`
- ⬜ [cmd] `cargo nextest run -p candler-schemas -E 'binary(guard_rpc_wire)'` passes (WIRE1, WIRE2)
- ⬜ [cmd] `test "$(find crates/candler-schemas/tests/fixtures/rpc -type f -exec cat {} + | wc -c)" -le 10240` (bytes, not disk blocks)
- ⬜ [nl] The report records the bites of WIRE1 (accept `"jsonrpc": "1.0"`) and WIRE2 (loosen the payload to any object), each tried and seen to fail
**References**: [R01 §Evaluator protocol](../../__design__/architecture.md) — methods, notifications, cancel `-32800`; [JSON-RPC 2.0](https://www.jsonrpc.org/specification) — §4 Request, §4.1 Notification, §5 Response, §5.1 Error; [R03 §Protocol](../../__reports__/m2a-evaluation-parity/00-architecture_v0.md) — one process per request, the envelope over the roots; `crates/candler-schemas/src/evaluation.rs` — `Request`, `Notification`, `RequestSide`, `DescribeResult`, `EvaluateResult`, `RequestCancelled`

## Step 1: Mint request ids as uuid v7
**Goal**: The core mints time-ordered request ids, so every live regrade has a new one (I4).
**Implementation Logic**:
`RequestId::new_v7() -> RequestId` in `primitives.rs`, beside the existing `text_type!(RequestId …)` and its `FromStr`, built from `uuid::Uuid::now_v7()` in its hyphenated lower-case form. The id's existing pattern accepts it unchanged: if it does not, stop and report a spec defect (the pattern is a published schema). No test of the version bits (R04 dropped list); WIRE1 checks that a minted id is accepted by the envelope.
**Deliverables**: `crates/candler-schemas/src/primitives.rs` (`RequestId::new_v7`)
**Consistency Checks**: `cargo clippy -p candler-schemas --all-targets -- -D warnings && test -z "$(git status --porcelain crates/candler-schemas/schema)"` (expected: PASS)
**Commit**: `feat(evaluator-protocol): mint request ids as uuid v7`

## Step 2: Add the JSON-RPC envelope as an additive root
**Goal**: One new root, `evaluator-message`, describes every line on an evaluator's stdin and stdout, and it embeds the existing roots by reference rather than copying them.
**Implementation Logic**:
1. `crates/candler-schemas/src/evaluation/rpc.rs` (declared `pub mod rpc;` in `evaluation.rs`, the non-`mod.rs` layout): a `JsonRpcVersion` unit type that reads and writes only `"2.0"`; `RpcRequest { jsonrpc, id: RequestId, call: Request }`; `RpcNotification { jsonrpc, notification: Notification }`; `RpcResponse { jsonrpc, id, result: ResponseResult }` with `ResponseResult` = `DescribeResult` | `EvaluateResult`; `RpcErrorResponse { jsonrpc, id, error: RpcError { code: i64, message, data? } }`, of which `RequestCancelled` is the `-32800` case; and `Message`, the union of the four.
2. `Request` and `Notification` keep their `method`/`params` shape and sit beside `jsonrpc` and `id` in the same JSON object, as JSON-RPC requires. Follow the `serde(remote = "Self")` and `object_only!` pattern of `evaluation.rs` so `null` and non-objects are refused and unknown keys are refused.
3. `roots()` in `evaluation.rs` gains `GeneratedSchema::of::<rpc::Message>("evaluator-message")`; the generated schema refers to the existing definitions, and no existing schema file changes (the additive gate). Bless with `CANDLER_BLESS_SCHEMAS=1` and read the new file's diff by hand before committing.
4. This commit touches only `crates/candler-schemas/` (schema publish rule).
**Deliverables**: `crates/candler-schemas/src/evaluation/rpc.rs` (`JsonRpcVersion`, `RpcRequest`, `RpcNotification`, `RpcResponse`, `ResponseResult`, `RpcErrorResponse`, `RpcError`, `Message`); `crates/candler-schemas/src/evaluation.rs` (`pub mod rpc`, the new root); `crates/candler-schemas/schema/evaluator-message.schema.json`
**Consistency Checks**: `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas -E 'binary(schema_drift)' && test -z "$(git diff --name-only --diff-filter=MDR origin/main -- crates/candler-schemas/schema)"` (expected: PASS)
**Commit**: `feat(schemas): frame evaluator messages as json-rpc 2.0`

## Step 3: Decide the envelope by shared wire vectors
**Goal**: Hand-written vectors from the JSON-RPC text and the existing roots accept and refuse exactly what the spec says, in a form the Python helper reads unchanged.
**Implementation Logic**:
1. `tests/fixtures/rpc/valid/*.json` and `tests/fixtures/rpc/invalid/*.json`, one message per file, hand-written from the JSON-RPC 2.0 text and R01 (never produced by the Rust types), plus `README.md` naming the spec clause each file comes from and `PROVENANCE.toml` (`synthetic = true`, the clause). Valid: a `describe` and an `evaluate` request, each notification method, a describe result, an evaluate result, the `-32800` error. Invalid, each a single fault on a valid base: `jsonrpc` missing, `jsonrpc` `"1.0"`, `id` a number, `id` `null`, an unknown top-level key, a request with both `result` and `method`.
2. `tests/guard_rpc_wire.rs` (`#![cfg(test)]`), two guards:
   - WIRE1 (R8): every `valid/` file parses as `Message` and every `invalid/` file is refused; a `RequestId::new_v7()` id placed in the valid `evaluate` request still parses. Oracle: JSON-RPC 2.0 §4–5 text.
   - WIRE2 (R8): a payload taken from the existing `evaluator-notification` root's vectors (or written to its schema) is accepted inside the envelope, and a single-fault copy of that payload (a missing required field) is refused inside the envelope too. Oracle: the existing root's schema, validated with `jsonschema` against `schema/evaluator-notification.schema.json`.
3. No test of the `-32800` literal or of serde derives (dropped list). Record both bites.
**Deliverables**: `crates/candler-schemas/tests/fixtures/rpc/{valid,invalid}/*.json`, `README.md`, `PROVENANCE.toml`; `crates/candler-schemas/tests/guard_rpc_wire.rs` (WIRE1, WIRE2)
**Consistency Checks**: `cargo nextest run -p candler-schemas -E 'binary(guard_rpc_wire)' && test "$(grep -c 'Guards:' crates/candler-schemas/tests/guard_rpc_wire.rs)" = "$(grep -c '#\[test\]' crates/candler-schemas/tests/guard_rpc_wire.rs)"` (expected: PASS)
**Commit**: `test(evaluator-protocol): decide the envelope by shared wire vectors`
