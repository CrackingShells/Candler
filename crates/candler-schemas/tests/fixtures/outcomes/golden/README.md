# Golden outcome records

One outcome record for each row of `../../legacy/golden_rows.jsonl`, in row order: file `NN-<status>-<id>.json`
is row `NN`, counted from 1. Each record is the clean form of its corpus row and of the row's grading entry in
`../../legacy/golden_grading.jsonl`, by the mapping below. `tests/outcome_fixtures.rs` derives each record's
`(status, value, weight, cause, role)` from the raw row through a table of its own and compares it with the file.

## Mapping

| Corpus field | Record field |
|---|---|
| `status`, `role`, `id` | `status`, `role`, `expectation_id`, unchanged |
| `value` integer | `value` `{kind: count}` |
| `value` float | `value` `{kind: number}`, written with the corpus's own text (`0.0`, `1.0`) |
| `value` null | no `value` |
| `score`, `max_score` | `weight {num, den}`; omitted when it equals the status default (pass 1/1, fail 0/1, blocked 0/1); dropped on `not_measured` |
| `blocked_by` of the grading entry | `cause {artifact}` |
| `not_measured` with a `TIER_D_REPLAY: ... reason=<code>` marker | `cause {legacy: <code>}` |
| `not_measured` whose evidence says the mapping declares zero `mappings` entries | `cause {legacy: zero_mappings}` |
| `not_measured` with no readable cause | `cause {legacy: unrecorded}` |
| `pass` with `score` 0 and `max_score` 0 | `weight {0, 0}` and `cause {legacy: zero_weight}` |
| the row's evidence text | not carried: `evidence` is `[]` |

Every record has the evaluator `htn-creator-eval@0.4.2` and one instrument: `htn_creator_scripts`, version
`0.4.2`, digest `git:28be2c158fbc6ee786a789242c2d8fd47bfde9a8`, `dirty: false`.

## Identity fields

A corpus row does not carry the identity fields of a record, so these are synthetic: well-formed and fixed, and
not the digests of any real run or request.

| Field | Value |
|---|---|
| `run.cell` | `sha256:` and the SHA-256 of the row's `cell` directory name |
| `run.replicate`, `run.attempt` | `N` of the row's `run-N`, and 1 |
| `suite_entry_digest` | `sha256:` and the SHA-256 of `golden-suite-entry/<id>` |
| `context_digest` | `sha256:` and the SHA-256 of `golden-context/<cell>/<run>` |
| `request_id` | the UUIDv7 `01926b3a-7c3e-7a1b-8c4d-5e6f7a8b9c0d` in every record |
