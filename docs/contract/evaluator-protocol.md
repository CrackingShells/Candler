# Evaluator protocol

## Surface

The messages Candler and an evaluator plug-in exchange, one JSON object per line: requests from the core, notifications from either side, and the evaluator's responses.

## Version

<!-- generated:version -->
Wire tag: none.

<!-- versions -->
Wire line: `schemas@0.6.0`.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
### `evaluator-describe-result`

| Path | Type | Class | Default |
|---|---|---|---|
| `environment` | `EvaluatorEnvironment` | required | - |
| `environment.lock_digest` | `Digest` | optional | - |
| `environment.packages` | `map of string` | required | - |
| `instruments` | `array of Instrument` | required | - |
| `instruments[].digest` | `InstrumentDigest` | required | - |
| `instruments[].dirty` | `boolean` | required | - |
| `instruments[].name` | `string` | required | - |
| `instruments[].version` | `string` | required | - |
| `kind` | `"deterministic" \| "probabilistic"` | required | - |
| `kinds` | `array of KindSpec` | required | - |
| `kinds[].default_deadline_ms` | `integer` | required | - |
| `kinds[].kind` | `Name` | required | - |
| `kinds[].needs` | `Needs` | required | - |
| `kinds[].needs.artifacts` | `array of string` | required | - |
| `kinds[].needs.inputs` | `array of Name` | required | - |
| `kinds[].needs.metadata` | `array of FieldPath` | required | - |
| `kinds[].needs.snapshots` | `array of Name` | required | - |
| `kinds[].needs.trace` | `boolean` | required | - |
| `kinds[].params_schema` | `any` | required | - |
| `name` | `Name` | required | - |
| `protocol` | `ProtocolVersion` | required | - |
| `protocol.major` | `integer` | required | - |
| `protocol.minor` | `integer` | required | - |
| `reuse` | `any` | optional | - |
| `version` | `string` | required | - |

### `evaluator-evaluate-params`

| Path | Type | Class | Default |
|---|---|---|---|
| `context` | `RunContext` | candler | - |
| `context.artifacts` | `array of ContextArtifact` | required | - |
| `context.artifacts[].abs` | `string` | required | - |
| `context.artifacts[].path` | `RelPath` | required | - |
| `context.artifacts[].sha256` | `Digest` | required | - |
| `context.inputs` | `array of ContextRef` | required | - |
| `context.inputs[].digest` | `Digest` | required | - |
| `context.inputs[].kind` | `Name` | required | - |
| `context.inputs[].path` | `string` | required | - |
| `context.metadata` | `map` | required | - |
| `context.raw` | `array of ContextRaw` | required | - |
| `context.raw[].path` | `string` | required | - |
| `context.raw[].stream` | `StreamName` | required | - |
| `context.root` | `string` | required | - |
| `context.run` | `RunKey` | required | - |
| `context.run.attempt` | `integer` | required | - |
| `context.run.cell` | `Digest` | required | - |
| `context.run.replicate` | `integer` | required | - |
| `context.snapshots` | `array of ContextRef` | required | - |
| `context.snapshots[].digest` | `Digest` | required | - |
| `context.snapshots[].kind` | `Name` | required | - |
| `context.snapshots[].path` | `string` | required | - |
| `context.trace` | `string` | optional | - |
| `items` | `array of Item` | candler | - |
| `items[].deadline_ms` | `integer` | required | - |
| `items[].expectation_id` | `ExpectationId` | required | - |
| `items[].kind` | `Name` | required | - |
| `items[].params` | `map` | required | - |
| `prior` | `array of OutcomeRecord` | candler | - |
| `prior[].cause` | `Cause` | optional | - |
| `prior[].cause.expectation` | `ExpectationId` | required | - |
| `prior[].cause.artifact` | `RelPath` | required | - |
| `prior[].cause.reason` | `ReasonCode` | required | - |
| `prior[].cause.legacy` | `ReasonCode` | required | - |
| `prior[].context_digest` | `Digest` | required | - |
| `prior[].evaluator` | `NameVersion` | required | - |
| `prior[].evidence` | `array of Evidence` | required | - |
| `prior[].evidence[].note` | `string` | optional | - |
| `prior[].evidence[].ref` | `EvidenceRef` | optional | - |
| `prior[].expectation_id` | `ExpectationId` | required | - |
| `prior[].instruments` | `array of Instrument` | required | - |
| `prior[].instruments[].digest` | `InstrumentDigest` | required | - |
| `prior[].instruments[].dirty` | `boolean` | required | - |
| `prior[].instruments[].name` | `string` | required | - |
| `prior[].instruments[].version` | `string` | required | - |
| `prior[].probability` | `Probability` | optional | - |
| `prior[].probability.calibration_ref` | `string` | required | - |
| `prior[].probability.p` | `number` | required | - |
| `prior[].request_id` | `RequestId` | required | - |
| `prior[].role` | `"gate" \| "quality" \| "descriptive"` | required | - |
| `prior[].run` | `RunKey` | required | - |
| `prior[].run.attempt` | `integer` | required | - |
| `prior[].run.cell` | `Digest` | required | - |
| `prior[].run.replicate` | `integer` | required | - |
| `prior[].status` | `"pass" \| "fail" \| "blocked" \| "not_measured" \| "not_applicable"` | required | - |
| `prior[].suite_entry_digest` | `Digest` | required | - |
| `prior[].value` | `Value` | optional | - |
| `prior[].value.kind` | `"number" \| "count" \| "fraction" \| "text" \| "counts"` | required | - |
| `prior[].value.number` | `Finite` | required | - |
| `prior[].value.count` | `integer` | required | - |
| `prior[].value.den` | `integer` | required | - |
| `prior[].value.num` | `integer` | required | - |
| `prior[].value.text` | `string` | required | - |
| `prior[].value.counts` | `map of integer` | required | - |
| `prior[].weight` | `Weight` | optional | - |
| `prior[].weight.den` | `integer` | required | - |
| `prior[].weight.num` | `integer` | required | - |
| `scratch` | `string` | candler | - |

### `evaluator-message`

#### `RpcRequest / method = "describe"`

| Path | Type | Class | Default |
|---|---|---|---|
| `id` | `RequestId` | required | - |
| `jsonrpc` | `"2.0"` | required | - |
| `method` | `"describe"` | required | - |
| `params` | `DescribeParams` | required | - |

#### `RpcRequest / method = "evaluate"`

| Path | Type | Class | Default |
|---|---|---|---|
| `id` | `RequestId` | required | - |
| `jsonrpc` | `"2.0"` | required | - |
| `method` | `"evaluate"` | required | - |
| `params` | `EvaluateParams` | required | - |
| `params.context` | `RunContext` | candler | - |
| `params.context.artifacts` | `array of ContextArtifact` | required | - |
| `params.context.artifacts[].abs` | `string` | required | - |
| `params.context.artifacts[].path` | `RelPath` | required | - |
| `params.context.artifacts[].sha256` | `Digest` | required | - |
| `params.context.inputs` | `array of ContextRef` | required | - |
| `params.context.inputs[].digest` | `Digest` | required | - |
| `params.context.inputs[].kind` | `Name` | required | - |
| `params.context.inputs[].path` | `string` | required | - |
| `params.context.metadata` | `map` | required | - |
| `params.context.raw` | `array of ContextRaw` | required | - |
| `params.context.raw[].path` | `string` | required | - |
| `params.context.raw[].stream` | `StreamName` | required | - |
| `params.context.root` | `string` | required | - |
| `params.context.run` | `RunKey` | required | - |
| `params.context.run.attempt` | `integer` | required | - |
| `params.context.run.cell` | `Digest` | required | - |
| `params.context.run.replicate` | `integer` | required | - |
| `params.context.snapshots` | `array of ContextRef` | required | - |
| `params.context.snapshots[].digest` | `Digest` | required | - |
| `params.context.snapshots[].kind` | `Name` | required | - |
| `params.context.snapshots[].path` | `string` | required | - |
| `params.context.trace` | `string` | optional | - |
| `params.items` | `array of Item` | candler | - |
| `params.items[].deadline_ms` | `integer` | required | - |
| `params.items[].expectation_id` | `ExpectationId` | required | - |
| `params.items[].kind` | `Name` | required | - |
| `params.items[].params` | `map` | required | - |
| `params.prior` | `array of OutcomeRecord` | candler | - |
| `params.prior[].cause` | `Cause` | optional | - |
| `params.prior[].cause.expectation` | `ExpectationId` | required | - |
| `params.prior[].cause.artifact` | `RelPath` | required | - |
| `params.prior[].cause.reason` | `ReasonCode` | required | - |
| `params.prior[].cause.legacy` | `ReasonCode` | required | - |
| `params.prior[].context_digest` | `Digest` | required | - |
| `params.prior[].evaluator` | `NameVersion` | required | - |
| `params.prior[].evidence` | `array of Evidence` | required | - |
| `params.prior[].evidence[].note` | `string` | optional | - |
| `params.prior[].evidence[].ref` | `EvidenceRef` | optional | - |
| `params.prior[].expectation_id` | `ExpectationId` | required | - |
| `params.prior[].instruments` | `array of Instrument` | required | - |
| `params.prior[].instruments[].digest` | `InstrumentDigest` | required | - |
| `params.prior[].instruments[].dirty` | `boolean` | required | - |
| `params.prior[].instruments[].name` | `string` | required | - |
| `params.prior[].instruments[].version` | `string` | required | - |
| `params.prior[].probability` | `Probability` | optional | - |
| `params.prior[].probability.calibration_ref` | `string` | required | - |
| `params.prior[].probability.p` | `number` | required | - |
| `params.prior[].request_id` | `RequestId` | required | - |
| `params.prior[].role` | `"gate" \| "quality" \| "descriptive"` | required | - |
| `params.prior[].run` | `RunKey` | required | - |
| `params.prior[].run.attempt` | `integer` | required | - |
| `params.prior[].run.cell` | `Digest` | required | - |
| `params.prior[].run.replicate` | `integer` | required | - |
| `params.prior[].status` | `"pass" \| "fail" \| "blocked" \| "not_measured" \| "not_applicable"` | required | - |
| `params.prior[].suite_entry_digest` | `Digest` | required | - |
| `params.prior[].value` | `Value` | optional | - |
| `params.prior[].value.kind` | `"number" \| "count" \| "fraction" \| "text" \| "counts"` | required | - |
| `params.prior[].value.number` | `Finite` | required | - |
| `params.prior[].value.count` | `integer` | required | - |
| `params.prior[].value.den` | `integer` | required | - |
| `params.prior[].value.num` | `integer` | required | - |
| `params.prior[].value.text` | `string` | required | - |
| `params.prior[].value.counts` | `map of integer` | required | - |
| `params.prior[].weight` | `Weight` | optional | - |
| `params.prior[].weight.den` | `integer` | required | - |
| `params.prior[].weight.num` | `integer` | required | - |
| `params.scratch` | `string` | candler | - |

#### `RpcNotification / method = "outcome"`

| Path | Type | Class | Default |
|---|---|---|---|
| `jsonrpc` | `"2.0"` | required | - |
| `method` | `"outcome"` | required | - |
| `params` | `OutcomeParams` | required | - |
| `params.record` | `OutcomeRecord` | required | - |
| `params.record.cause` | `Cause` | optional | - |
| `params.record.cause.expectation` | `ExpectationId` | required | - |
| `params.record.cause.artifact` | `RelPath` | required | - |
| `params.record.cause.reason` | `ReasonCode` | required | - |
| `params.record.cause.legacy` | `ReasonCode` | required | - |
| `params.record.context_digest` | `Digest` | required | - |
| `params.record.evaluator` | `NameVersion` | required | - |
| `params.record.evidence` | `array of Evidence` | required | - |
| `params.record.evidence[].note` | `string` | optional | - |
| `params.record.evidence[].ref` | `EvidenceRef` | optional | - |
| `params.record.expectation_id` | `ExpectationId` | required | - |
| `params.record.instruments` | `array of Instrument` | required | - |
| `params.record.instruments[].digest` | `InstrumentDigest` | required | - |
| `params.record.instruments[].dirty` | `boolean` | required | - |
| `params.record.instruments[].name` | `string` | required | - |
| `params.record.instruments[].version` | `string` | required | - |
| `params.record.probability` | `Probability` | optional | - |
| `params.record.probability.calibration_ref` | `string` | required | - |
| `params.record.probability.p` | `number` | required | - |
| `params.record.request_id` | `RequestId` | required | - |
| `params.record.role` | `"gate" \| "quality" \| "descriptive"` | required | - |
| `params.record.run` | `RunKey` | required | - |
| `params.record.run.attempt` | `integer` | required | - |
| `params.record.run.cell` | `Digest` | required | - |
| `params.record.run.replicate` | `integer` | required | - |
| `params.record.status` | `"pass" \| "fail" \| "blocked" \| "not_measured" \| "not_applicable"` | required | - |
| `params.record.suite_entry_digest` | `Digest` | required | - |
| `params.record.value` | `Value` | optional | - |
| `params.record.value.kind` | `"number" \| "count" \| "fraction" \| "text" \| "counts"` | required | - |
| `params.record.value.number` | `Finite` | required | - |
| `params.record.value.count` | `integer` | required | - |
| `params.record.value.den` | `integer` | required | - |
| `params.record.value.num` | `integer` | required | - |
| `params.record.value.text` | `string` | required | - |
| `params.record.value.counts` | `map of integer` | required | - |
| `params.record.weight` | `Weight` | optional | - |
| `params.record.weight.den` | `integer` | required | - |
| `params.record.weight.num` | `integer` | required | - |

#### `RpcNotification / method = "progress"`

| Path | Type | Class | Default |
|---|---|---|---|
| `jsonrpc` | `"2.0"` | required | - |
| `method` | `"progress"` | required | - |
| `params` | `ProgressParams` | required | - |

#### `RpcNotification / method = "log"`

| Path | Type | Class | Default |
|---|---|---|---|
| `jsonrpc` | `"2.0"` | required | - |
| `method` | `"log"` | required | - |
| `params` | `LogParams` | required | - |

#### `RpcNotification / method = "cancel"`

| Path | Type | Class | Default |
|---|---|---|---|
| `jsonrpc` | `"2.0"` | required | - |
| `method` | `"cancel"` | required | - |
| `params` | `CancelParams` | required | - |
| `params.request_id` | `RequestId` | required | - |

#### `RpcResponse`

| Path | Type | Class | Default |
|---|---|---|---|
| `id` | `RequestId` | required | - |
| `jsonrpc` | `"2.0"` | required | - |
| `result` | `ResponseResult` | required | - |
| `result.environment` | `EvaluatorEnvironment` | required | - |
| `result.environment.lock_digest` | `Digest` | optional | - |
| `result.environment.packages` | `map of string` | required | - |
| `result.instruments` | `array of Instrument` | required | - |
| `result.instruments[].digest` | `InstrumentDigest` | required | - |
| `result.instruments[].dirty` | `boolean` | required | - |
| `result.instruments[].name` | `string` | required | - |
| `result.instruments[].version` | `string` | required | - |
| `result.kind` | `"deterministic" \| "probabilistic"` | required | - |
| `result.kinds` | `array of KindSpec` | required | - |
| `result.kinds[].default_deadline_ms` | `integer` | required | - |
| `result.kinds[].kind` | `Name` | required | - |
| `result.kinds[].needs` | `Needs` | required | - |
| `result.kinds[].needs.artifacts` | `array of string` | required | - |
| `result.kinds[].needs.inputs` | `array of Name` | required | - |
| `result.kinds[].needs.metadata` | `array of FieldPath` | required | - |
| `result.kinds[].needs.snapshots` | `array of Name` | required | - |
| `result.kinds[].needs.trace` | `boolean` | required | - |
| `result.kinds[].params_schema` | `any` | required | - |
| `result.name` | `Name` | required | - |
| `result.protocol` | `ProtocolVersion` | required | - |
| `result.protocol.major` | `integer` | required | - |
| `result.protocol.minor` | `integer` | required | - |
| `result.reuse` | `any` | optional | - |
| `result.version` | `string` | required | - |
| `result.reported` | `integer` | required | - |

#### `RpcErrorResponse`

| Path | Type | Class | Default |
|---|---|---|---|
| `error` | `RpcError` | required | - |
| `error.code` | `integer` | required | - |
| `error.data` | `any` | free | - |
| `error.message` | `string` | required | - |
| `id` | `RequestId` | required | - |
| `jsonrpc` | `"2.0"` | required | - |

### `evaluator-notification`

#### `method = "outcome"`

| Path | Type | Class | Default |
|---|---|---|---|
| `method` | `"outcome"` | required | - |
| `params` | `OutcomeParams` | required | - |
| `params.record` | `OutcomeRecord` | required | - |
| `params.record.cause` | `Cause` | optional | - |
| `params.record.cause.expectation` | `ExpectationId` | required | - |
| `params.record.cause.artifact` | `RelPath` | required | - |
| `params.record.cause.reason` | `ReasonCode` | required | - |
| `params.record.cause.legacy` | `ReasonCode` | required | - |
| `params.record.context_digest` | `Digest` | required | - |
| `params.record.evaluator` | `NameVersion` | required | - |
| `params.record.evidence` | `array of Evidence` | required | - |
| `params.record.evidence[].note` | `string` | optional | - |
| `params.record.evidence[].ref` | `EvidenceRef` | optional | - |
| `params.record.expectation_id` | `ExpectationId` | required | - |
| `params.record.instruments` | `array of Instrument` | required | - |
| `params.record.instruments[].digest` | `InstrumentDigest` | required | - |
| `params.record.instruments[].dirty` | `boolean` | required | - |
| `params.record.instruments[].name` | `string` | required | - |
| `params.record.instruments[].version` | `string` | required | - |
| `params.record.probability` | `Probability` | optional | - |
| `params.record.probability.calibration_ref` | `string` | required | - |
| `params.record.probability.p` | `number` | required | - |
| `params.record.request_id` | `RequestId` | required | - |
| `params.record.role` | `"gate" \| "quality" \| "descriptive"` | required | - |
| `params.record.run` | `RunKey` | required | - |
| `params.record.run.attempt` | `integer` | required | - |
| `params.record.run.cell` | `Digest` | required | - |
| `params.record.run.replicate` | `integer` | required | - |
| `params.record.status` | `"pass" \| "fail" \| "blocked" \| "not_measured" \| "not_applicable"` | required | - |
| `params.record.suite_entry_digest` | `Digest` | required | - |
| `params.record.value` | `Value` | optional | - |
| `params.record.value.kind` | `"number" \| "count" \| "fraction" \| "text" \| "counts"` | required | - |
| `params.record.value.number` | `Finite` | required | - |
| `params.record.value.count` | `integer` | required | - |
| `params.record.value.den` | `integer` | required | - |
| `params.record.value.num` | `integer` | required | - |
| `params.record.value.text` | `string` | required | - |
| `params.record.value.counts` | `map of integer` | required | - |
| `params.record.weight` | `Weight` | optional | - |
| `params.record.weight.den` | `integer` | required | - |
| `params.record.weight.num` | `integer` | required | - |

#### `method = "progress"`

| Path | Type | Class | Default |
|---|---|---|---|
| `method` | `"progress"` | required | - |
| `params` | `ProgressParams` | required | - |

#### `method = "log"`

| Path | Type | Class | Default |
|---|---|---|---|
| `method` | `"log"` | required | - |
| `params` | `LogParams` | required | - |

#### `method = "cancel"`

| Path | Type | Class | Default |
|---|---|---|---|
| `method` | `"cancel"` | required | - |
| `params` | `CancelParams` | required | - |
| `params.request_id` | `RequestId` | required | - |

### `evaluator-request-side`

#### `Request / method = "describe"`

| Path | Type | Class | Default |
|---|---|---|---|
| `method` | `"describe"` | required | - |
| `params` | `DescribeParams` | required | - |

#### `Request / method = "evaluate"`

| Path | Type | Class | Default |
|---|---|---|---|
| `method` | `"evaluate"` | required | - |
| `params` | `EvaluateParams` | required | - |
| `params.context` | `RunContext` | candler | - |
| `params.context.artifacts` | `array of ContextArtifact` | required | - |
| `params.context.artifacts[].abs` | `string` | required | - |
| `params.context.artifacts[].path` | `RelPath` | required | - |
| `params.context.artifacts[].sha256` | `Digest` | required | - |
| `params.context.inputs` | `array of ContextRef` | required | - |
| `params.context.inputs[].digest` | `Digest` | required | - |
| `params.context.inputs[].kind` | `Name` | required | - |
| `params.context.inputs[].path` | `string` | required | - |
| `params.context.metadata` | `map` | required | - |
| `params.context.raw` | `array of ContextRaw` | required | - |
| `params.context.raw[].path` | `string` | required | - |
| `params.context.raw[].stream` | `StreamName` | required | - |
| `params.context.root` | `string` | required | - |
| `params.context.run` | `RunKey` | required | - |
| `params.context.run.attempt` | `integer` | required | - |
| `params.context.run.cell` | `Digest` | required | - |
| `params.context.run.replicate` | `integer` | required | - |
| `params.context.snapshots` | `array of ContextRef` | required | - |
| `params.context.snapshots[].digest` | `Digest` | required | - |
| `params.context.snapshots[].kind` | `Name` | required | - |
| `params.context.snapshots[].path` | `string` | required | - |
| `params.context.trace` | `string` | optional | - |
| `params.items` | `array of Item` | candler | - |
| `params.items[].deadline_ms` | `integer` | required | - |
| `params.items[].expectation_id` | `ExpectationId` | required | - |
| `params.items[].kind` | `Name` | required | - |
| `params.items[].params` | `map` | required | - |
| `params.prior` | `array of OutcomeRecord` | candler | - |
| `params.prior[].cause` | `Cause` | optional | - |
| `params.prior[].cause.expectation` | `ExpectationId` | required | - |
| `params.prior[].cause.artifact` | `RelPath` | required | - |
| `params.prior[].cause.reason` | `ReasonCode` | required | - |
| `params.prior[].cause.legacy` | `ReasonCode` | required | - |
| `params.prior[].context_digest` | `Digest` | required | - |
| `params.prior[].evaluator` | `NameVersion` | required | - |
| `params.prior[].evidence` | `array of Evidence` | required | - |
| `params.prior[].evidence[].note` | `string` | optional | - |
| `params.prior[].evidence[].ref` | `EvidenceRef` | optional | - |
| `params.prior[].expectation_id` | `ExpectationId` | required | - |
| `params.prior[].instruments` | `array of Instrument` | required | - |
| `params.prior[].instruments[].digest` | `InstrumentDigest` | required | - |
| `params.prior[].instruments[].dirty` | `boolean` | required | - |
| `params.prior[].instruments[].name` | `string` | required | - |
| `params.prior[].instruments[].version` | `string` | required | - |
| `params.prior[].probability` | `Probability` | optional | - |
| `params.prior[].probability.calibration_ref` | `string` | required | - |
| `params.prior[].probability.p` | `number` | required | - |
| `params.prior[].request_id` | `RequestId` | required | - |
| `params.prior[].role` | `"gate" \| "quality" \| "descriptive"` | required | - |
| `params.prior[].run` | `RunKey` | required | - |
| `params.prior[].run.attempt` | `integer` | required | - |
| `params.prior[].run.cell` | `Digest` | required | - |
| `params.prior[].run.replicate` | `integer` | required | - |
| `params.prior[].status` | `"pass" \| "fail" \| "blocked" \| "not_measured" \| "not_applicable"` | required | - |
| `params.prior[].suite_entry_digest` | `Digest` | required | - |
| `params.prior[].value` | `Value` | optional | - |
| `params.prior[].value.kind` | `"number" \| "count" \| "fraction" \| "text" \| "counts"` | required | - |
| `params.prior[].value.number` | `Finite` | required | - |
| `params.prior[].value.count` | `integer` | required | - |
| `params.prior[].value.den` | `integer` | required | - |
| `params.prior[].value.num` | `integer` | required | - |
| `params.prior[].value.text` | `string` | required | - |
| `params.prior[].value.counts` | `map of integer` | required | - |
| `params.prior[].weight` | `Weight` | optional | - |
| `params.prior[].weight.den` | `integer` | required | - |
| `params.prior[].weight.num` | `integer` | required | - |
| `params.scratch` | `string` | candler | - |

#### `EvaluateResult`

| Path | Type | Class | Default |
|---|---|---|---|
| `reported` | `integer` | required | - |

#### `RequestCancelled`

| Path | Type | Class | Default |
|---|---|---|---|
| `code` | `-32800` | required | - |
| `message` | `string` | required | - |

<!-- /generated:fields -->

## Refusals

<!-- generated:refusals -->

## Notes
