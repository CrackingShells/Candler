# seam_fixes: leaf report (v0)

Status: done. Four fixes of seam review 1 (F8, F6, F2, F1) and the F5 doc line, one commit each.
Every guard was run on the base of its commit and failed there, then passed on the tip. The full
`cargo nextest run --workspace` passes (753 tests), as does `cargo clippy --workspace --all-targets
-- -D warnings`.

## F8: kill off a terminal
- Change: the terminal test moved from `run` into `kill()`'s `if !args.yes` branch, after
  `store_root` and `targets`.
- Gate: `kill_off_a_terminal_reports_the_store_before_the_confirmation` (`guard_cli_kill.rs`).
  **PASS.** Base: stderr is `config.kill_not_confirmed` where the store's failure is wanted. Tip:
  `candler.internal` "`kill` failed: Not a directory", and the control run on a good store still
  gives "pass -y".
- Deviation from the brief: `--store <nonexistent dir>` is not a store failure on this base
  (`targets` treats a missing `jobs/` as no jobs), so the guard uses `kill --all` with a store path
  that is a regular file, which does fail in `targets`.

## F6: every treatment level renders its failures alike
- Change: `resolve` resolves the levels in one loop; a preflight failure of any level goes through
  `render_failures` / `print_failures`, and the loop stops at the first failing level.
- Gate: `o1_resolve_reports_a_later_level_failure_like_a_first_one`. **PASS.** Base: stdout empty
  for the later level, the first level prints `treatment_identity.error`. Tip: stdout and stderr
  equal for the missing tag in the first and in the second level.

## F2: an MCP refusal names its field
- Change: `InvalidValue` gives its path as the pointer; the entry is read with
  `serde_path_to_error` (already in `[workspace.dependencies]`, now also a dependency of
  `candler-harness`), so the path ends at the field. A field-path segment cannot be an index, so a
  fault inside `args` points at `args`. The registry row and both contract pages say what the path
  is.
- Gate: the four `mcp` rows of `guard_silent_acceptance` ROWS. **PASS.** Base: `pointer: None` in
  all four. Tip: `/mcp/servers/fs/command`, `/mcp/servers/fs/args`, `/mcp/servers/fs` (neither
  `command` nor `url`: the entry is the fault) and `/mcp/servers/fs/env/token`.
- Every other `InvalidValue` now carries a pointer too (a cell field path as a pointer); no guard
  pinned `None` for them.
- F5: the doc of `Setup::collector` says it is the core's file collector applied to the driver's
  specs.

## F1: one reader of `replicates`
- Schemas commit: `ExecutionError::ReplicatesNotCount`, code `experiment.replicates_not_count`,
  pointer `/replicates`, message "must be a whole number from 1 to 4294967295"; `check_replicates`
  beyond 32 bits returns it. Its registry row and the contract page row are in the same commit,
  because the registry test needs them.
- Core commit: `check_replicates_input` keeps only the path refusal; `ReplicatesNotInteger` and
  `experiment.replicates_not_integer` are gone from code, `codes.toml` and the contract page
  (`git grep` over `crates/` and `docs/` prints nothing); earlier reports keep the code they
  recorded. `Levels::replicates` refuses any non-`u64` value with the new code.
- Gates. **PASS** each.
  - `replicates_are_checked_once` (schemas): base `experiment.cost_not_positive`, tip the new code,
    pointer and range.
  - Table in `guard_silent_acceptance` (op API: layers then `plan_runs`; `"three"`, `2.5`, `-1`,
    `5000000000`): base panics in the layers with `experiment.replicates_not_integer`, tip all four
    give `experiment.replicates_not_count` at `/replicates`.
  - `o1_resolve_refuses_what_is_no_replicate_count_as_the_plan_does` (CLI, `resolve --set`, `-1`,
    `2.5`, `5000000000`): base `experiment.replicates_not_integer`, tip the new code, pointer and
    range.
- Deviation: the brief's one table for both entries is two tables with one hand-written
  expectation, since the CLI and the op API fixtures live in different crates.
