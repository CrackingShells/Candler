# Normalised trace

## Surface

The normalised trace Candler stores for each run: a header, the sessions and their events, read from the harness's raw output and from what the gateway relayed, by an evaluator or an analyst.

## Version

<!-- generated:version -->
Wire tag: none.

<!-- versions -->
Wire line: `schemas@0.14.0`.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
### `kind = "header"`

| Path | Type | Class | Default |
|---|---|---|---|
| `kind` | `"header"` | required | - |
| `raw` | `array of RawFileRef` | required | - |
| `raw[].digest` | `Digest` | required | - |
| `raw[].stream` | `StreamName` | required | - |
| `reader` | `NameVersion` | required | - |
| `schema` | `"normalised-trace/1"` | required | - |

### `kind = "session"`

| Path | Type | Class | Default |
|---|---|---|---|
| `ended_at` | `Timestamp` | optional | - |
| `ended_by` | `EndedBy` | required | - |
| `ended_by.kind` | `"completed" \| "agent_error" \| "cap" \| "gateway_error" \| "budget_stop" \| "killed" \| "interrupted" \| "unknown"` | required | - |
| `ended_by.reason` | `ReasonCode` | optional | - |
| `ended_by.cap` | `"turns" \| "cost" \| "wall"` | required (ended_by.kind = cap) | - |
| `id` | `SessionId` | required | - |
| `kind` | `"session"` | required | - |
| `model` | `string` | optional | - |
| `parent_session` | `SessionId` | optional | - |
| `raw_ref` | `RawRef` | required | - |
| `raw_ref.frame` | `integer` | required | - |
| `raw_ref.stream` | `StreamName` | required | - |
| `source` | `SessionSource` | required | - |
| `source.harness` | `Name` | required | - |
| `source.harness_version` | `string` | required | - |
| `source.reader` | `Name` | required | - |
| `source.reader_version` | `string` | required | - |
| `started_at` | `Timestamp` | optional | - |

### `kind = "event"`

| Path | Type | Class | Default |
|---|---|---|---|
| `event` | `Event` | required | - |
| `event.at` | `Timestamp` | optional | - |
| `event.ext` | `map` | required | - |
| `event.kind` | `EventKind` | required | - |
| `event.kind.first_message` | `string` | required (event.kind.kind = context_start) | - |
| `event.kind.kind` | `"context_start" \| "user_message" \| "assistant_message" \| "tool_call" \| "tool_result" \| "notification" \| "usage"` | required | - |
| `event.kind.system` | `string` | required (event.kind.kind = context_start) | - |
| `event.kind.text` | `string` | optional | - |
| `event.kind.argv` | `array of string` | optional | - |
| `event.kind.name` | `string` | required (event.kind.kind = tool_call) | - |
| `event.kind.origin` | `ToolOrigin` | required (event.kind.kind = tool_call) | - |
| `event.kind.origin.kind` | `"builtin" \| "mcp" \| "other"` | required | - |
| `event.kind.origin.server` | `string` | required (event.kind.origin.kind = mcp) | - |
| `event.kind.origin.tool` | `string` | optional | - |
| `event.kind.wrapped_calls` | `array of WrappedCall` | required (event.kind.kind = tool_call) | - |
| `event.kind.wrapped_calls[].name` | `string` | required | - |
| `event.kind.wrapped_calls[].origin` | `ToolOrigin` | required | - |
| `event.kind.wrapped_calls[].origin.kind` | `"builtin" \| "mcp" \| "other"` | required | - |
| `event.kind.wrapped_calls[].origin.server` | `string` | required (event.kind.wrapped_calls[].origin.kind = mcp) | - |
| `event.kind.wrapped_calls[].origin.tool` | `string` | optional | - |
| `event.kind.denied` | `Denial` | optional | - |
| `event.kind.denied.by` | `"harness" \| "hook" \| "user"` | required | - |
| `event.kind.denied.reason` | `string` | optional | - |
| `event.kind.is_error` | `boolean` | required (event.kind.kind = tool_result) | - |
| `event.kind.cache_creation_tokens` | `integer` | required (event.kind.kind = usage) | - |
| `event.kind.cache_read_tokens` | `integer` | required (event.kind.kind = usage) | - |
| `event.kind.cost_usd` | `Finite` | optional | - |
| `event.kind.input_tokens` | `integer` | required (event.kind.kind = usage) | - |
| `event.kind.output_tokens` | `integer` | required (event.kind.kind = usage) | - |
| `event.kind.scope` | `"message" \| "session_total"` | required (event.kind.kind = usage) | - |
| `event.raw_ref` | `RawRef` | required | - |
| `event.raw_ref.frame` | `integer` | required | - |
| `event.raw_ref.stream` | `StreamName` | required | - |
| `event.seq` | `integer` | required | - |
| `event.span` | `SpanId` | optional | - |
| `kind` | `"event"` | required | - |
| `session` | `SessionId` | required | - |

<!-- /generated:fields -->

## Refusals

<!-- generated:refusals -->
No refusal of this surface has a code.
<!-- /generated:refusals -->

## Notes

- `context_start` is the root session's first event when the gateway saw a request that carries tools: it holds the run's first such request that has a system text or a user message, `system` and `first_message`, as the gateway received them. A trace stored without it still reads. [checked: schemas@0.13.0]
- Of a list of blocks the text blocks are kept, the others dropped, joined by a newline. An OpenAI request's `system` and `developer` messages are `system`; `first_message` is its first `user` message only. Claude Code puts instruction files and hook output in `first_message`, OpenCode in `system`. [checked: schemas@0.13.0]
- The gateway's log line for it is 15,500 bytes on Claude Code and 8,943 on OpenCode (fake model, no instruction files), one per run. [checked: schemas@0.13.0]
