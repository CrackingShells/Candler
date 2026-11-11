# Outcome record

## Surface

The result of one evaluator for one expectation of one run: an evaluator reports the fields it owns, Candler adds the fields marked `candler` and stores the record.

## Version

<!-- generated:version -->
Wire tag: none.

<!-- versions -->
Wire line: `schemas@0.6.0`.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
| Path | Type | Class | Default |
|---|---|---|---|
| `cause` | `Cause` | optional | - |
| `cause.expectation` | `ExpectationId` | required | - |
| `cause.artifact` | `RelPath` | required | - |
| `cause.reason` | `ReasonCode` | required | - |
| `cause.legacy` | `ReasonCode` | required | - |
| `context_digest` | `Digest` | required | - |
| `evaluator` | `NameVersion` | required | - |
| `evidence` | `array of Evidence` | required | - |
| `evidence[].note` | `string` | optional | - |
| `evidence[].ref` | `EvidenceRef` | optional | - |
| `expectation_id` | `ExpectationId` | required | - |
| `instruments` | `array of Instrument` | required | - |
| `instruments[].digest` | `InstrumentDigest` | required | - |
| `instruments[].dirty` | `boolean` | required | - |
| `instruments[].name` | `string` | required | - |
| `instruments[].version` | `string` | required | - |
| `probability` | `Probability` | optional | - |
| `probability.calibration_ref` | `string` | required | - |
| `probability.p` | `number` | required | - |
| `request_id` | `RequestId` | required | - |
| `role` | `"gate" \| "quality" \| "descriptive"` | required | - |
| `run` | `RunKey` | required | - |
| `run.attempt` | `integer` | required | - |
| `run.cell` | `Digest` | required | - |
| `run.replicate` | `integer` | required | - |
| `status` | `"pass" \| "fail" \| "blocked" \| "not_measured" \| "not_applicable"` | required | - |
| `suite_entry_digest` | `Digest` | required | - |
| `value` | `Value` | optional | - |
| `value.kind` | `"number" \| "count" \| "fraction" \| "text" \| "counts"` | required | - |
| `value.number` | `Finite` | required | - |
| `value.count` | `integer` | required | - |
| `value.den` | `integer` | required | - |
| `value.num` | `integer` | required | - |
| `value.text` | `string` | required | - |
| `value.counts` | `map of integer` | required | - |
| `weight` | `Weight` | optional | - |
| `weight.den` | `integer` | required | - |
| `weight.num` | `integer` | required | - |

<!-- /generated:fields -->

## Refusals

<!-- generated:refusals -->

## Notes
