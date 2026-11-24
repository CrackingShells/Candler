# Kill off a TTY

**Goal**: `candler kill <job>` without `--yes`, off a terminal, refuses with a message on stderr that names `--yes` instead of prompting on stdout and doing nothing (P6, audit-5).
**Estimate**: RiVault first, $1 with the Sonnet fallback, cap $2. At the cap, stop, commit what is green, and list what is left. Builds use the target dir the campaign README names, removed at the leaf's merge.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/integrity HEAD` succeeds
**Success Gates**:
- ⬜ A guard at the CLI runs `candler kill <job>` with stdin not a terminal and no `--yes`: nothing is printed on stdout, stderr names `--yes`, and the job is untouched; on the base the same guard fails on the stdout prompt
- ⬜ On a terminal, and with `--yes`, `kill` behaves as before
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass, with the target dir the campaign README names, and `container-e2e` is green on the pull request
**References**: [E2E audit, D5](../../../__reports__/diet/01-findings_e2e_audit_v0.md) — the defect as found; [Cycle plan v2 §Open items](../../../__reports__/diet/04-architecture_cycle_plan_v2.md) — audit-5

## Step 1: Refuse off a TTY
**Goal**: A non-interactive consumer is told how to confirm.
**Implementation Logic**:
Write the guard first and see it fail on the base. Then make `kill` check for a terminal before prompting; off one, without `--yes`, it refuses through the CLI's existing refusal path, with the exit code that path uses.
**Deliverables**: the CLI `kill` command; its guard at the CLI; `__reports__/diet/leaves/kill_off_tty_v0.md`
**Consistency Checks**: `cargo nextest run -p candler -E 'test(/kill/)'` (expected: PASS)
**Commit**: `fix(cli): refuse kill without --yes off a terminal`
