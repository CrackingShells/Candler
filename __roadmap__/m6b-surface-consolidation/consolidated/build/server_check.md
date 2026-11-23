# Server start check

**Goal**: Once per job, before its first run, in a container set up as a run's (same image, mounts, env and secrets), each declared stdio MCP server's start command is started with stdin open and must still be running after a short wait, or the job is refused with the server's stderr; remote servers are skipped; no MCP client is added (C2, H1, ruled by Q4 and Q4c).
**Estimate**: about 45 Sonnet calls ($1.8), cap 60 (amendment A2); at 60 calls, stop, commit what is green, list what is left `[V]`
**Pre-conditions**:
- [ ] [cmd] Every-leaf pre-conditions (build README, Gotchas)
- [ ] [cmd] Docker answers, for the real-container case only: `docker info >/dev/null` (if not, `docker desktop start`, then wait until it answers; never stop or prune it)
- [ ] [nl] OWNED (regex): `^crates/candler-core/src/jobs/orchestrate/[a-z_]+\.rs$|^crates/candler-core/src/isolation/(container/[a-z_]+|testing|refusal)\.rs$|^crates/candler-core/src/runner/exec/[a-z_]+\.rs$|^crates/candler-core/src/isolation/mod\.rs$|^crates/candler-harness/src/mcp\.rs$|^crates/candler-core/tests/it/(main|guard_job_orchestrate|guard_server_check|guard_container_e2e)\.rs$|^crates/candler-core/tests/it/guard_container_e2e/.*$|^docs/contract/(codes\.toml|[a-z-]+\.md)$|^__reports__/m6b-surface-consolidation/build/server_check_v0\.md$`. `codes.toml` gains a row only if step 1 shows no existing code fits (appended, `side` from the existing six, no `remedy`). Nothing enters the integrity record and nothing is hashed
**Success Gates**:
- ⬜ [cmd] Targeted tests: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_server_check::/) | test(/^guard_job_orchestrate::/)'`
- ⬜ [cmd] The real-container case passes and leaves nothing behind: `before=$(docker ps -aq | sort); cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'test(/server_that_exits_refuses_the_job_in_docker/)' && test "$(docker ps -aq | sort)" = "$before"`
- ⬜ [cmd] Ownership check prints nothing; `cargo clippy -p candler-core -p candler-harness --all-targets -- -D warnings`; DIGEST passes
- ⬜ [nl] The check reuses the run's own container setup (the report names the function, with `path:line`) rather than a second mount path; the wait is a named constant with its reason in its doc comment; the report says whether a `codes.toml` row was added and why; the verifier closed with no open finding
**References**: [R16 §Verdicts C2, H1, §Responsibility record (the box border), §Seams (C2 with codes_review)](../../../../__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md); R18 `leaves/round02_candidates_v0.md` §Q4c start-command gate (facts and risks: mounts exist only per run, a server's `env`, egress, one container per server); R17 round 05 (Q4c ruling); [R01 §Contracts & Invariants](../../../../__design__/architecture.md)

## Step 1: Read the job start and write the guards, seen red
**Goal**: The leaf knows how a job starts its first container and how a run's container is set up, and the guards fail on the tip.
**Implementation Logic**:
1. Through colgrep (index `crates/candler-core` of this worktree, then `crates/candler-harness`): where a job starts its first run (`jobs/orchestrate/{mod,schedule,session}.rs`); how a run's container gets its image, mounts (`mount_inputs` in `runner/exec/mod.rs`), env, secrets and network (`isolation/container/{run,mounts,exec}.rs`); how the harnesses read the declared servers (`candler-harness/src/mcp.rs`, `servers`); whether the `isolation/testing.rs` double can start a command with stdin open, report that it exited and return its stderr; which refusal code a job refused before any run gets today.
2. Cells of one job can differ in image or servers: the check covers each distinct pair of image and declared stdio servers in the same pass before the first run. Record the design in the report.
3. Guards in a new `guard_server_check.rs`, using the double, each with a `Guards:` line: `server_that_exits_refuses_the_job_with_its_stderr` (no run starts, the refusal carries the stderr); `server_that_stays_up_lets_the_job_start`; `remote_server_is_skipped` (no container is started for a `url` server); `job_without_stdio_servers_starts_no_check`.
**Deliverables**: `crates/candler-core/tests/it/guard_server_check.rs`: the four guards; `crates/candler-core/tests/it/main.rs`: the module line; `__reports__/m6b-surface-consolidation/build/server_check_v0.md`: section `## First reads`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_server_check::/)'` (expected: FAIL)
**Commit**: `test(jobs): guard the server start check before a job's first run`

## Step 2: Start each declared server before the first run
**Goal**: The four guards pass.
**Implementation Logic**:
1. Before the first run of a job, build the container a run of that cell would get, through the run's own setup function (same image, mounts, env, secrets, network), start each declared stdio server's command with stdin held open, wait `SERVER_START_WAIT`, and require it to be running; collect the stderr of any that exited, bounded in size.
   - Amendment A2: the tip has no such function (a run's container setup is inline in `drive`, `runner/exec/mod.rs`). Extract from `drive` the part that composes a cell's container (the driver's `plan`, `isolation.plan`, `apply_harness_env`, `mount_inputs`, secrets and network) into one function that takes a workspace path and writes no run record; `drive` calls it, and its existing guards stay green unchanged (that is the refactor's guard; commit `refactor(runner): …` on its own, before the check). The check calls the same function on a scratch workspace that it removes after.
   - Amendment A2: the `Isolation` trait gains one operation that starts a command in that container with stdin held open, waits, and returns running, or exited with its bounded stderr, removing the container by id on both paths; the Docker implementation lives in `isolation/container/`, and `isolation/testing.rs` gains the double the guards use.
2. A server that exited refuses the job with its key and its stderr; no run starts. Remote servers (`url`) are skipped. Every container the check started is removed by id, on success and on failure.
3. The wait is a named constant whose doc comment says what it catches (a fast crash at import) and what it does not (a server that starts and does not answer MCP: the census records that per run).
4. If step 1 found no code that fits, append one row to `codes.toml` and regenerate the pages with `CANDLER_BLESS_CONTRACT=1`.
**Deliverables**: `crates/candler-core/src/jobs/orchestrate/` (the check and where the job calls it), `crates/candler-core/src/isolation/container/` (any helper the run's setup needs to expose), `SERVER_START_WAIT`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_server_check::/) | test(/^guard_job_orchestrate::/)'` (expected: PASS)
**Commit**: `feat(jobs): refuse a job whose declared server exits at start`

## Step 3: The real-container case
**Goal**: The check is shown to work against Docker, once, without leaving anything behind.
**Implementation Logic**:
1. One ignored test, `server_that_exits_refuses_the_job_in_docker`, beside the existing container end-to-end guards: a tiny image (reuse an existing test image; build none if one fits) and a server command that exits with a message; the job is refused with that message and no container remains.
2. Scratch roots under `$HOME`; remove what the test built, by id.
**Deliverables**: `crates/candler-core/tests/it/guard_container_e2e.rs` (or its module dir): `server_that_exits_refuses_the_job_in_docker`
**Consistency Checks**: `cargo nextest run -p candler-core --run-ignored only --no-tests=fail -E 'test(/server_that_exits_refuses_the_job_in_docker/)'` (expected: PASS)
**Commit**: `test(isolation): check the server start against a real container`

## Step 4: Bites, the report and the verifier
**Goal**: The guards are shown to bite, and the leaf is verified.
**Implementation Logic**:
1. Bites, each made, seen red and reverted: check after the first run instead of before (red: a run starts); close stdin at start (red on `server_that_stays_up_lets_the_job_start` with a server that exits at EOF); start remote servers (red on `remote_server_is_skipped`); drop the stderr (red on the refusal's content).
2. Write the report (build README, Every leaf), with step 1's answers.
3. Dispatch a fresh Sonnet verifier (model passed explicitly) with this brief verbatim, adding only the branch name. **Verifier brief:** "You verify the leaf `__roadmap__/m6b-surface-consolidation/consolidated/build/server_check.md` on branch <branch>. Read the build README (Gotchas), R16 (`__reports__/m6b-surface-consolidation/01-findings_verdicts_v3.md`) row C2 and the box border in §Responsibility record, `__reports__/m6b-surface-consolidation/leaves/round02_candidates_v0.md` §Q4c, then the diff from the leaf's branch point and the code through colgrep. Assume something is wrong. Attack: a container set up differently from a run's (a second mount list, missing env, secrets or network); an MCP client or handshake added; a check that runs per run, after the first run, or at `resolve`; a remote server started; a container left behind on either path; a wait that is a bare number; stdin closed; anything written to the integrity record or hashed; a `codes.toml` row added when an existing code fits, or with a `remedy`; a real-container test that leaves an image or a container; an oracle read from the code under test; a comment carrying history or ids. Report each finding with the step, `path:line` and the evidence. Change nothing."
4. Fix the findings; have the same verifier re-check through SendMessage; record each finding and its disposition in the report.
**Deliverables**: `__reports__/m6b-surface-consolidation/build/server_check_v0.md`: sections `## First reads`, `## Bites`, `## D3`, `## Verifier`, `## Follow-ups`
**Consistency Checks**: `for s in 'First reads' Bites D3 Verifier Follow-ups; do grep -q "^## $s" __reports__/m6b-surface-consolidation/build/server_check_v0.md || exit 1; done` (expected: PASS)
**Commit**: `reports(api): record the server start check build`
