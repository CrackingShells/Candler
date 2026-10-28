# M2b Execution parity

**Goal**: `candler run` executes an experiment under host isolation and produces runs equivalent to today's `run-config`, with supervision that leaks nothing.
**Pre-conditions**:
- [ ] `../m1_contracts.md` is `done` and merged on `main`
- [ ] Reminder `candler-M2b` read ("same sandbox composition, same provenance fields", "from any working directory")
- [ ] Eliott's go and model choice for the smoke matrix, which spends tokens; RiVault only after asking him, since its model table changed recently
- [ ] A Linux host for the Linux half of the supervision gates: a VM, or CI after the first push (Eliott's call); until then those gates are recorded as not yet run, never as passed
**Success Gates**:
- ⬜ A smoke experiment started from an unrelated directory produces runs whose sandbox composition and provenance fields match today's `htn-creator-eval run-config` for the same cell
- ⬜ Killing the job supervisor mid-run, then running `candler ps` or `candler run`, leaves no process carrying the run marker, on macOS and on Linux
- ⬜ Resume after a kill replays the stored resolved values; a changed harness version is refused unless forced, and a forced deviation is recorded per run
- ⬜ `ps`, `kill` (with `--all` and `-y`), the first event set and `watch --json` work
- ⬜ A canary secret appears nowhere in the store, the job directory, the events or the logs
- ⬜ The comparability diff over the run manifest's comparability fields returns a stratum verdict: two runs differing only in a declared factor compare, two differing in an undeclared field are refused (I7)
**References**: [R01 §Jobs, runs and supervision, §Harness drivers, §Budget, §CLI surface over the jobs](../../../../__design__/architecture.md) — job model, run identity, ends, driver traits

## Step 1: Plan the milestone

**Goal**: The campaign `__roadmap__/m2b-execution-parity/` exists and Eliott has approved it.
**Implementation Logic**:
In plan mode, author the campaign with `managing-roadmaps`, covering:

- **Job supervisor**: detached, job directory, `flock` lock, `events.jsonl`, `lost` detection; `ps`, `kill`, `watch --json`.
- **Experiment resolution**: experiment file → cells with the source of every value; budget; preflight steps registered into M1's `resolve` (isolation build, harness version and capabilities, secrets, gateway auth probe), spending no tokens.
- **Comparability diff and stratum verdict** over the run manifest's comparability fields (I7); M4 and M5 rely on it.
- **Runs**: run key `(cell digest, replicate, attempt)`, the per-experiment lock, `run.done`, the resume contract, the ends table.
- **Host isolation**: workspace build, environment allowlist plus the driver's overlay, secrets by name, the confinement hook carried over from `_confine_hook.py` together with the HTN repo's sandbox contract (`SANDBOX_REFERENCE.md`, v5).
- **Stray sweep and slots**: Linux child subreaper and `/proc` marker scan; macOS same-user marker scan; machine-wide slot files.
- **Claude Code driver**: `prepare` (skill checkout at the pinned tag, MCP configuration, hook), `command`, `stop`, `classify_exit` from the sessions, `collect`.
- **Catalog snapshot capture** and census per run; `candler run`.

Adversarial verification is required for supervision (failures are silent: pid reuse, orphans after the supervisor dies, detached descendants) and for the I7 manifest diff (the nuisance set).
Gotchas: Claude Code 2.1.304 starts its Bash shell and its hooks detached, in new sessions, so a group kill misses them; `CLAUDECODE` must never reach the child; today's resume deletes partial runs, Candler must not; the confinement hook is the only confinement today (permission deny-rules were inert); Docker is not needed before M5.
**Deliverables**: `__roadmap__/m2b-execution-parity/README.md` and its leaves
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/m2b-execution-parity/` (expected: PASS)
**Commit**: `docs(roadmap): plan the m2b-execution-parity milestone`

## Step 2: Build and land

**Goal**: The milestone campaign is executed and lands on `main`.
**Implementation Logic**:
Execute the campaign on `feat/m2b-execution-parity`, then land it on `main` with one `--no-ff` merge. The Success Gates above must pass on `main` after the merge. If a design decision changed, `__design__/architecture.md` changes in the same merge.
**Deliverables**: job supervisor, experiment resolution, run identity and resume, host isolation, stray sweep, slots and the comparability diff in `candler-core`; the Claude Code `HarnessDriver`; `candler run`, `ps`, `kill`, `watch --json`; a supervision test suite
**Consistency Checks**: `cargo test --workspace` (expected: PASS)
**Commit**: `merge(orchestrator): land execution parity under host isolation`

## Step 3: Close out

**Goal**: The milestone is recorded and the next ones can start cold.
**Implementation Logic**:
Write a knowledge-transfer report under `__reports__/m2b-execution-parity/`. Mark this leaf `done` with `dirtree-rdm update`. Propose the reminder reconciliation to Eliott: `candler-M2b` complete; `candler-M3` and `candler-M4` startable. Apply only on his yes, through `sw_write.py`.
**Deliverables**: `__reports__/m2b-execution-parity/` knowledge-transfer report; this leaf's status in `__roadmap__/candler-v1/contracts/parity/README.md`
**Consistency Checks**: `bash ~/.claude/plugins/cache/cracking-shells/managing-roadmaps/1.2.0/skills/managing-roadmaps/scripts/dirtree-rdm.sh validate __roadmap__/candler-v1/contracts/parity/` (expected: PASS)
**Commit**: `docs(reports): close the m2b-execution-parity milestone`
