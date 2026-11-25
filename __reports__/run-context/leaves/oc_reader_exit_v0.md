# Leaf report: OpenCode exit

Leaf: `__roadmap__/run_context/readers/oc_reader_exit.md`. Branch `leaf/run-context/oc_reader_exit`.

## The exit rule as implemented

In `tool_drafts` (`crates/candler-harness-opencode/src/export.rs`), beside the `argv` reading: a part
named `shell` whose `state.status` is `completed` and whose `state.metadata.exit` is an integer gives
`exit: {kind: code, code: N}` on its result. Everything else gives none: an error state (`aborted`,
`permission.rejected`, ...), a `completed` state with no `metadata.exit` (the probe's timed-out call,
which reports `signal: "SIGTERM"` and `timeout: true`), a non-integer or out-of-range value, and any
tool that is not `shell`. The reader reads no `signal` key, no `timeout` key and no tool output text,
and never builds a signal from a code.

The probe (question 1) found the code at `state.metadata.exit` as the plan expected, so the rule is
the plan's. The probe's table says "whatever the status"; the leaf Goal says an error state has none.
No recorded error state carries `metadata.exit`, so the two agree on every fixture; the code follows
the leaf.

## The oracle

Expected values come from `jq` over each `export.json`, never from the reader:

```
jq -c '[.messages[]?.content[]? | select(.type=="tool")
  | {id, name, status: .state.status, exit: .state.metadata.exit,
     signal: .state.metadata.signal, timeout: .state.metadata.timeout, err: .state.error.type}]' \
  crates/candler-harness-opencode/tests/fixtures/streams/<fixture>/export.json
```

| Fixture | name | status | exit | other | expected `exit` on the result |
|---|---|---|---|---|---|
| `completed_o1` | shell | completed | 0 | | code 0 |
| `shell_exit_stderr_timeout` call 1 | shell | completed | 3 | | code 3 |
| `shell_exit_stderr_timeout` call 2 (stderr only) | shell | completed | 2 | | code 2 |
| `shell_exit_stderr_timeout` call 3 | shell | completed | null | signal SIGTERM, timeout true | none |
| `sigint_mid_tool`, `interrupt_api` | shell | error | null | `aborted` | none |
| `oc_permission_ask` | shell | error | null | `permission.rejected` | none |
| inline document, tool `read` | read | completed | 7 (hand-written) | | none |

No recorded export holds a non-`shell` tool, so that row is an inline document in the table whose
`metadata.exit` is set to 7 to show the rule reads the tool's name.

## Guards on the base

Table: `opencode_export_reader_covers_trace_errors_cache_cost_and_child_sessions`. On the base
(`exit` column added, reader passing `None`) it fails at the first row, `completed_o1`:

```
assertion `left == right` failed
  left:  ... {"is_error": false, "kind": "tool_result"} ...
  right: ... {"exit": {"code": 0, "kind": "code"}, "is_error": false, "kind": "tool_result"} ...
```

The table stops at its first failure, so the later rows were checked separately: on a scratch branch
from the base plus the test commit (deleted), with the `completed_o1` row's exit temporarily removed
from the expectation, the table fails at the probe's rows with the reader giving none:

```
assertion `left == right` failed: shell_exit_stderr_timeout
  left:  [{"is_error": false, "kind": "tool_result"} x3]
  right: [{"exit": {"code": 3, ...}, ...}, {"exit": {"code": 2, ...}, ...}, {"is_error": false, ...}]
```

So every row with a code fails on the base. The rows without an exit (aborted, rejected, `read`)
pass on the base by design: they guard against reading too much (the `read` row bites when the
`shell` check is dropped).

## Rulings

- The non-`shell` row's inline document is an accepted deviation from "written from `jq` over the
  exports": no recorded export holds a non-`shell` tool. The hand-set `exit: 7` is an input, and the
  row bites if the `shell` check is dropped.
- `sigterm_mid_tool` (aborted, not in the table) also reads no exit.
- An `exit` that is not an `i32` gives no exit, as the reader treats other malformed harness fields.

## Bites

Each applied on a scratch branch (deleted), the table run, the first failing message recorded.

| Bite | Change | Failing message |
|---|---|---|
| (a) `metadata.exit` read only on error states | `Some("completed")` replaced by `Some("error")` in the status test | `completed_o1` row: left `{"is_error": false, "kind": "tool_result"}`, right carries `"exit": {"code": 0, "kind": "code"}` |
| (b) a missing `metadata.exit` taken as 0 | `.or(Some(0))` after `as_i64` | `assertion left == right failed: shell_exit_stderr_timeout`: left's third result carries `"exit": {"code": 0, ...}`, right's carries none |

## Finding outside the leaf

`cargo nextest run --workspace` has one failure, not caused by this leaf:
`candler-core::it guard_harness_facts::harness_census_equals_cf3de30_on_every_fixture` panics with
`no baseline for streams/shell_exit_stderr_timeout` (`guard_harness_facts.rs:726`). The probe's new
fixture directory has no baseline row in that census. The other 734 tests pass. The fix belongs to
whoever owns the probe fixture's census entry.

## What is left

- The coordinator's fresh Sonnet verifier (Step 3) and `container-e2e` on the pull request.
- Nothing else; no `confirm_` check remains (`git grep -n confirm_ -- crates python` prints nothing).
