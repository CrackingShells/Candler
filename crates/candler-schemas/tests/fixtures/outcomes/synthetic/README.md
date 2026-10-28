# Synthetic outcome records

One record for each variant of each member of the outcome record: the five statuses, the three roles, the five value
kinds, the four cause keys, the three evidence forms, weights with and without a legacy cause, a probability, and
instruments with a content digest, a git digest, and a dirty flag. A `gate` or `quality` record has no probability
and no dirty instrument, so the records with those two are `descriptive`.

The identity fields are synthetic: each digest is `sha256:` and the SHA-256 of a label, the request id is the UUIDv7
`01926b3a-7c3e-7a1b-8c4d-5e6f7a8b9c0d`, and the evaluator is `synthetic-eval@1.0.0`.

`tests/outcome_fixtures.rs` checks that together the files cover every status, role, value kind, cause key, evidence
form, weight form, probability and instrument form, and that each one validates against the committed schema, reads,
passes `validate()` and writes back as the same JSON.
