# Leaf report: helper_context

An earlier attempt by another model wrote nothing, so this branch (`leaf/run-context/helper_context`, from `feat/run-context` at `5fd60ff0`) is a fresh start.

## Spec note

The Step 2 commit subject in the leaf is 76 characters and the commit hook allows 72. The subject used is `feat(evaluator-helper): hand a kind the run's parts and a typed trace` (the words "stored" dropped). The commit carries a `BREAKING CHANGE:` footer, because `RunContext.trace` was a `Path` and is now a `ServedFile`.

## The reader's API

`RunContext` carries `trace` and `manifest` as `ServedFile(path, abs, sha256)` or `None`, and `raw` as a tuple of mappings with `stream`, `path` and `sha256` (empty when not served). `candler_evaluator.read_trace(context.trace)` (also a path) yields each line of the trace as the model of its `kind` generated from `normalised-trace.schema.json`, and raises `pydantic.ValidationError` at a line the schema refuses, an unknown field included.

## Guards on the base (Step 1, `9292fd6f`)

| Test | Failing message on the base |
|:--|:--|
| `test_a_context_keeps_the_trace_raw_and_manifest_with_their_digests` | `AttributeError: 'RunContext' object has no attribute 'manifest'` |
| `test_a_context_without_the_parts_has_none_and_nothing` | `AttributeError: 'RunContext' object has no attribute 'manifest'` |
| `test_the_reader_yields_typed_records_with_exit_origin_and_argv` | `ImportError: cannot import name 'read_trace' from 'candler_evaluator'` |
| `test_the_reader_takes_the_served_trace_of_a_context` | same `ImportError` |
| `test_the_reader_refuses_a_line_with_an_unknown_field` | same `ImportError` |
| `test_a_describe_result_never_carries_null_for_an_unset_need` | passes on the base (see below) |

The describe guard is green on the base: the helper already strips `None` twice (`exclude_none=True` in `loop.py` `describe` and in `frames.py`), so no `null` reaches the wire today. It is a guard against a regression, and its bite shows it can fail. Its oracle is the shared vectors: `valid/response_describe_needs_raw_manifest.json` (the wire form of `raw`/`manifest` as booleans) and the request `valid/request_describe.json` that drives the helper process.

## Bites (Step 3)

| Bite (scratch branch, one wrong change) | Failing message |
|:--|:--|
| `from_wire` drops `manifest` (`manifest=None`) | `test_a_context_keeps_the_trace_raw_and_manifest_with_their_digests`: `AssertionError: assert (ServedFile(path='trace.jsonl', ...) is not None and None is not None)` |
| the generated trace models accept unknown fields (`extra="allow"`) | `test_the_reader_refuses_a_line_with_an_unknown_field`: `Failed: DID NOT RAISE ValidationError` |
| the writer emits `null` for an unset need (`exclude_none` removed from both dumps, `loop.py` and `frames.py`) | `test_a_describe_result_never_carries_null_for_an_unset_need`: `AssertionError: assert None not in dict_values([[], [], None, [], None, [], False])` |

Removing `exclude_none` from only one of the two dumps leaves the guard green: the other still strips the `None`. Both scratch branches are deleted.

## Checks on the final commit (from `python/candler-evaluator`)

- `uv run pytest`: 18 passed.
- `uv run ruff check`: all checks passed; `uv run ruff format --check`: 27 files already formatted.
- `uv run --frozen python scripts/gen_models.py`: no diff (`git status` clean after it); `normalised_trace.py` is generated from `normalised-trace.schema.json`.
- `python3 scripts/contract_versions.py --check` after the bump: the blocks match `0.2.0`.
- `git grep -n confirm_ -- crates python`: prints nothing.
- The contract page's Notes hold 119 words (limit 120), each bullet stamped.
- No Rust build was needed and none was run.

## Left

- The mandatory Sonnet review (a wire context or trace line the helper reads differently from the Rust reader) is for the coordinator to dispatch.
- Not checked against the Rust reader: an empty line in a trace file raises `ValidationError` in `read_trace`; whether the Rust reader tolerates one is not verified.
- `candler_evaluator.__init__` also exports `ServedFile` and `read_trace`; the generated `Needs.raw`/`Needs.manifest` stay `bool | None`, as the schema generator emits them.
- The tag `candler-evaluator@0.2.0` is not cut.
