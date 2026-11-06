# Evaluator wire vectors

Hand-written from the JSON-RPC 2.0 text (https://www.jsonrpc.org/specification) and the
evaluator protocol of `__design__/architecture.md`; never produced by the Rust types. One message
per file, plain JSON. Rust (`guard_rpc_wire.rs`) and the Python helper read the same files.

Valid, each must be accepted:

| file | clause |
|:-----|:-------|
| `request_describe`, `request_evaluate` | §4 Request: `jsonrpc` "2.0", `method`, `params`, a string `id` (the protocol mints UUIDv7 text ids) |
| `notification_outcome`, `notification_progress`, `notification_log`, `notification_cancel` | §4.1 Notification: a request without `id` |
| `response_describe`, `response_evaluate` | §5 Response: `jsonrpc`, `id`, `result` |
| `error_cancelled` | §5.1 Error object: integer `code`, `message`; `-32800` is the protocol's cancelled request |

Invalid, each is one fault on a valid base and must be refused:

| file | fault | clause |
|:-----|:------|:-------|
| `jsonrpc_missing` | `jsonrpc` absent | §4: the member is required |
| `jsonrpc_1_0` | `jsonrpc` is "1.0" | §4: exactly "2.0" |
| `id_number` | `id` is the number 7 | the protocol's ids are strings (R01 §Evaluator protocol) |
| `id_null` | `id` is `null` | §4: the protocol never uses a null id |
| `unknown_key` | a top-level key `extra` | §4: the members are those listed |
| `result_and_method` | `result` beside `method` | §5: `result` belongs to a response only |
