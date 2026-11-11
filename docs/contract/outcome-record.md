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
| Code | When | Path at fault | Side to fix |
|---|---|---|---|
| `outcome.cause_missing` | A blocked, not-measured or not-applicable outcome carries no cause. | - | plugin |
| `outcome.cause_not_allowed` | A pass or fail outcome carries a cause other than the legacy one. | - | plugin |
| `outcome.evidence_empty` | A piece of evidence has neither a reference nor a note. | - | plugin |
| `outcome.fraction_denominator_zero` | The denominator of a fraction is 0. | - | plugin |
| `outcome.instrument_digest_dirty` | An instrument digest names a commit of a dirty tree. | - | plugin |
| `outcome.instrument_dirty` | A gate or quality outcome names an instrument that is not clean. | - | plugin |
| `outcome.probability_not_allowed` | A gate or quality outcome carries a probability. | - | plugin |
| `outcome.probability_out_of_range` | A probability lies outside 0 to 1. | - | plugin |
| `outcome.text_empty` | A text field of an outcome has no character. | - | plugin |
| `outcome.value_invalid` | A value of an outcome record has the wrong form for what it names, such as a name with a space. | - | plugin |
| `outcome.weight_above_one` | The numerator of a weight exceeds its denominator. | - | plugin |
| `outcome.weight_mismatch` | The weight of an outcome does not fit its status. | - | plugin |
| `outcome.weight_not_allowed` | A not-measured or not-applicable outcome carries a weight. | - | plugin |
| `outcome.zero_weight_without_legacy` | A weight of 0/0 comes without the legacy cause. | - | plugin |
<!-- /generated:refusals -->

## Notes
