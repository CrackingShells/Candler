# Leaf report: Claude Code reader exit status

The Claude Code reader sets `ToolResult.exit` on a `Bash` result from what Claude Code wrote, in
`crates/candler-harness-claude-code`. Branch `leaf/run-context/cc_reader_exit`, two commits on `feat/run-context`.

## The exit rule as implemented

`parse.rs::exit_of` reads, from the result block and its line: the block's `is_error`; the first line of the first
text content; and `backgroundTaskId`, `timedOutAfterMs` and `interrupted` in the line's `tool_use_result`.

1. A denial (`tool_result_meta` non-execution entry for the call) gives no exit.
2. A result whose `tool_use_result` has `backgroundTaskId` or `timedOutAfterMs`, or `interrupted: true`, gives none.
3. An error result whose first text line is `Exit code ` followed only by decimal digits that fit an `i32` gives `code N`;
   any other error text gives none.
4. A result that is not an error gives `code 0`.
5. `sessions.rs::Registry::emit` clears the exit unless the call that this result answers, found by the session that
   holds the result and the call id, is named `Bash`. The name is stored when the call is emitted.

The prefix is read only on an error result: a successful command that prints `Exit code 5` on stdout is code 0.
The reader never reads the system line about the background task and never infers a signal.
`READER` and `READER_VERSION` move to `0.3.0` since the output changed.

## The jq oracle

Every expected `exits` row in the seven real fixtures' `expected.toml` comes from this, run over the decompressed raw stream
(`gzip -dc <stream> | jq -n -r -f oracle.jq | jq -n -r -R --arg root <first session id> -f rows.jq`).

```jq
# oracle.jq: one row per tool_result: session (root, or root/<parent call>), call id, call name, is_error,
# first line of first text content, background flag, non-execution kind
[inputs | select(.type=="assistant" or .type=="user")] as $lines
| ([$lines[] | .message.content | if type=="array" then .[] else empty end | select(.type=="tool_use") | {key:.id, value:.name}] | from_entries) as $names
| $lines[] | . as $l
| ($l.message.content | if type=="array" then .[] else empty end | select(.type=="tool_result")) as $r
| [ ($l.session_id + (if $l.parent_tool_use_id then "/" + $l.parent_tool_use_id else "" end)),
    $r.tool_use_id, $names[$r.tool_use_id], ($r.is_error // false),
    (($r.content | if type=="string" then . elif type=="array" then ([.[]|select(.type=="text")|.text]|first // "") else "" end) | split("\n")[0]),
    ($l.tool_use_result | if type=="object" then (has("backgroundTaskId") or has("timedOutAfterMs")) else false end),
    ($l.tool_result_meta // [] | map(select(.id==$r.tool_use_id) | .non_execution_kind) | first) ] | @json
```

`rows.jq` applies the rule to each oracle row (Bash only; background gives none; `^Exit code [0-9]+$` gives N; other error
gives none; non-error gives 0) and keeps every nonzero or background row plus the first row of each distinct shape
(child or root session, tool, is_error, expected value, background, denial). The rows are in
`tests/fixtures/streams/*/expected.toml` under `[[exits]]`; each also names the call's `tool`, which the guard checks.
Real fixtures with rows: `api_error` (Exit code 127, a denial), `mini_zero_mappings` (Exit code 1), `multi_init`
(child-session Bash, exit 1 and 0), `subagent_denials` (child-session Bash, a denial), `mini_clean`, `mini_incomplete`,
and the probe's `shell_exit_stderr_timeout` (`Exit code 3`, `Exit code 2` followed by `oops`, and the timed-out call with
no exit although `is_error` is false). `synthetic_mcp` has one absence row (an MCP tool's error). The other synthetic
fixtures have none.

Beside the fixtures, `expected_facts` runs a literal stream (`exit_join_stream`): one call id is a `Read` in the root
session and a `Bash` in the subagent session; and a `Bash` result that is not an error but reports `interrupted`.
Only absences and the child's code 0 (a code row on hand-built lines that no Claude Code wrote, kept to pin the join).

## Guards on the base (eb83b015, reader passes `None`)

`cargo nextest run -p candler-harness-claude-code -E 'test(expected_facts)'` fails:

```
api_error: exit of the result of Bash call toolu_01NmVTy7CtJXWmkzZhfECk2M in 50b18959-0cb5-4deb-b336-1c8c57970383
  left: [None]
 right: [Some(0)]
```

The test stops at the first failing row. Row classes by what each needs: a `Bash` code 0 (above); nonzero 1 and 127;
the stderr-only `Exit code 2`; a child-session `Bash`; every absence row (denial, `Read`, `Edit`, `Agent`, an MCP tool,
the timed-out call) passes on the base because the base gives no exit. The literal stream's child `Bash` code 0 fails on the base too.

## Bites

Each applied on `scratch/bites` (deleted), one at a time, then reverted. All failures are from `expected_facts`.

| bite | change | failing message |
|---|---|---|
| (a) prefix parse removed | the prefix `Exit code ` made unmatchable | `api_error: exit of the result of Bash call toolu_019oLKsnNZZYDgoAFCpGxN9k in 50b18959-...` left `[None]` right `[Some(127)]` |
| (b) code 0 for every non-error result | the `Bash` name check never clears | `api_error: exit of the result of Edit call toolu_01DvCBeLjPX4jbFuFuTJRphU in 50b18959-...` left `[Some(0)]` right `[None]` |
| (c) join on span without the session | name map keyed by call id only | `shared call id: exit of the result of Read call X in S` left `[Some(0)]` right `[None]` |
| (d) background check removed | `backgroundTaskId` and `timedOutAfterMs` ignored | `shell_exit_stderr_timeout: exit of the result of Bash call toolu_fake_sh_timeout in d5ff5efc-...` left `[Some(0)]` right `[None]` |
| (e) extra: `interrupted` removed | `interrupted` ignored | `shared call id: exit of the result of Bash call I in S` left `[Some(0)]` right `[None]` |

Bite (c) cannot fail on a real fixture: Claude Code's call ids are unique, so span-only and (session, span) agree on every
recorded stream. The literal stream is the only place the two differ.

## What is left

- `tests/fixtures/digests.toml` still names `claude-code-stream-json@0.2.0` and the old digests. No test reads it in this tree
  (the ratchet named in its header and in the fixtures README is not present), so it is untouched and stale; it needs a bless
  when the ratchet returns.
- The fresh Sonnet verifier of Step 3 is the coordinator's to dispatch.
- No signal is read; Claude Code reports none.
- `container-e2e` on the pull request is not run here.
- Workspace suite with `--no-fail-fast`: 732 of 735 pass. `guard_harness_facts::harness_census_equals_cf3de30_on_every_fixture`
  fails on the base too ("no baseline for streams/shell_exit_stderr_timeout"; known, fixed on another branch).
  `guard_supervise::a_pre_stop_command_leaves_nothing_in_its_group` and `guard_wiring::wiring_stop_execs_in_the_container_with_the_observed_id`
  failed under full load and pass alone (0.9 s and 97 s). `fmt --check`, `clippy -D warnings` and `doc -D warnings` pass.
- Gate 4 lists what `parse.rs` reads; the code also reads `interrupted` and the denial meta, as Step 2's text requires.

## Review round

- **Unjoined result kept its exit.** `SpanId` refuses an empty id, `#` and white space, so a raw line can carry
  `"tool_use_id":"no span"`; `span_of` then gives no span and the result kept `exit_of`'s code 0. `Registry::emit` now clears
  the exit of a result with no span too. A literal row (a non-error result with id `no span`, `check_spanless_results`) was
  red on the code before the fix: `Error: "shared call id: a result joined to no call has exit Some(Code { code: 0 })"`.
- **One shell name.** `SHELL_TOOL` (`parse.rs`) is used by the argv rule and by the gate in `sessions.rs`.
- **Only what the gate needs.** `names: HashMap<(usize, String), String>` became `shell: HashSet<(usize, String)>`, filled by
  `Bash` calls only; each remaining clone keeps its `// BORROW:` note.
- **Both join orders.** The shared-id stream has `X` (root `Read` result first) and `Y` (child `Bash` result first, root `Read`
  result after it). `Y` already passed on the code before the fix.
- **Docs.** `emit` and `exit_of` say what they do (no construction detail; non-error result with no flag reads as exit 0);
  the empty-digits case is explicit.
- **Tests.** `ExitRow` holds `&str`, no `// BORROW: owned text` on literals; `check_exits` returns `Err` for every failure.
- **Bite (c) again**, on `scratch/bites` (deleted), set keyed by call id only: `Error: "shared call id: exit of the result of
  Read call X in S is [Some(0)], want None"`.
- Checks: `fmt --check`, `clippy -p ... --all-targets -D warnings`, `nextest run -p ...` (21 of 21), `doc -D warnings`, and
  `git grep -n confirm_ -- crates python` (empty) pass.
