# Claude Code reader fixtures

Eleven streams feed `guard_reader`. Each lives in `streams/<name>/` with `expected.toml`
(hand-written from `jq` over the raw lines, never from Candler code) and `usage_totals.toml`
(from `usage_totals.jq`).

| stream | source |
|---|---|
| `multi_init`, `subagent_denials`, `api_error` | golden suite `htn-creator-eval` 0.4.2, copied byte for byte |
| `mini_clean`, `mini_incomplete`, `mini_zero_mappings` | the `candler-core` mini-corpus, named by relative path in `expected.toml`; no copy here |
| `shell_exit_stderr_timeout`, `shell_exit_semantics` | real Claude Code 2.1.311 on the parity rig; "Record a stream again" below |
| `synthetic_hooks`, `synthetic_mcp`, `synthetic_orphans` | `make_synthetic.py`, from Claude Code's documented stream-json shapes |

`PROVENANCE.toml` lists the sha256 of every file here except itself.
`guard_fixture_provenance` recomputes each hash and refuses an unlisted file.

## Regenerate

```sh
# the three golden streams (read only golden directory)
uv run select_streams.py "$GOLDEN" --write
# the three synthetic streams
python3 make_synthetic.py
# usage totals of one stream
gzip -dc streams/<name>/agent_stream.jsonl.gz | jq -nrR -f usage_totals.jq > streams/<name>/usage_totals.toml
```

## Record a stream again

`shell_exit_stderr_timeout` and `shell_exit_semantics` are real recordings of Claude Code on the parity rig of
`crates/candler/tests/it/guard_cli_e2e/parity_e2e.rs`: the production gateway sidecar in front of the scripted
fake model `assets/docker/fakemodel/fakemodel.py`, `bypassPermissions`, one cell. No token is spent. Docker
must be running; the agent image is the one `candler run` builds for the cell.

1. Write a temporary `#[ignore]` test in `crates/candler/tests/it/guard_cli_e2e/` (its own `#[path]` module in
   `guard_cli_e2e.rs`; `graded_trace_e2e.rs` is the model). It calls `start_fake(&real, &mut made, label,
   &[format!("FAKE_G1_COMMANDS={json}")])`, builds the experiment with `base_experiment("g1", prompt)`, replaces
   `@HARNESS@` by `claude-code`, calls `seal(...)` and copies the sealed run's directory out (`cp -R run_dir
   <scratch>`) before the `World` drops. Run it with
   `cargo nextest run -p candler --test it -E 'test(<name>)' --run-ignored only --test-threads 1`. Delete the
   test when the files are copied.
2. The fake-model id is `g1`: it answers a first prompt with a `Bash` call for each command of
   `FAKE_G1_COMMANDS` in turn (two or more), then `ToolSearch` and the call of the tool `stamp` of the MCP
   server `parity`, then "done". `shell_exit_semantics` used
   `["echo hello | grep zzz", "printf a > a.txt; printf b > b.txt; diff a.txt b.txt", "test -f /missing", "echo fine"]`.
3. The fixture is the run's `raw/stdout.jsonl.gz`, one raw frame's text per line:
   `gzip -dc <run>/raw/stdout.jsonl.gz | jq -r 'select(.kind == "frame") | .data.text | rtrimstr("\n")' | gzip -n -9 > streams/<name>/agent_stream.jsonl.gz`.
   Then write `expected.toml` by hand from `jq` over those lines, make `usage_totals.toml` as above, and list
   all three files in `PROVENANCE.toml`.

`shell_exit_stderr_timeout` was recorded with an earlier fake-model script `sh1`, which was not committed and
cannot be run from the repository. Its shape, to add again to `fakemodel.py` if that stream must be
re-recorded: a first prompt answered with one assistant message holding three `Bash` `tool_use` blocks,
`toolu_fake_sh_exit3` `{command: "exit 3"}`, `toolu_fake_sh_stderr` `{command: "echo oops >&2; exit 2"}` and
`toolu_fake_sh_timeout` `{command: "python3 -c 'import time; time.sleep(30)'", timeout: 1000}` (the tool's own
`timeout` field, in milliseconds; a bare `sleep 30` is refused by Claude Code), any later request answered
"done". The rig is the same; `g1` cannot stand in for it because it sends one call per request and sets no
`timeout`.
