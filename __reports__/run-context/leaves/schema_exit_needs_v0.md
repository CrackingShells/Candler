# Leaf report: schema_exit_needs (v0)

Branch `leaf/run-context/schema_exit_needs`, cut from `feat/run-context` at `abdf7c22`.

## Spec defects, first

1. **`exit` on a `tool_call` line is not refused by the Rust reader, and the base does not refuse a `tool_result` with `exit` either.** The leaf says both are refused as an unknown field. `__design__/architecture.md` §Wire encodings, row "Unknown fields", says readers of the records Candler writes ignore unknown fields so an older reader reads a newer record, and `KnownKind` has no serde `deny_unknown_fields` (only the schema has `additionalProperties: false`). On the base the line with `exit` reads and the `exit` is dropped; the guard failed there as "not written back as read" and, in its final order, as "`{"code":3}` is accepted". The guard therefore asserts the refusals that hold (a bare `{code}`, a string, `null`) and omits the `tool_call` row. Meeting the row would mean refusing unknown keys on every event, which reverses that rule; it is the lead's call.
2. **`Needs` now refuses an unknown key in serde** (`deny_unknown_fields`), which the gate "needs that carry an unknown key are invalid, both readers alike" requires: on the base the Rust reader ignored an unknown key in `needs` (`needs_unknown_key.json` was accepted), so a plug-in's misspelt need would be silently unserved. This departs from the blanket "Unknown fields" sentence for a describe result; AGENTS.md's one-reader rule supports it. Removing the attribute is one line if the lead rules otherwise.

## Guards, each seen red on the base and under its bite

| Guard | Base failing message | Bite and its failing message |
|---|---|---|
| `guard_trace_wire::a_tool_result_carries_its_exit_as_the_process_exit_object_and_nothing_else` | `{"kind":"tool_result","is_error":true,"exit":{"code":3}} is accepted` (first version of the test: `...exit":{"kind":"code","code":3}} is not written back as read`) | `exit` typed `Option<Value>`: `{"kind":"tool_result","is_error":true,"exit":{"code":3}} is accepted` |
| `guard_rpc_wire::the_envelope_accepts_valid_messages_and_refuses_single_faults` (vectors `response_describe_needs_raw_manifest`, `needs_raw_not_boolean`, `needs_unknown_key`) and `test_guard_wire_vectors.py` | Rust: `needs_raw_not_boolean.json is accepted`; Python: `response_describe_needs_raw_manifest.json` refused (assert `[...] == []`) | `raw` with a deserialiser that takes any value: `needs_raw_not_boolean.json is accepted`; no `deny_unknown_fields`: `needs_unknown_key.json is accepted` |
| `guard_context_digest::the_digest_of_a_context_adds_a_key_for_each_served_part_and_only_for_those` (rows of the new `context_digest_served.toml`) | `a context: Error("invalid type: map, expected a string")` | trace key left out when served: `assertion left == right failed: a served trace (input)` |
| `guard_context_digest::context_digest_follows_the_oracle_and_ignores_paths` (the four existing rows, unchanged) | passes on the base | trace key always added: `empty lists and empty metadata (input)` fails (the new rows fail too) |

The helper's `test_guard_isolation.py` checks the invalid vectors with an envelope-only `is_frame`; it now skips `needs_*` vectors, whose fault that check cannot see.

Existing digest rows: `context_digest.toml`, `context_digest_runs.toml` and the other generated files are byte-identical after the generator was extended (`gen_vectors.py --check` regenerates them). The new rows are a new file, `context_digest_served.toml`, five contexts from `gen_vectors.py` over python `rfc8785` (ASSUMED note 14 gives the spelling: `trace` and `manifest` are the digest string, `raw` is `[{stream, sha256}]` sorted by stream, each only when served). The old test of `context_digest_follows_the_oracle_and_ignores_paths` kept its claim for root and absolute paths and lost its `trace` and `raw` rows, because those parts now enter the digest; the new test covers them.

## Marker

`normalised-trace/1` stays. `__design__/architecture.md` §Wire encodings: "Absent vs null" (an optional field is omitted when absent), "Rust shape" (a new optional field is a minor in Rust as in the schema) and "Version marker" (the marker is the `schema` tag of the header, which names the file's shape family, not each field) say an optional field does not move it; the §Normalised trace model section names no rule that does. Old traces lack `exit` and read unchanged.

## Type of `{path, abs, sha256}`

`ContextArtifact` is reused for `context.trace` and `context.manifest`; only its doc was reworded from "artifact" to "stored file of a run" (its fields say path, abs, sha256 and are true for all three). A rename to a new shared name would have churned the schema definition, the generated models and every caller for no change in shape. `ContextRaw` keeps its own `path` (absolute) and gains `sha256`.

## Commits and builds

| Commit | Builds |
|---|---|
| `test(schemas): state the exit field, the new needs and their digest keys` | yes (guards red) |
| `feat(schemas)!: carry a shell call's exit and the stored run's needs` | schemas crate yes; workspace no: the two readers still call `tool_result` with two arguments. No API added only for that |
| `refactor(adapter-claude-code): pass no exit to the tool result` | workspace builds only after the next commit |
| `refactor(adapter-opencode): pass no exit to the tool result` | yes, from here the workspace builds |
| `style(adapter-claude-code): wrap the tool result call as rustfmt does` | yes (the commit above it was not formatted) |
| `feat(evaluator-helper): read the trace of a context as the object it is` | yes |
| `docs(evaluator-protocol): regenerate the needs and the context sections` | yes |
| `test(schemas): check the refused exit rows before the kept ones` | yes |
| this report | yes |

## Checks on the tip

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, `cargo nextest run --workspace` (735 passed, 32 skipped); helper `uv run pytest` (12 passed), `uv run ruff check`, `gen_models.py` leaves no diff; `scripts/contract_versions.py --check` exits 0; `git grep -n confirm_ -- crates python` prints nothing. `container-e2e` was not run (CI).

## Left for other leaves, and observations

- `crates/candler-core/src/evaluation/engine/run.rs` unions the needs of the sent entries field by field and does not copy `raw` or `manifest`; `serve_context` must fold them, or a declared need is dropped silently.
- The contract page's Notes are unchanged (the page is at its 120-word budget); the new needs, `exit` and the `is_error` caveat reach the schema descriptions, not the page. Whether a Note should state them, at the price of an older one, is open.
- The generated Python `Needs.raw` and `Needs.manifest` are typed `bool | None` with default `False`, so the helper's model takes `null` where the Rust reader and the schema (`type: boolean`) refuse it. No vector tests it.
- The helper's new API (typed trace, manifest, raw digests) is `helper_context`'s; here `RunContext.from_wire` takes the trace's `abs` and nothing else changed by hand.
- Spend was not measured from transcripts.
