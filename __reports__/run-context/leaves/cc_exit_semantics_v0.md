# cc_exit_semantics: which exit Claude Code's Bash result states

Case **(b)**: Claude Code writes `is_error: false` for three commands that exit 1, and a field tells them from a true exit 0.
The reader now gives no exit for such a result. OpenCode's reader was right and is unchanged.

## Probe

Question: what do real Claude Code 2.1.311 and real OpenCode 2.2.7 write for `grep` with no match, `diff` of two different
files and `test -f /missing`? Rig: `parity_e2e.rs` (`start_fake`, `base_experiment`, `seal`), fake model `g1`, one run per
harness, commands `echo hello | grep zzz`, `printf a > a.txt; printf b > b.txt; diff a.txt b.txt`, `test -f /missing`,
and the control `echo fine`. No token was spent. `g1` took exactly two commands; it now takes two or more
(`assets/docker/fakemodel/fakemodel.py`), so the recording is repeatable (see `tests/fixtures/README.md`, "Record a stream again").
The probe's test code is deleted; the recordings are the fixtures `streams/shell_exit_semantics/` of both crates.

### Claude Code (`agent_stream.jsonl.gz`, lines 3, 5, 7, 9; `jq` over the `user` lines)

| command | `is_error` | content | `tool_use_result` |
|---|---|---|---|
| `grep` no match | false | `(Bash completed with no output)` | `{stdout:"", stderr:"", interrupted:false, isImage:false, returnCodeInterpretation:"No matches found", noOutputExpected:false}` |
| `diff` differs | false | the diff text | same keys, `returnCodeInterpretation:"Files differ"` |
| `test -f /missing` | false | `(Bash completed with no output)` | same keys, `returnCodeInterpretation:"Condition is false"` |
| `echo fine` (exit 0) | false | `fine` | `{stdout:"fine", stderr:"", interrupted:false, isImage:false, noOutputExpected:false}`, no `returnCodeInterpretation` |

No field and no text carries the number 1. What distinguishes the three is the key `returnCodeInterpretation`, which
the exit-0 result does not have.

### OpenCode (`export.json`, `state.metadata.exit`; the same in `stdout.jsonl`)

| command | `status` | `metadata.exit` | content ends |
|---|---|---|---|
| `grep` no match | completed | 1 | `Exit code: 1` |
| `diff` differs | completed | 1 | `Exit code: 1` |
| `test -f /missing` | completed | 1 | `Exit code: 1` |
| `echo fine` | completed | 0 | no exit line |

OpenCode states each code, and its reader (`metadata.exit`) already gives code 1, 1, 1 and 0. No change there.

## Decision

Case (b). `exit_of` (`crates/candler-harness-claude-code/src/parse.rs`) gives no exit to a non-error result whose
`tool_use_result` has `returnCodeInterpretation`: the harness states a nonzero exit without a number, and Candler never
infers one. A non-error result without it still gives code 0, with the existing exceptions (`backgroundTaskId`,
`timedOutAfterMs`, `interrupted`). Candler writes no code 1 for these: the interpretation text says "No matches found",
not 1.

The reader's output changes for a recorded kind of result, so the reader version is 0.4.0. `READER` and `READER_VERSION`
are one macro invocation, `reader_version!(0, 4, 0)`, so the text and the version cannot differ (the OpenCode crate's
duplicates are another leaf's).

### Finding #5 (one decision, at the source): not possible as written

The spec asks `exit_of` to gate on the call's tool. At parse time the reader sees one `tool_result` block and not the
call it answers: the call is in an earlier line and its session is known only when `Registry::emit` runs (the same
call id can be held by a child session). So the tool is knowable in `emit` and nowhere earlier. The decision is made
once there, and `exit_of` no longer invents anything: it reads what the result states (code from `Exit code N`, code 0
for an unflagged non-error result, none otherwise). The clearing in `emit` is not dead: an error result of an MCP tool
whose text starts `Exit code 4` would otherwise carry a code. Spec defect, reported; no code works around it.

## Guard

`guard_reader::expected_facts` (`exits` rows of `shell_exit_semantics/expected.toml`, written from `jq`, never from the reader):

| call | tool | row |
|---|---|---|
| `toolu_fake_g1_0` grep | Bash | no code |
| `toolu_fake_g1_1` diff | Bash | no code |
| `toolu_fake_g1_2` test | Bash | no code |
| `toolu_fake_g1_3` echo fine | Bash | code 0 |
| `toolu_fake_search` | ToolSearch | no code |
| `toolu_fake_stamp` | mcp__parity__stamp | no code |

OpenCode: `guard_opencode_driver` gains the row `shell_exit_semantics`: code 1, 1, 1, 0 for the shell calls and none for `stamp`.

Bites (both seen red):

1. The old rule (a non-error result gives code 0), on the fixture before the fix:
   `Error: "shell_exit_semantics: exit of the result of Bash call toolu_fake_g1_0 in 654ac8db-b358-475c-94fc-31ff9f84be61 is [Some(0)], want None"`
2. The gate in `Registry::emit` off (a non-Bash result keeps its exit):
   `Error: "api_error: exit of the result of Edit call toolu_01DvCBeLjPX4jbFuFuTJRphU in 50b18959-0cb5-4deb-b336-1c8c57970383 is [Some(0)], want None"`

## Docs

`parse.rs` (`exit_of`) and the crate README ("What the reader reads") state the rule. The crate CHANGELOG is written by
release-plz from commits and was not edited. `tests/fixtures/README.md` says how to record the streams again, with the
fake-model id `g1`, its commands and the rig; for `shell_exit_stderr_timeout` it gives the shape of the uncommitted script
`sh1` (three `Bash` calls in one message, ids, commands and the `timeout` field), since the probe report holds enough.

## Not covered

- A real exit 0 is still read from the absence of `returnCodeInterpretation`, as the harness states no number for it.
  A Claude Code release that adds an interpretation to other commands would drop their exit to none (safe), not to a wrong code.
- Other commands Claude Code's semantics list (`rg`, `find`, `[ ]`) were not recorded; they share the field.
- A non-Bash tool whose error text starts `Exit code N` is cleared by `emit`; no recorded stream holds one.
- The OpenCode crate's duplicated reader-version constants are untouched.

## Docker

Before: 20 images, 3 containers. After: the same. The fake-model image the probe built was removed by the rig's own
cleanup by id; the two agent images it ran (`sha256:57f339fc3580...`, `sha256:e287badacc75...`) were already present and
are left. No image of mine remains; `candler-gateway:latest` untouched.
