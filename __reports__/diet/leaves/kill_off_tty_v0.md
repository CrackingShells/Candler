# kill_off_tty: leaf report (v0)

Status: done. `candler kill <job>` without `--yes`, off a terminal, refuses on stderr before any
line reaches stdout; on a terminal and with `--yes` the stop behaves as before.

## First reads
- `__roadmap__/integrity/causes/kill_off_tty.md`, audit-5 in
  `__reports__/diet/01-findings_e2e_audit_v0.md` (D5: `candler kill <job>` without `--yes` prints
  `stop them? [y/N]` on stdout and exits 2 when not on a terminal; the first kill attempt of the
  audit did nothing for that reason).
- `crates/candler/src/commands/kill.rs` (`kill`, `confirm`, `run`), `commands/refusal.rs`
  (`CliRefusal::KillNotConfirmed` = "pass -y to confirm"), `commands/mod.rs` (`CliError::refused_by`,
  the exit mapping: any `CliError` is exit 2), `main.rs` (`dispatch`, `status`, `render`).
- `crates/candler/tests/it/guard_cli_kill.rs` (the established way other `kill` guards run the real
  binary `CARGO_BIN_EXE_candler` with `Stdio::null()` stdin and make a lost job), and
  `guard_cli_kill_live.rs` (`confine` sets `Stdio::null()` for the b41 dry run).

## The defect and the change
Off a terminal and without `-y`, `kill` wrote `would stop: <id>` and the `stop them? [y/N] `
prompt to stdout, then read a stdin that is not a terminal (EOF), and finally refused with exit 2
on stderr — a consumer that cannot answer the prompt still saw it and did nothing.

Change (Grit, `crates/candler/src/commands/kill.rs::run`): before locking stdin and prompting,
refuse when `std::io::stdin().is_terminal()` is false and `args.yes` is false, through the existing
refusal path `CliError::refused_by(CliRefusal::KillNotConfirmed)`, which `main` maps to exit 2 and
renders `candler: refused: pass -y to confirm` on stderr. On a terminal, or with `-y`, the prompt
and the stop are unchanged. No library crate and no schema changed.

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| Guard at the CLI runs `candler kill <job>` with stdin not a terminal and no `--yes`: nothing on stdout, stderr names `--yes`, job untouched; on the base the same guard fails on the stdout prompt | PASS | base red and tip green, below |
| On a terminal, and with `--yes`, `kill` behaves as before | PASS | b41 real-daemon `-y` full reap passes; the guard's `-y` control passes |
| `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | PASS | see Checks |

### Gate 1 — off a terminal, no `--yes`

**Base red.** On `origin/feat/integrity` (the worktree's starting HEAD, `6ffe2850`) the new guard
`kill_off_a_terminal_refuses_before_prompting` fails on the stdout-prompt assertion:

```
$ cargo nextest run -p candler -E 'test(kill_off_a_terminal_refuses_before_prompting)'
    FAIL [   1.543s] (1/1) candler::it guard_cli_kill::kill_off_a_terminal_refuses_before_prompting
    thread '…' panicked at crates/candler/tests/it/guard_cli_kill.rs:131:5:
    assertion `left == right` failed: stdout carried the prompt or the plan: "would stop: 01890000-0000-7000-8000-000000000003\nstop them? [y/N] "
      left: "would stop: 01890000-0000-7000-8000-000000000003\nstop them? [y/N] "
     right: ""
```

**Tip green.** After the `run` change the same guard passes: stdout is empty, stderr names the
confirm flag, no docker call is made (the job is untouched):

```
$ cargo nextest run -p candler -E 'test(kill_off_a_terminal_refuses_before_prompting)'
    PASS [   1.713s] (1/1) candler::it guard_cli_kill::kill_off_a_terminal_refuses_before_prompting
```

The guard's refusals assert, in order: exit status 2, `stdout == ""`, `stderr.contains("-y")`
(the `KillNotConfirmed` message, whose flag is `-y`/`--yes`), and the fake client's call log is
empty. The `-y` control in the same guard returns exit 0 with no docker stub needed past the
refusal.

### Gate 2 — on a terminal and with `--yes`, unchanged

The fake-client `guard_cli_kill.rs` B31 (`kill --all` refusal + `kill --all -y` control, exit 0 and
the `container ls` that lists the job) and A36 (store via `--store`/`CANDLER_STORE` with `-y`)
still pass. The real-daemon b41 in `guard_cli_kill_live.rs`, which runs `kill --all -y` against two
live supervisors and asserts each is reaped and ends `killed`, passes; its pre-`-y` dry run (stdin
closed) is updated to assert empty stdout off a terminal instead of the `would stop: …` line that
is no longer emitted. The leaf's consistency check passes:

```
$ cargo nextest run -p candler -E 'test(/kill/)'
    Summary [  6.023s] 9 tests run: 9 passed, 146 skipped
    (also pass standalone: 9/9, and in the full workspace run)
```

### Checks
```
$ cargo fmt --check                                     # exit 0
$ cargo clippy --workspace --all-targets -- -D warnings # exit 0
$ cargo nextest run --workspace                         # 741 pass, 32 skipped, exit 0
$ RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps  # exit 0
```
An early 741-test workspace run was SIGTERMed at 120 s and reported 12 failures under the
deadline; rerun with the deadline lifted it reports all 741 pass, so those were deadline kills, not
test failures. `crates/candler-schemas/schema/` is untouched (no struct or doc-comment changed), so
there is no drift. The `container-e2e` group (`b40`, `b41`, and `guard_cli_e2e/i8_e2e`) ran green
on the real daemon in this environment; the pull request's own `container-e2e` label runs it in CI.

## Verifier
A fresh reader given the leaf, this report and the diff: the base failure is honest (the guard was
run before the fix and failed on the exact stdout prompt); tip green; on-terminal and `-y`
behaviour is proven by b41's full real reap and the guards' `-y` controls; no library crate, no
schema and no roadmap status changed; source comments state what the code is without history or
plan references. No gate is skipped as a pass.

## Left
Nothing in scope. `crates/candler-schemas/schema/` never regenerated; no drift. `b41`'s dry-run
assertion was the only other test edited, to match off-terminal empty stdout.
