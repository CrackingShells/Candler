# e2e_audit

**Goal**: Establish, per claim of the README purpose paragraph, what works end to end, from real RiVault runs and stored bytes an agent has read.
**Estimate**: about 45 Sonnet calls ($1.5 with the verifier), cap 70. At 70 calls, stop, commit what is green, and list what is left in the leaf report.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/diet HEAD` succeeds, with every L0 piece merged into `feat/diet`
- [ ] `[nl]` OWNED = `^__reports__/diet/(01-findings_e2e_audit_v0\.md|audit/|README\.md$)`
**Success Gates**:
- ⬜ `__reports__/diet/01-findings_e2e_audit_v0.md` has one row per claim (experiment matrix, runners, isolation, tool-catalog snapshots, provenance, fresh evaluation, on-disk evaluation, pluggable evaluators with one record format, one operation API with thin surfaces, MCP server, domain plug-in) with works, broken or not built, and the store paths read
- ⬜ The agent has read the blob bytes, outcome records, manifests and catalog snapshots it cites
- ⬜ Every run in the audit is one a claim needs: 8 cells, one kill and resume, one ingest
- ⬜ `cargo fmt --check` passes
- ⬜ `cargo clippy --workspace --all-targets -- -D warnings` passes
- ⬜ `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes
- ⬜ DIGEST unchanged: `cargo nextest run --workspace -E 'test(/cell_digest|context_digest|build_context/)'` passes with no vector edited
- ⬜ GOLDEN unchanged: `cargo nextest run --workspace -E 'test(/golden/)'` passes with no vector edited
- ⬜ Ownership: `git diff --name-only feat/diet...HEAD | grep -vE '^__reports__/diet/(01-findings_e2e_audit_v0\.md|audit/|README\.md$)'` prints nothing
- ⬜ No product code changed
**References**: [Diet plan](../../../__reports__/diet/00-architecture_diet_plan_v0.md) — §How we establish what really works: the RiVault audit; [AGENTS.md](../../../AGENTS.md) — checks and conventions

## Step 1: Probe and design the audit experiment
**Goal**: A small experiment that touches every Execution claim.
**Implementation Logic**:
Load the `using-rivault` skill and check the VPN and key first; if RiVault is unreachable, stop and report. Build `candler` from the `feat/diet` tip with S1 merged. Write one experiment under `__reports__/diet/audit/`, shaped like `__reports__/m4-session-runner/live/opencode.toml`: OpenCode harness through the gateway, factors model (Reasoning-Small and one other cheap RiVault model) by treatment (skill on, skill off), 2 replicates, one declared stdio MCP server, and a task that makes the agent write a file. Write a plug-in suite that needs `artifacts` and reads that file. Keep the store under `$HOME`.
**Deliverables**: `__reports__/diet/audit/experiment.toml`, the plug-in suite, `commands.sh`.
**Consistency Checks**: `candler resolve` on the file prints a record with 8 cells (expected: PASS)
**Commit**: `reports(api): add the e2e audit experiment`

## Step 2: Run, kill, resume, evaluate, ingest
**Goal**: Produce the stored bytes each claim is judged from.
**Implementation Logic**:
Run the job; kill it after the first cells seal; run it again and check from `ps`/`watch` records that only the missing cells ran. Run the kept container e2e guards once and the i8 canary. `evaluate` with the artifact-reading suite and with a second fixture evaluator; `show --rates`. `ingest` the mini legacy corpus and `evaluate` it. `analyze` the store with one non-comparable pair added on purpose. Measure the wall time of the server start check on this job. Remove containers and images the audit built, by id.
**Deliverables**: the audit store (not committed) and `__reports__/diet/audit/commands.sh` with every command run.
**Consistency Checks**: each command's record saved beside the commands (expected: PASS)
**Commit**: `reports(api): record the e2e audit commands`

## Step 3: Read the bytes and judge each claim
**Goal**: A verdict per claim from evidence, not from exit codes.
**Implementation Logic**:
Read the blob bytes against what the trace shows the model wrote; outcome records against the bytes; manifests against the experiment file; the catalog snapshot against the server's real tool list; the analyze refusal and its cause; the ingested rows against the corpus oracle. Read each `commands/*.rs` to judge "one operation API, thin surfaces". Record MCP as not built (M7). Name each broken claim with the smallest fix you can see and whether one capped piece can do it.
**Deliverables**: `__reports__/diet/01-findings_e2e_audit_v0.md` (findings report: verdict table first, evidence paths, then the broken claims for `audit_fixes`); the `__reports__/diet/README.md` entry.
**Consistency Checks**: every verdict cites a stored path or a source location (expected: PASS)
**Commit**: `reports(api): judge each readme claim end to end`
