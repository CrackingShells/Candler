# observe, leaf report v0

`candler resolve`, `ls`, `watch` and `ps` over the foreground preflight, the stored runs and the job directory, with one colour rule for every verb. The implementer was Sonnet. Branch `m3/observe` is cut from fe1a942 and holds five commits: the P5 report, then one `feat(cli)` commit per step.

## P5

**Answer: yes, a host-side model-list read.** A preflight can check the gateway's token for free, with no sidecar and no completion request.

- **Request:** `GET <model.endpoint>/v1/models` from the host, with `Authorization: Bearer <value of the secret model.token_secret names>`. It carries no body and asks for no completion.
- **Cost:** a model-list read is a catalogue read, not an inference call, on an OpenAI-style endpoint. The gateway's own readiness probe (`ready_once`, `READY_URL`) makes the same request through the sidecar, and treats 401 as `NotReadyCause::Unauthorized`.
- **Answers seen live,** against the endpoint the repo's smoke experiment names, with the secret that experiment names (`RIVAULT_LLM_INFERENCE`, already set in this shell):

| Request | Status |
|:--|:--|
| no token | 401 |
| the real token | 200 |
| a made-up token | 401 |

- **Reachable from here:** yes, with no VPN step needed at the time of the probe. The unreachable case (no answer, DNS failure, timeout) is read from curl's exit status and treated as "unreachable".
- **What was sent where:** the token went to that one endpoint only, through curl's config on stdin, so it appeared in no argument, log or file. Nothing but the status code was printed. No completion or chat request was made, and no sidecar was started.
- **Consequence:** Step 2 adds `resolve/steps/gateway.rs` (`GatewayStep`, check `gateway_token`).

## What was built
- **Secrets step** (`resolve/steps/secrets.rs`): one check per declared secret, named `secret_<NAME>@1`, passing when present. It is handed names that are present, never values.
- **Gateway step** (`resolve/steps/gateway.rs`): check `gateway_token`, the host-side model-list read of P5, through `curl` under the supervisor. The bearer header goes to curl's standard input, so the token is in no argument and no file. 200 passes, 401 is "the token was refused", no answer is "unreachable". No HTTP client crate is in the lock, and `Cargo.toml` is not owned, hence curl.
- **`resolve`:** text lines `<path>\t<value as JSON>\t<source>` sorted by path, or `--json`. Failures are lines on stdout with their check id (`secrets.secret_<NAME>`, `gateway.gateway_token`, `<step>.error`, `plugins.dir_missing`), all collected before the exit, then the usual one-line refusal on stderr. Exit 2.
- **`ls`:** `<path>\t<cells>\t<runs>` per experiment, `<path>\tskipped\t<reason>` per other TOML file.
- **`watch`:** JSON lines by default, `--live` conflicts with `--json`. The progress line is `progress runs <ended>/<started> outcomes <n>`, with `/<planned>` after `n` when the job planned its outcomes.
- **`term.rs`:** `color_enabled`, `stdout_color`, `Paint`. `Paint` writes every escape byte, including the line rewrite (`Paint::rewrite`).
- **`ps`:** a fifth field `what`, from `spec.json`.
- **`session.rs`:** `read_spec` writes the verified bytes to `<job dir>/spec.json`.

## Found beyond the plan
- **Step 2's check ids:** `<step name>.<check name>`, each without its version token (`secrets.secret_ALPHA_SECRET`, not `secrets@1.secret_ALPHA_SECRET@1`). They are check ids, not codes of `codes.toml`, and no section was appended. The exit is 2 through the existing `resolve.preflight_failed` refusal, so `--error-format json` gives that code and one stderr line.
- **Step 3's stored-run match:** a sealed run matches a file when, for some cell of the file, every value the cell declares equals the run's recorded value at that path. Settings are not compared, because a manifest holds none (`candler_schemas` drops them from `Comparable`). `Comparable::values()` is public, so the count is shown. Cases it cannot tell apart: two files that declare the same values count the same runs; an integer written as `30.0` does not equal a recorded `30`; an unsealed run is not counted.
- **Step 5's spec file:** `spec.json` is the bytes the launcher sent. The hunk is the one function of `session.rs`, plus the `SPEC_FILE` constant and one import. A job started before this change has no file and shows `-`.
- **`resolve` stores nothing:** it does not call `resolve_inputs` (which writes blobs to the store), so a declared agent input's `request.inputs` setting is not in its output. `run` adds it.
- **`plugins.dir_missing`** is checked on a resolution that succeeds, too.
- **The gateway check needs the network.** An offline `candler resolve` fails on `gateway.gateway_token`, by design: that is the P5 check.

## Spec defects
1. **OWNED omits four paths the leaf cannot do without:**
   - `crates/candler/src/main.rs`: the dispatch arms `resolve::run(args)` and `ls::run(args)` take neither the global `--store` nor an async context, so `--store` could not reach the verbs. Two arms changed.
   - `crates/candler/src/commands/mod.rs`: one `pub mod term;` line, since `term.rs` is a new module.
   - `crates/candler-core/src/resolve/steps/gateway.rs`: Step 3 creates it when P5 says yes, and OWNED lists only `{secrets,mod}.rs`.
   - `resolve/steps/mod.rs` gains two `pub mod` lines (`gateway` and `secrets`), not one.
   The OWNED gate prints these three files as outside; they are listed again under "Files outside OWNED".
2. **Unit guards cannot sit in `guard_cli_observe.rs`.** The binary's modules are not reachable from a test crate. `o3_color_...` reads `term.rs` through a `#[path]` include, so it is selected by `test(/^o3_/)`. `o1_the_source_printer_names_all_eight_kinds` is an inline test of `resolve.rs`, whose nextest name starts with `commands::resolve::tests::`, so `test(/^o1_/)` does not select it, nor the two inline gateway rows. The unanchored `test(/o1_/)` selects all seven; it ran 15 tests across `o1_`, `o2_` and `o3_`, all PASS.
3. **Step 2's world cannot pass its own gateway check.** The `cli_run` experiment's endpoint is `http://127.0.0.1:9`, so `resolve` would fail with exit 2. The guards pass `--set model.endpoint=<a local stand-in>`, which answers 200 only to the right bearer token.
4. **`ps` cannot show `suite:` yet.** `JobSpecFile` has no `Evaluate` arm until `evaluate_body`. `what_of` matches the enum exhaustively, so that leaf's merge fails to build in `ps.rs` until it adds the `suite:` arm, which is the point of the exhaustive enum. `o3_ps_shows_the_experiment_or_the_suite` covers `experiment:` and `-` (no file, and a file that is not a spec).
5. **Step 4 words the progress line twice** (`progress outcomes <n>/<planned>` and `progress runs <ended>/<started> outcomes <n>`). One line serves both.
6. **`o3_spec_file_holds_no_secret_value` as written**, "through the real supervisor and the test harness", has no harness in `candler-core` that runs `read_spec`. The guard re-executes its own test binary as a child that calls `read_spec` with a planted value in the environment. A second row, `o3_launched_job_spec_json_holds_no_secret_value` in `guard_cli_observe.rs`, runs the real `candler run` supervisor over the fake docker.

## Behaviour changes (reported, none marked)
1. `watch` no longer requires `--json` (R10 §3, approved). Its `--json` help changed from "the only form there is" to "this is the default". It stays unstamped, since `watch --json` is on `PENDING_STAMPS`.
2. `ps` rows gain a fifth tab-separated field. The four earlier fields keep their places, and the existing guards read them by position.
3. `resolve`, `ls`: new verbs, not changes. Both refused with "m3 scaffold" before.
4. A job directory gains `spec.json` once its supervisor has read the spec.
5. New options carry the U2 stamps: `resolve --set`, `resolve --json` and `watch --live`.

## Guards and bites
Each bite was run on the working tree, seen red, and reverted.

| Guard | Bite (mutation) | Result |
|:--|:--|:--|
| `o1_the_secrets_step_derives_presence_and_fails_each_missing_secret` | a missing secret passing; a value (read from the environment under the secret's name) in the explanation | red; red. The value row needed a planted value, so the present secret is `PATH` |
| `o1_resolve_reports_every_failure_at_once_with_codes` | only the first failure kept | red |
| `o1_resolve_prints_every_value_with_its_source_and_starts_no_job` | an empty source column; a `jobs/x` directory created; a `docker create` made | red; red; red |
| `o1_resolve_checks_the_gateway_token_by_a_model_list_read` | a 401 judged a pass | red (the inline verdict row is red too) |
| `o2_ls_lists_experiments_under_experiments_and_the_paths_given` | the whole project scanned; the working directory scanned too | red; red |
| `o2_ls_skips_other_toml_with_its_reason_and_never_scans_the_cwd` | skipped lines dropped; reasons emptied | red; red |
| `o3_color_is_on_only_on_a_terminal_with_no_color_unset` | `NO_COLOR` ignored; colour on a pipe | red; red |
| `o3_watch_prints_json_lines_by_default` | "watch needs --json" restored | red |
| `o3_live_on_a_pipe_prints_plain_lines_and_no_escape_byte` | the terminal branch forced with the painter on | red |
| `o3_ps_shows_the_experiment_or_the_suite` | the fifth field dropped; the fields reordered | red; red |
| `o3_spec_file_holds_no_secret_value` | the environment appended to `spec.json`; nothing written | red; red |

`o1_the_source_printer_names_all_eight_kinds` and `o1_the_gateway_verdicts_follow_the_answer` were not mutated: the first holds nine distinct texts by construction and the second is the table the other row checks through the binary. The guards were written after the code in this leaf, not before it: each was seen red by mutation, not by absence of the verb.

## Gates
- OWNED: FAIL as literally written. It prints `main.rs`, `commands/mod.rs` and `gateway.rs` (spec defect 1).
- O1, O2, O3 as written (`^`-anchored): PASS (4, 2 and 5 tests). Unanchored over `o1_|o2_|o3_`: PASS (15 tests).
- Existing `guard_cli_e2e`, `guard_cli_kill`, `guard_cli_kill_live`, `guard_cli_run`: PASS (14 run, 1 ignored for Docker). The numstat of `guard_cli*.rs` shows 0 deleted lines (one added file).
- Runtime guards (`guard_job_runtime`, `guard_job_orchestrate`, `guard_job_store`): PASS (8 run, 1 skipped).
- No scaffold stub in `resolve.rs` or `ls.rs`: PASS. `watch needs --json` is gone from `crates`: PASS.
- G-platform (one `cfg(not(unix))`) and G-digest (no `oracle` or `vectors` line changed): PASS. `term.rs` uses `std::io::IsTerminal`, so no platform-inventory row was added.
- STD in the `-p candler -p candler-core` forms: PASS. `fmt --check`, clippy with `-D warnings` over all targets, nextest (271 run, 19 skipped) and `cargo doc` with `-D warnings`.
- LINUX (`--target x86_64-unknown-linux-gnu`, `-p candler -p candler-core --tests`): PASS. LINUXRUN is the coordinator's.
- SUBJ: `cog check fe1a942..HEAD` reports no errored commits, and every subject is at most 72 characters.
- SPAWN (core source clean at HEAD, `cargo clean -p candler-core`, then `guard_signal`): PASS (9 tests), run before the spawning guard.
- P5's answer is above, and the gateway step exists because it says yes.

## Files outside OWNED
- `crates/candler/src/main.rs`: the `Resolve` and `Ls` dispatch arms.
- `crates/candler/src/commands/mod.rs`: `pub mod term;`.
- `crates/candler-core/src/resolve/steps/gateway.rs`: new, Step 3.

## Cost
About 45 minutes of wall clock under a load of about 8. The target directory peaked at 3.2 GiB and is deleted. P5 made three `GET /v1/models` reads (no token, the real token, a made-up token) and nothing else; no token was spent.
