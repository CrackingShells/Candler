# Outcome record

## Surface

The result of one evaluator for one expectation of one run: an evaluator reports the fields it owns, Candler adds the fields marked `candler` and stores the record.

## Version

<!-- generated:version -->
Wire tag: none.

<!-- versions -->
Wire line: `schemas@0.7.0`.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
| Path | Type | Class | Default |
|---|---|---|---|
| `cause` | `Cause` | optional | - |
| `cause.expectation` | `ExpectationId` | required (exactly one key of cause) | - |
| `cause.artifact` | `RelPath` | required (exactly one key of cause) | - |
| `cause.reason` | `ReasonCode` | required (exactly one key of cause) | - |
| `cause.legacy` | `ReasonCode` | required (exactly one key of cause) | - |
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
| `value.number` | `Finite` | required (value.kind = number) | - |
| `value.count` | `integer` | required (value.kind = count) | - |
| `value.den` | `integer` | required (value.kind = fraction) | - |
| `value.num` | `integer` | required (value.kind = fraction) | - |
| `value.text` | `string` | required (value.kind = text) | - |
| `value.counts` | `map of integer` | required (value.kind = counts) | - |
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
| `outcome.text_empty` | A `note` of a piece of evidence, or a `calibration_ref` of a probability, has no character. | - | plugin |
| `outcome.value_invalid` | A value of an outcome record has the wrong form for what it names, such as a name with a space. | - | plugin |
| `outcome.weight_above_one` | The numerator of a weight exceeds its denominator. | - | plugin |
| `outcome.weight_mismatch` | The weight of an outcome does not fit its status. | - | plugin |
| `outcome.weight_not_allowed` | A not-measured or not-applicable outcome carries a weight. | - | plugin |
| `outcome.zero_weight_without_legacy` | A weight of 0/0 comes without the legacy cause. | - | plugin |
<!-- /generated:refusals -->

## Notes

- An evaluator gives `pass` and `fail` no `cause`, and `blocked`, `not_measured` and `not_applicable` exactly one: a single key among `expectation`, `artifact` and `reason`. [checked: candler-evaluator@0.1.0]
- `not_measured` and `not_applicable` carry no `weight`. `num` never exceeds `den`; a `pass` needs `num` equal to `den`, a `fail` a `num` below it, a `blocked` a `num` of 0. [checked: candler-evaluator@0.1.0]
- The helper sends `descriptive` for `role` and zeros for both digests as placeholders of the fields Candler fills. [checked: candler-evaluator@0.1.0]
- Candler's reader ignores an unknown key of an outcome record; the helper's model refuses it. [checked: schemas@0.6.0]
