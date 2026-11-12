# OpenCode driver

**Goal**: `candler-harness-opencode` implements `HarnessDriver` and its readers for OpenCode V2 from the facts `opencode_facts` recorded, passes the shared fixture suite, and ships a base image recipe; nothing in it needed a branch in the core or a trait item it lacked.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] Read first: `__reports__/m4-session-runner/leaves/opencode_facts_v0.md`
- [ ] [nl] OWNED: `crates/candler-harness-opencode/**`, `Cargo.lock` (this crate's dependencies), `assets/docker/base-opencode/**`; the `ASSETS` array of `crates/candler/src/commands/assets.rs` (the binary embeds every file under `assets/docker`, as M3's U3 guard checks); its error enums' codes in `docs/contract/codes.toml`
**Success Gates**:
- ⬜ [cmd] K1 for OpenCode: `cargo nextest run -p candler-harness-opencode --no-tests=fail -E 'test(/^conformance_/)'`
- ⬜ [cmd] O1: the export parse, the forced `--standalone --format json`, the in-container pre-stop carrying `{session_id}`, the isolated state paths, and `codemode` read from a cell value under `mcp.*`: `cargo nextest run -p candler-harness-opencode --no-tests=fail -E 'test(/^opencode_/)'`
- ⬜ [cmd] The driver declares no session capability and the execution outside its child: `cargo nextest run -p candler-harness-opencode --no-tests=fail -E 'test(/^opencode_capabilities_/)'`
- ⬜ [cmd] The base recipe pins the version and checksum `opencode_facts` recorded: `v=$(sed -n 's/^version *= *"\(.*\)"/\1/p' crates/candler-harness-opencode/tests/fixtures/PROVENANCE.toml); grep -q "$v" assets/docker/base-opencode/Dockerfile && grep -q sha256 assets/docker/base-opencode/Dockerfile`
- ⬜ [cmd] STD (`-p candler-harness-opencode`), LINUX, DIGEST and SUBJ pass
- ⬜ [nl] The leaf report lists every `classify_exit` class and whether a recorded fixture or only the documentation backs it, and every place the driver had to work around the traits
- ⬜ [nl] The `opencode_driver` verifier closes with no open finding: it attacks "what did OpenCode need that the trait could not express" (a default that only fits Claude Code, a fact passed through an unrelated field, behaviour hidden in `prepare` that the core should own, an export that cannot be the authoritative trace)
**References**: [R01 §OpenCode v2, §Harness drivers](../../../../__design__/architecture.md); [R09 D9, D11](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-approved-plan.md); [the Claude Code base recipe](../../../../assets/docker/base/Dockerfile) — the pinning pattern

## Step 1: Write the driver's fixture cases
**Goal**: The suite states OpenCode's expected behaviour from its recorded runs before the driver exists.
**Implementation Logic**:
Turn each recorded case of `tests/fixtures/streams/` into a conformance case (stream, export, expected sessions, observed id, end class, init report or census source). Write O1's tests against the driver's API. Both fail.
**Deliverables**: `crates/candler-harness-opencode/tests/conformance.rs`; `crates/candler-harness-opencode/tests/guard_opencode_driver.rs`: `opencode_*`
**Consistency Checks**: `cargo nextest run -p candler-harness-opencode --no-tests=fail -E 'test(/^conformance_/)'` (expected: FAIL)
**Commit**: `test(adapter-opencode): state the driver's behaviour from the recorded runs`

## Step 2: Implement the driver and its readers
**Goal**: The driver turns a cell into an OpenCode run and its output into sessions.
**Implementation Logic**:
1. `command`: `run --standalone --format json` with the model and the prompt as the facts require; `prepare`: a home-rooted `$XDG_CONFIG_HOME/opencode/opencode.json` (`PreparedFile::in_home`) whose provider names the `GatewayEnv` variables as `{env:<base>}/v1` and `{env:<token>}` (confirm inline `{env:}` concatenation in V2 with `opencode debug config` inside the base image, offline), MCP servers under `mcp.servers.<name>` with `codemode` from the cell value `mcp.<server>.codemode`, the skill layout `opencode_facts` found.
2. `state_paths`: HOME's XDG directories and `OPENCODE_DB` (a file).
3. `stop`: `Action::Exec` running an in-container script that finds the `opencode serve` child by exact `/proc/<pid>/cmdline` match, reads its port and `OPENCODE_PASSWORD` from `/proc`, and calls `opencode api ... session.interrupt` with `{session_id}` (resolved through `Action::resolve`); then SIGTERM and the grace the facts show (TERM is honoured, exit 143).
4. `collect`: `CollectSpec::Exec` running `opencode session export --standalone {session_id}` inside the isolation, with the run's state environment; `live_reader`: a Lines reader over the NDJSON that reports the session id and early events; `offline_reader`: a Document reader of the export, the authoritative trace.
5. `classify_end` from the recorded classes: exit 1 is a failure, an API interrupt or an abort, told apart by the last `error` event's type or the export's `outcome`, and the export wins; `capabilities`: no session, `execution_outside_child`, and `census = false` for now: the gateway census source belongs to the `opencode_wiring` amendment, pending Eliott's approval, which adds it.
**Deliverables**: `crates/candler-harness-opencode/src/{lib.rs,driver.rs,live.rs,export.rs,prepare.rs,skills.rs}`: `OpenCodeDriver`, `OpenCodeLiveReader`, `OpenCodeExportReader`, `OpenCodeSkills`
**Consistency Checks**: `cargo nextest run -p candler-harness-opencode --no-tests=fail -E 'test(/^conformance_/)|test(/^opencode_/)'` (expected: PASS)
**Commit**: `feat(adapter-opencode): drive opencode v2 through the shared traits`

## Step 3: Add the base image recipe
**Goal**: An OpenCode experiment has a base image, pinned as Claude Code's is.
**Implementation Logic**:
`assets/docker/base-opencode/Dockerfile`: the same Debian base, uv and Python as `assets/docker/base`, OpenCode V2 at the pinned version, its checksum verified, and the harness version label under the key the core reads. A local build needs Docker and reuses the download Eliott approved for `opencode_facts` (linux-arm64 only); the x64 checksum may be written from the release's published digest, but no x64 file is downloaded without his go. Remove the image by id afterwards.
**Deliverables**: `assets/docker/base-opencode/Dockerfile`
**Consistency Checks**: `docker build -q assets/docker/base-opencode` (expected: PASS)
**Commit**: `build(adapter-opencode): add the opencode base image recipe`
