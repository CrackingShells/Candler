# Probe P-shell: how each harness reports a failing shell call

Recorded on 2026-11-25 on the `parity_e2e.rs` rig: real Claude Code 2.1.311 and real OpenCode 2.2.7,
each in its agent base image, behind the production gateway sidecar, against the scripted fake model
(model `sh1`, one turn of three shell calls: `exit 3`; `echo oops >&2; exit 2`;
`python3 -c 'import time; time.sleep(30)'` with the tool's own `timeout: 1000`, in milliseconds).
No token was spent. The streams are the harnesses' stdout, one raw frame's text per line.

Fixtures (provenance in each harness's `tests/fixtures/PROVENANCE.toml`):

- `crates/candler-harness-claude-code/tests/fixtures/streams/shell_exit_stderr_timeout/agent_stream.jsonl.gz`
- `crates/candler-harness-opencode/tests/fixtures/streams/shell_exit_stderr_timeout/stdout.jsonl` and `export.json`

Every line below is quoted from those files (line numbers are the fixture's).

| Q | Answer in one line | Decision |
|---|---|---|
| 1 | OpenCode writes `status: completed` with the code in `metadata.exit` | the OpenCode reader takes `exit` from `metadata.exit` whatever the status |
| 2 | Neither gives a code on its own timeout, and the shapes differ | `exit` stays absent on a timeout; one guard, below, that a timeout never reads as code 0 |
| 3 | Yes: `Exit code N` leads the content, stdout empty or not | the Claude Code prefix rule holds; stderr text follows it |

## 1. OpenCode, a nonzero exit

`stdout.jsonl` line 4 (the `tool_use` event; `state.metadata.metadata` is the one that matters, the outer
`metadata` also carries `content`, `providerCall`, `rawInput`):

    {"status":"completed","truncated":false,"exit":3}

`export.json` lines 67 and 83-86 (the same call, the export reader's input): `state.status` is `"completed"` (line 67) and
`state.metadata` holds `"exit": 3` (line 85). `state.error` is absent. The content blocks end with
`"Exit code: 3"`. The plan's expectation (`completed`, `metadata.exit: N`) holds.

Reader rule for `oc_reader_exit`: a `shell` result with `state.metadata.exit` an integer gives `exit = code N`,
whatever `state.status` says (`completed` here). Read it in the export (`state.metadata.exit`); the live
`stdout.jsonl` event nests it one level deeper (`state.metadata.metadata.exit`) and is not the reader's input.
No `exit` key gives no exit (a missing `metadata` is not code 0).

## 3. Claude Code, stderr only (question 3 before 2)

`agent_stream.jsonl.gz` line 6, `exit 3`:

    "content":"Exit code 3","is_error":true   (tool_use_result: "Error: Exit code 3")

Line 7, `echo oops >&2; exit 2`:

    "content":"Exit code 2\noops","is_error":true   (tool_use_result: "Error: Exit code 2\noops")

The prefix holds when stdout is empty and the command wrote only to stderr: the stderr text follows
the first line. There is no numeric field anywhere in either result. OpenCode, for the same stderr-only
call: `stdout.jsonl` line 5 `{"status":"completed","truncated":false,"exit":2}`, output `"oops\n"`.

Reader rule for `cc_reader_exit`: for a `Bash` result with `is_error: true`, the content up to the first
newline matching `^Exit code (\d+)$` gives `code N`, with or without text after it. Content that does not
start so gives no exit.

## 2. A call ended by the harness's own timeout

Claude Code: the timeout does not end the call. `agent_stream.jsonl.gz` line 10:

    "is_error":false, "content":"Command did not complete within its 1s timeout and was moved to the background (ID: bpvpiu488). ..."
    tool_use_result: {"stdout":"","stderr":"","interrupted":false,...,"backgroundTaskId":"bpvpiu488","timedOutAfterMs":1000}

The result is `is_error: false` and carries no code. Line 14, long after, is a separate system line about the
background task: `"summary":"Background command \"outlives timeout\" completed (exit code 0)"` (the sleep ran to
its end). That text belongs to the task, not to the call's result.

OpenCode: the harness kills the command. `stdout.jsonl` line 6 and `export.json` lines 150-153:

    {"status":"completed","truncated":false,"signal":"SIGTERM","timeout":true}

`status` is `completed`, `exit` is absent, `signal` is `"SIGTERM"`, `timeout` is `true`. The content text says
`Exit code: 143 (SIGTERM)`, the harness's rendering for the model, not a field.

Decision: neither harness gives a code field on a timeout, so `exit` stays absent on a timeout and the
plan's "no timeout guard" holds in the sense that no code is read. Two guards the readers still need, because
the shapes are traps for the two rules above:

- `cc_reader_exit`: the plan's rule "`is_error: false` gives code 0" must not fire on this result.
  A result whose `tool_use_result` has `backgroundTaskId` or `timedOutAfterMs` gives no exit.
- `oc_reader_exit`: a result with no `metadata.exit` gives no exit even when `status` is `completed`.
  The `signal` and `timeout` keys are the harness's own report; the plan's rule that Candler never infers a
  signal from a code stays, and whether to feed `signal: "SIGTERM"` to the `signal` variant of `ExitStatus`
  is the reader leaf's call (it is a harness-reported name, not an inference).

## Other things seen

- Claude Code refused a bare `sleep 30` itself (`<tool_use_error>Blocked: standalone sleep 30 ...`); the
  recorded call uses a Python sleep. A reader meets that text as an `is_error: true` result with no
  `Exit code` prefix, and it gives no exit.
- The Claude Code stream holds two `init` and two `result` lines in one session: the background task's end
  started a second turn. The committed `expected.toml` (one session, `completed`) and `usage_totals.toml` keep
  `guard_reader` green; the reader leaf adds its `exits` rows.
- OpenCode's stdout holds one `retry` line (a 429 from the gateway on the first model request). The retry
  succeeded; it does not touch the shell calls.

## Deviations

- The two commit subjects are shortened to 72 characters, which the commit hook allows: Step 1 is
  `test(trace-model): record each harness's failed and timed-out shell call` and Step 2 is
  `reports(trace-model): how each harness reports a failed shell call`.
- The fake-model script `sh1` is not committed: it was added to `assets/docker/fakemodel/fakemodel.py` for the
  run and reverted in Step 2; its text is in each `PROVENANCE.toml` entry. `git diff feat/run-context --stat --
  crates/candler/tests assets` prints nothing at the tip.

## Docker

Before: 19 images, 3 containers. After: no container of mine remains (`docker ps -aq` is as before); the fake-model
image the test built was removed by the test's own cleanup by id. The two agent images the runs used
(`sha256:57f339fc3580...` for Claude Code, `sha256:e287badacc75...` for OpenCode) were already present and are
left. `candler-gateway:latest` now resolves to `5913b0aa1289` (created 2026-11-17), where it resolved to
`17f734223887` before the run, which is no longer present: the product's own build step rebuilt that shared
tag, so it is not removed here.
