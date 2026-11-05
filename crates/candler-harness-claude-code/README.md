# candler-harness-claude-code

The trace reader for Claude Code. It implements `candler_harness::reader::TraceReader` for the
stream-json output of Claude Code: it reads the frames of one
`agent_stream.jsonl` and returns the sessions and events of the normalised trace
(`normalised-trace/1`, [R01 §Normalised trace model](../../__design__/architecture.md)).

## Public items

| Item | Holds |
|---|---|
| `ClaudeCodeReader` | The reader. `ClaudeCodeReader::new()` returns `Result<_, ClaudeCodeError>`; the type has no `Default`. It frames by line. |
| `ClaudeCodeError` | Why a reader cannot be built: its identity is not a valid `name@version`. |
| `READER` | The reader identity as text, `claude-code-stream-json@<version>`. The version changes whenever the reader's output does. |
| `SUPPORTED` | The Claude Code versions the reader supports, as a SemVer requirement. |
| `detect` | A public module: `detect::detect` recognises a supported stream from its first line and returns its format version and a confidence. |

The modules `parse` (one line into events), `sessions` (roots, subagent children, event numbering
and how each session ended) and `usage` (token counts and their sums) are private.

## What the reader reads

- Session roots and subagent children, each with its own event numbering.
- Tool calls and results. A result the harness refused carries a denial whose reason is the
  `non_execution_kind` of the line's `tool_result_meta`.
- Usage per message and, from the result line, per session with its cost.
- Content blocks it has no event for, kept whole in a notification.
- A stream with no init line, or with frames before any session, ends in `ReaderError::Incomplete`.

## Guard files

| File | Pins |
|---|---|
| `tests/guard_reader.rs` | the reader against the streams of `tests/fixtures/streams/`: facts, citation, usage, the version table, replay, and the ratchet of trace digests |
| `tests/guard_fixture_provenance.rs` | every fixture file is listed in `tests/fixtures/PROVENANCE.toml` with a hash that recomputes |

`tests/fixtures/` holds the streams, `PROVENANCE.toml`, the expected digests in `digests.toml`, the
scripts that select or write the streams, and its own README.

```sh
cargo nextest run -p candler-harness-claude-code
```

## Use

`candler-core` takes this crate as a dev-dependency only: the core's seams and import guards feed a
real stream to the reader, and the core's library code sees only the `TraceReader` trait.
