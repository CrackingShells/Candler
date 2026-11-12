# OpenCode facts

**Goal**: OpenCode V2's facts that R01 leaves unconfirmed are measured inside a Candler-hardened container, and recorded streams and exports exist for the driver to be built against.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The scope exists: `grep -q '"adapter-opencode"' cog.toml`
- [x] [nl] Eliott gave his go for the download before Step 1 (2026-11-12): `opencode-linux-arm64.tar.gz` of the GitHub release `anomalyco/opencode` v2.2.7, 120,326,654 bytes, sha256 `c1682057f0815cb2abdd6e00305f1146ebaf1c167f0335cdb74ddc80008120e7`; that file only
- [ ] [nl] Eliott gave his go and model for at most 3 RiVault runs before the real-model runs of Step 2, after a RiVault health check
- [ ] [nl] OWNED: `crates/candler-harness-opencode/{Cargo.toml,src/lib.rs,tests/fixtures/**}`, `Cargo.lock` (the new member only), `__reports__/m4-session-runner/leaves/opencode_facts_v0.md`
**Success Gates**:
- ⬜ [cmd] The stub crate is a workspace member and builds: `cargo metadata --format-version 1 --no-deps | jq -e '.packages[] | select(.name=="candler-harness-opencode")' >/dev/null && cargo build -p candler-harness-opencode`
- ⬜ [cmd] The fixtures carry their provenance: `test -f crates/candler-harness-opencode/tests/fixtures/PROVENANCE.toml && grep -q '^version' crates/candler-harness-opencode/tests/fixtures/PROVENANCE.toml && grep -q '^sha256' crates/candler-harness-opencode/tests/fixtures/PROVENANCE.toml`
- ⬜ [cmd] Each recorded case holds its stdout, stderr, exit status and, where a session exists, its export: `for d in crates/candler-harness-opencode/tests/fixtures/streams/*/; do test -f "$d/stdout.jsonl" && test -f "$d/exit.toml" && test -f "$d/expected.toml" || echo "$d"; done` prints nothing
- ⬜ [nl] The leaf report answers every question of Step 1 and Step 2 with its evidence (a command and its output), or names it unanswerable and why; it says which `classify_exit` classes are fixture-backed and which come from the documentation only
- ⬜ [nl] If no tool inventory source exists, the finding goes to the coordination session before L1, which decides D9's census rule
- ⬜ [cmd] Docker holds no image the probe built: the image list after equals the list recorded before (`docker images -q | sort`)
- ⬜ [cmd] SUBJ passes
**References**: [R01 §OpenCode v2, §Harness drivers](../../__design__/architecture.md) — the facts to confirm and the trait they feed; [R09 D9, D11](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m4-approved-plan.md) — what the driver will do with them

## Step 1: Install V2 in a hardened image and read it
**Goal**: The binary, its version and its offline surface are known without spending a token.
**Implementation Logic**:
1. Install the pinned file: `opencode-linux-arm64.tar.gz` of the GitHub release `anomalyco/opencode` v2.2.7 (the latest V2 on 2026-11-12; npm `opencode-ai` is still on 1.x), checked against its sha256 in the Dockerfile. Any other file needs a new go from Eliott.
2. Build a scratch image (probe Dockerfile, not committed) from the same Debian base as `assets/docker/base`, installing only OpenCode. Never install it on the host.
3. Run it under Candler's container hardening (read-only root, `noexec` tmpfs `/tmp`, `--cap-drop ALL`, `no-new-privileges`, a non-root uid, an internal network) and record:
   - the binary name and `--version`;
   - `run --help`: `--standalone`, `--format json`, `--port`, budgets (turns, cost) if any;
   - `session export --help` and where the export goes;
   - `debug paths` under an isolated `HOME`, `XDG_*` and `OPENCODE_DB` (a file path);
   - how a provider's base URL and key are configured when the URL is known only at run time (`opencode.json` with `{env:VAR}`, or variables), and where that file must live;
   - whether a client exists in the image to call the interrupt endpoint from inside the container (`curl`, or an `opencode` subcommand);
   - the skill layout (where OpenCode reads skills or agent instructions from);
   - how MCP code mode is set per server.
4. Create the stub crate: `Cargo.toml` (`publish = false`, no dependencies), a `src/lib.rs` holding only the crate doc, and `tests/fixtures/PROVENANCE.toml` (version, sha256, source URL).
**Deliverables**: `crates/candler-harness-opencode/Cargo.toml`; `crates/candler-harness-opencode/src/lib.rs` (crate doc); `crates/candler-harness-opencode/tests/fixtures/PROVENANCE.toml`; `Cargo.lock` (the member)
**Consistency Checks**: `cargo build -p candler-harness-opencode` (expected: PASS)
**Commit**: `chore(adapter-opencode): reserve the driver crate and pin opencode v2`

## Step 2: Record runs, stops and failures
**Goal**: The event schema, the exit codes, the stop behaviour and the export are recorded from real runs.
**Implementation Logic**:
1. Failure classes first, which spend no token and reach no RiVault host: an unknown model and a refused key, both against the fake model (`assets/docker/fakemodel`) through the gateway sidecar, and an unreachable endpoint. Record stdout, stderr and the exit status of each.
2. Three runs, each with a tool call (`--standalone --format json`, a small workspace task), first against a scratch copy of the fake model scripted for OpenCode's tools (no token), then, after Eliott's go and model, at most 3 on RiVault that confirm the shape:
   - a run that completes, then its `session export`;
   - a run stopped by SIGTERM mid-tool (then SIGINT if TERM is ignored), recording the exit status and whether the server child died;
   - a run interrupted from inside the container through `POST /api/session/{id}/interrupt`, recording the port, the response and the exit status.
3. Note the NDJSON event kinds, where the session id first appears, whether any event lists the agent's tools (the census), and whether a tool inventory can be had another way.
4. Store each case under `tests/fixtures/streams/<case>/`: `stdout.jsonl`, `stderr.txt`, `exit.toml`, `export.json` when a session exists, and an `expected.toml` written by hand from the observation (session ids, end class, tool calls).
**Deliverables**: `crates/candler-harness-opencode/tests/fixtures/streams/<case>/{stdout.jsonl,stderr.txt,exit.toml,export.json,expected.toml}`
**Consistency Checks**: `ls crates/candler-harness-opencode/tests/fixtures/streams | wc -l | awk '$1<4{exit 1}'` (expected: PASS)
**Commit**: `test(adapter-opencode): record opencode v2 streams, stops and exports`

## Step 3: Report the facts
**Goal**: Each fact R01 marks unconfirmed has a value and its evidence, and every D9 item has the answer it needs.
**Implementation Logic**:
Write the leaf report (`writing-reports`, findings): a table of fact, value, evidence and the D9 item it decides (gateway or provider config, state paths, census source, skill layout, the in-container stop, export authority). Name every fact that contradicts the V2 documentation. Remove the scratch image by its id.
**Deliverables**: `__reports__/m4-session-runner/leaves/opencode_facts_v0.md`
**Consistency Checks**: `for k in 'binary' 'exit code' 'signal' 'budget' 'event' 'export' 'OPENCODE_DB' 'interrupt' 'codemode' 'census' 'skill'; do grep -qi "$k" __reports__/m4-session-runner/leaves/opencode_facts_v0.md || echo "$k"; done | wc -l | grep -qx 0` (expected: PASS)
**Commit**: `reports(runner): record the opencode v2 facts`
