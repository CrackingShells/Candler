# Probes P2 and P6 (commit a26fe53)

## P2: can `run-manifest/1` record a raw-trace import today? No.

Evidence (`crates/candler-schemas/src/execution.rs`, `ImportRecord`, lines 2378-2397; committed `schema/run-manifest.schema.json`, `$defs.ImportRecord`):
- `base_commit: GitCommit` is required (execution.rs:2382). The schema's `required` list is `format, base_commit, legacy_integrity, sources, requests`. `GitCommit` is 40 or 64 lowercase hex (schema `$defs.GitCommit`). The drafters are right.
- `legacy_integrity: JsonValue` is also required (execution.rs:2384). It may be any non-null JSON. A raw trace has no integrity block, so this blocks the import as well. The brief did not name it.
- Already fine: `rules: Option<Digest>` is optional with the `present` pattern (execution.rs:2392-2397). `sources` and `requests` are plain `Vec`s, so they may be empty. A raw trace can list its files in `sources`, as `{path: RelPath, sha256}`, with `requests: []`.
- `format: NameToken` needs no change. A label such as `claude-code-trace@1` fits `name@token`.

Minimal changes, to go in the single `feat(schemas)` commit:
1. `base_commit: Option<GitCommit>`, with `default`, `skip_serializing_if = "Option::is_none"` and `deserialize_with = "present"`, copied from `rules`. The schema moves it out of `required`.
2. `legacy_integrity: Option<JsonValue>`, same attributes. An explicit `null` stays refused by `present`.
3. `ImportRecord::new` (execution.rs:2400) takes `(format)` only. Add `.base_commit(..)` and `.legacy_integrity(..)` setters, like `.rules()`.
4. Follow-on: `crates/candler-core/src/import/assemble.rs:406`, which reads `record.legacy_integrity`. The schema drift test and the wire guards also change.

## P6: does `evaluation.outcome` carry what `watch --live` needs? Half.

Evidence:
- The payload is `{record: OutcomeRecord}` (execution.rs:3452). The record carries `run`, `expectation_id`, `evaluator`, `request_id`, `status`, `weight`, `evidence` and more (outcome.rs:804-820). The "done" count is the number of `evaluation.outcome` events, and "which run and expectation" is already in each one. That is enough for MCP too.
- Nothing carries the planned total. `job.started` has an empty payload `{}` (execution.rs:3418), and `JobRecord` holds only the id, supervisor, `spec_digest` and state. `JobSpec` (`candler-core/src/jobs/model.rs:17`) holds experiment run plans only, with no evaluate plan.
- Nothing emits `evaluation.outcome`. `git grep` finds the constructor only in schemas and its tests, and no job body in candler-core uses it. The evaluate job has not been built.

Proposal, as one additive schemas change: a new event kind `evaluation.planned` with payload `{outcomes: u32}`, emitted once at the start of an evaluate job. `outcomes` is the count of (run, expectation) records planned, including ones already stored. That adds a 14th kind in `KNOWN_EVENT_KINDS` (execution.rs:3527, `[&str; 13]`). The alternative is an optional `planned` on `job.started`.

Reading the total from the evaluate job's own spec file would avoid a schemas change, but MCP could not draw progress from events alone.

## Decisions (M3 coordinator, 2026-11-11)

- **P2: branch B.** `contract_wire` makes `ImportRecord.base_commit` and `ImportRecord.legacy_integrity` optional in its single `feat(schemas)` commit. `ImportRecord::new` keeps its three-argument signature, so `candler-core` compiles unchanged within the schemas-only commit. That departs from proposal item 3 above, which would have forced a core edit into the release-triggering commit. A new `ImportRecord::raw(format)` builds a raw-trace origin. A legacy record serialises to the same bytes, so no digest moves. Guard: `c5_`.
- **P6: a new event kind.** `evaluation.planned {outcomes: u32}`, emitted once by the evaluate job after selection and before the first outcome, is part of the same schemas commit (guard `c6_`). `evaluate_body` emits it, and `watch --live` draws `outcomes <n>/<planned>` from it. The alternative, an optional field on `job.started`, was rejected: `job.started` is emitted by the generic job runtime, which M4 also edits.
