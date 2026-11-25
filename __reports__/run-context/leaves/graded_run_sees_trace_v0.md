# Leaf report: graded run sees trace

Leaf: `__roadmap__/run_context/readers/e2e/graded_run_sees_trace.md`. Branch `leaf/run-context/graded_run_sees_trace`, cut from `feat/run-context` at `9d337922`. No pull request was opened and nothing was pushed; publishing waits on the maintainer's go.

## The guard

`guard_cli_e2e::graded_trace_e2e::graded_run_sees_trace`, in `crates/candler/tests/it/guard_cli_e2e/graded_trace_e2e.rs`, `#[ignore]` like the parity guard and named in `container-e2e.yml`'s expected-guard list.

| Part | What it is |
|---|---|
| Rig | The parity guard's: real Claude Code and OpenCode in their base images, the production gateway, the scripted fake model. The fake model's start, the shared experiment and the sealing of one run are now functions both guards call (`start_fake`, `base_experiment`, `seal`). |
| Script | New fake-model id `g1`: the shell command `printf 'graded-answer-3f9a\n' > answer.txt` (exit 0), `exit 3`, then the `parity` server's `stamp` tool. The commands reach the fake model from the guard (`FAKE_G1_COMMANDS`), so the guard holds them once. |
| Kind | `tests/fixtures/plugins/graded-trace/`: `graded_run`, declaring `needs.trace = true` and an artifact glob, reading the trace with the helper's `read_trace`. It pairs a call's `argv` with its result's `exit` by `span`. It passes only if each expected `(argv, exit)` pair, the origin `{kind: mcp, server: parity, tool: stamp}` and the served `answer.txt` with its text are each held exactly once. Equality on whole values, no substring. |
| Oracle | The script's own commands, read back from the requests the fake model recorded; the exit statuses a shell gives them; the server and tool the script calls. |
| Grading | `candler evaluate` over the sealed run's store; the outcome record must be one `pass` and the report must refuse nothing. |

How each real binary reports a shell call: both give `argv = [input.command]`, the whole command string as one element. Claude Code's tool is `Bash`; OpenCode's is `shell`.

## Bites

Each applied on a scratch branch from the guard's commit, one at a time, run with `--run-ignored only`, then the branch deleted.

| Bite | Result | Failing message |
|---|---|---|
| (a) `assemble` serves no trace (`if needs.trace` made never true, `assemble.rs`); applied twice, again after the fix round with the same result | both rows red | `claude-code: the kind did not pass once: [{... "evidence": [{"note": "the context serves no trace"}], "status": "fail" ...}]`, and the same line for `opencode` |
| (b) the OpenCode reader writes no `exit` (`export.rs`, `exit` replaced by none on the result) | OpenCode row red, Claude Code's green | `opencode: the kind did not pass once: [{... "note": "([\"printf 'graded-answer-3f9a\\n' > answer.txt\"], {'kind': 'code', 'code': 0}) is held 0 times among []; (['exit 3'], {'kind': 'code', 'code': 3}) is held 0 times among []" ... "status": "fail"}]`; no `claude-code:` line |

## Local run

On the tip, `cargo nextest run -p candler --test it -E 'test(/^guard_cli_e2e::/)' --run-ignored only --test-threads 1` gave:

```
PASS [  24.728s] (1/4) candler::it guard_cli_e2e::graded_trace_e2e::graded_run_sees_trace
PASS [ 105.989s] (2/4) candler::it guard_cli_e2e::i8_e2e::i8_canary_never_reaches_the_agent
PASS [  25.915s] (3/4) candler::it guard_cli_e2e::kill_e2e::b40_kill_all_destroys_lost_jobs_containers
PASS [  26.240s] (4/4) candler::it guard_cli_e2e::parity_e2e::parity_one_installed_server_one_traced_call
Summary [ 182.873s] 4 tests run: 4 passed
```

The guard grades both harnesses in the one test, so its single PASS line covers Claude Code and OpenCode. Without `--test-threads 1` the parity and i8 guards failed on other containers of the machine, as the defect below says.

The container-e2e PASS gate is waiting on a pull request.

## What the guard does not cover of P0

The guard asks P0's question: does a kind that needs an artifact and the trace see each shell call's command and exit status and each MCP call's `{server, tool}`, on a real run. It does not cover P0's own kind or P0's suite: not their code, their params, the other kinds a request of that suite sends beside it (the engine serves a request the union of its kinds' needs), their evaluator environment, or the release HTN pins (this guard runs the workspace build). It does not cover HTN's agent task, a real model's tool use, a signalled or timed-out shell call, a denied call, or a shell call inside OpenCode's code mode.

## Spec defects met

- The leaf says `needs.artifacts = ["answer.txt"]`. The registry refuses it: `plugin.need_glob_outside_outputs`, "artifact glob `answer.txt` is not under `outputs/`". The kind declares `outputs/answer.txt`; the served path is `answer.txt`. The `Needs.artifacts` doc said the globs were relative to the run's outputs; it now says each starts with `outputs/`, and the schemas, the helper's models, `architecture.md` and the leaf's gate follow (`docs(schemas)` commit).
- OpenCode's shell tool is `shell`, takes no `description` and refuses extra keys, so the script sends `{command}` alone to both harnesses.
- The parity guard's `ps -aq` equality is false on a machine other sessions share, and under parallel nextest threads. The new guard checks leftovers by its own labels and job ids instead; the parity guard is unchanged in that respect.

## Not covered

- A `seal` panic in the first harness's row stops the second row; the guard is red either way.
- The per-job `candler-env` images are not removed: they are content-addressed and reused across runs by design, as the parity guard does. The guard removes the fake model's image and container by id, and the jobs' containers and networks by label (`Made` holds every job id).
- The guard checks that the product left no container or network of its jobs, before `Made` cleans up. Parity's full `ps -aq` equality stays, since CI runs the group serially.
- A scratch root under `$HOME` that is freshly created may not be mounted in time by Docker Desktop: a run then fails at the start check of the declared servers (`bind source path does not exist`). It passes once the root has been mounted, for instance after a `docker run -v` of it.

## Left

The `container-e2e` PASS line, and the coordinator's verifier. Nothing else.
