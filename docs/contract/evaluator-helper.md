# Evaluator helper

## Surface

The Python package `candler-evaluator`, which an author of an evaluator kind uses to speak the protocol; a kind returns the fields of an outcome record that Candler does not fill.

## Version

<!-- generated:version -->
Wire tag: none.

<!-- versions -->
Package: `candler-evaluator@0.1.0`.
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
| `helper.deadline_out_of_range` | An evaluate request has an item whose deadline_ms is above 2**53 - 1; the helper answers with error -32602. | - | candler |
| `helper.instrument_commit_invalid` | An instrument's git install records a commit_id that is not a full lower-case hex hash. | - | plugin |
| `helper.instrument_install_unrecorded` | An instrument's installation has no direct_url.json, as an install from a package index has none. | - | plugin |
| `helper.instrument_install_unsupported` | An instrument's direct_url.json is neither a git install nor an editable one. | - | plugin |
| `helper.instrument_not_installed` | An instrument names a distribution that is not installed. | - | plugin |
| `helper.instrument_record_malformed` | An instrument's direct_url.json is not a JSON object. | - | plugin |
| `helper.instrument_vcs_unsupported` | An instrument is installed from a version control system other than git. | - | plugin |
| `helper.instrument_version_invalid` | An instrument's installed version is not SemVer 2.0. | - | plugin |
| `helper.line_not_json` | A line Candler sent is not JSON and carries no request id to answer. | - | candler |
| `helper.message_invalid` | A line Candler sent is not a message of the evaluator protocol and carries no request id to answer. | - | candler |
| `helper.method_unknown` | A request Candler sent names a method other than describe and evaluate; the helper answers with error -32601. | - | candler |
| `helper.outcome_cause_invalid` | A cause is not an object with exactly one of the keys expectation, artifact and reason, holding a non-empty string. | - | plugin |
| `helper.outcome_cause_missing` | A blocked, not_measured or not_applicable outcome carries no cause. | - | plugin |
| `helper.outcome_cause_not_allowed` | A pass or fail outcome carries a cause. | - | plugin |
| `helper.outcome_cause_reason_invalid` | A cause reason is not a lower-case code, or is one that only Candler emits. | - | plugin |
| `helper.outcome_evidence_invalid` | The evidence is not a list of objects whose only members are a non-empty ref and a non-empty note. | - | plugin |
| `helper.outcome_key_unknown` | A kind's outcome mapping has a key other than status, value, weight, cause and evidence. | - | plugin |
| `helper.outcome_not_json` | An outcome holds a value that JSON cannot carry, such as NaN. | - | plugin |
| `helper.outcome_status_invalid` | A kind's outcome has a status that is not one of pass, fail, blocked, not_measured and not_applicable. | - | plugin |
| `helper.outcome_type_invalid` | A kind returns something that is neither an Outcome nor a mapping. | - | plugin |
| `helper.outcome_value_invalid` | A value is not an object whose kind is one of number, count, fraction, text and counts. | - | plugin |
| `helper.outcome_weight_invalid` | A weight is not an object with the integers num and den, with den at least 1 and num from 0 to den. | - | plugin |
| `helper.outcome_weight_mismatch` | A weight disagrees with the status: a pass needs num equal to den, a fail a num below den, a blocked a num of 0. | - | plugin |
| `helper.outcome_weight_not_allowed` | A not_measured or not_applicable outcome carries a weight. | - | plugin |
| `helper.plugin_identity_missing` | A plug-in gives no name or version, and names no distribution that supplies them. | - | plugin |
| `helper.plugin_kinds_missing` | A plug-in module defines no KINDS. | - | plugin |
| `helper.request_invalid` | A request Candler sent has an id but fails the protocol's models; the helper answers with error -32602. | - | candler |
<!-- /generated:refusals -->

## Notes
