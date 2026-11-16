# Plug-in environment

**Goal**: An experiment that sets `isolation.build.plugin = "<plugin>/<env>"` gets that plug-in environment's values and inputs as a layer below its own file (D4, N1–N3). The core reads only the manifest and adds no knowledge of MCP or Python. A real container serves the declared MCP server's tools to the agent (N6).
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] OWNED: `crates/candler-core/src/resolve/steps/plugin_environment.rs` (new) and its registration line, `crates/candler-core/src/resolve/steps/image.rs` (source base directories and the append), the manifest read in `crates/candler-core/src/evaluation/registry/`, the per-cell egress union in `crates/candler-core/src/runner/plan.rs` (beside `egress::settle`'s call), the multi-base input resolution in `crates/candler-core/src/experiment/inputs.rs`, the fragment load before `resolve_inputs` in `crates/candler/src/commands/run.rs`, the fixture `crates/candler-core/tests/fixtures/plugins/agent-env/`, new guard and e2e files; it does not edit `assets/docker/`, the code `isolation.plugin_environment_missing`
- [ ] [nl] Its step registers before `image` and before `egress_setting`'s step; the coordinator checks the order at the merge
**Success Gates**:
- ⬜ [cmd] Equivalence: an experiment selecting `p/e` resolves to the same values, the same `isolation.env_lock` and the same mounts as one writing p/e's values and inputs out by hand, including an appended requirements list with a `treatment:` entry: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_plugin_environment::equivalent_/)'`
- ⬜ [cmd] Precedence and sources: an experiment scalar beats the plug-in's and both sources are recorded; lists append in order and deduplicate; a `dockerfile`/`context` pair split across layers is refused by the existing rule; an unknown plug-in or environment is refused with its code: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_plugin_environment::layer_/)'`
- ⬜ [cmd] The settings and digests hold: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_settings_out_of_cell::/)'` and DIGEST
- ⬜ [cmd] On a real container, the agent lists the declared MCP server's tools inside the container, the evaluator-side catalog of the run equals them, an evaluator-only canary is absent by name and bytes, and everything the guard built is removed by id: `cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*plugin_environment_serves_live_mcp_tools/)'`
- ⬜ [cmd] No Python and no MCP knowledge enter the core: the campaign README's G7 and G8 commands pass on the leaf's branch
- ⬜ [cmd] SPAWN, STD (`-p candler-core -p candler`), LINUX, E2E and SUBJ pass
- ⬜ [nl] The leaf report records each bite and the answers to the verifier's attack list
- ⬜ [nl] The verifier closed with no open finding. It attacks: path escape (a symlink, `..`, case-insensitivity on macOS); which `plugins.dirs` entry wins when two plug-ins share a name; an absolute plug-in path leaking into a digest; the append's order and deduplication changing a lock silently; a fragment path outside the allowed set getting past the reader; the plug-in's egress reaching the per-cell `egress::settle`; the per-cell egress merge's read of `Resolution.preflight`: only passing reports are read, a resumed or re-planned job computes the same merged egress as the first plan (`preflight` is not persisted), no other step's derived egress gets in, and the cell digest holds the merged and settled list, not the file's list alone
**References**: [R06 D4 and the two install routes](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md); [R07 N1–N3, N6](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/htn-agent-runtime-needs-for-m5.md); [R01 §Plug-in manifest, §Agent environment and secrets](../../../../__design__/architecture.md)

## Step 1: Enter a plug-in environment as a layer
**Goal**: The preflight step `plugin_environment` reads the selected environment from the manifest and derives its values below the experiment file, with their source recorded.
**Implementation Logic**:
1. Verify first:
   - how `evaluate` finds manifests in `plugins.dirs[]`; re-use that reader, run nothing;
   - how `Layered` ranks a `Derived` value against the experiment file;
   - whether a derived value can feed the image step that reads `isolation.build.*` (the step order).
2. Guards first, seen red:
   - `equivalent_*`: an experiment selecting `p/e` against the same values and inputs written out by hand. Oracle: the hand-written experiment through the existing path. Bite: paths resolved against the cwd instead of the plug-in directory.
   - `layer_*`: a table. An experiment scalar wins and both sources are recorded; an unknown plug-in uses the existing not-found code; an unknown environment gives `isolation.plugin_environment_missing`. Bite: the plug-in winning a scalar.
3. The step makes the fragment's relative paths absolute against the plug-in directory (I14). It refuses a path that escapes the directory, through `..` or a symlink, with the existing mount or path code.
4. Each fragment value path is derived, the three lists included, recorded as `Derived {step: "plugin_environment@<token>", from: "<plugin>/<env>"}`. The step never derives `inputs`: the image step derives that root, and two steps deriving one path are refused (`DerivedTwice`). The loader is a pure `Fragment` function (manifest, selector, absolutised paths, inputs with the plug-in directory as base) that the image step, `runner/plan.rs` and the CLI reuse.
**Deliverables**: `crates/candler-core/src/resolve/steps/plugin_environment.rs`: `PluginEnvironmentStep`; the code `isolation.plugin_environment_missing`; `crates/candler-core/tests/it/guard_plugin_environment.rs`: `equivalent_*`, `layer_*`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_plugin_environment::/)'` (expected: PASS)
**Commit**: `feat(isolation): provision a plug-in's agent environment as a layer`

## Step 2: Append lists and mount the environment's inputs
**Goal**: Requirements, prune and egress lists append plug-in entries first. The plug-in's requirement sources read against its own directory. Its inputs mount as the agent's own.
**Implementation Logic**:
1. Guards first, seen red:
   - an appended requirements list (plug-in sources, then the experiment's, `treatment:` included) gives the same lock as the same list written out by hand;
   - prune and egress append and deduplicate;
   - a `dockerfile`/`context` pair taken from two layers is refused by the existing must-be-set-together rule;
   - the environment's inputs mount read-only at their `at`, and a mount-point collision with an experiment input is refused by the existing rule.

   Bite: plug-in requirements replaced instead of appended.
2. A derived value loses to the experiment file and a later step sees only winners, so the append is made where each list is read:
   - `resolve/steps/image.rs`: requirements and prune are the fragment's entries, then the winner's, deduplicated keeping the first. Each requirement source carries the directory it is relative to, so a plug-in source reads from the plug-in directory and an experiment source from the experiment file's. No new Python token: the compile and its script are unchanged;
   - `runner/plan.rs`, per cell: the plug-in step's raw `isolation.egress` from `Resolution.preflight` (a helper in `plugin_environment.rs`) is united with the cell's before `egress::settle`, so a plug-in entry that is invalid, or meets `bridge`, is refused per cell;
   - `crates/candler/src/commands/run.rs`: the fragment is loaded before `resolve_inputs`, and the plug-in's inputs resolve against the plug-in directory into the same `ResolvedInputs`, under the same declared-twice and mount-collision rules.
3. The provenance: the plug-in's value carries its `Derived` source; the merged lists are recorded in the step's report.
**Deliverables**: `crates/candler-core/src/resolve/steps/image.rs`: source base directories; `crates/candler-core/src/resolve/steps/plugin_environment.rs`: `Fragment` and the egress helper; `runner/plan.rs`: the egress union; `experiment/inputs.rs` and `commands/run.rs`: the plug-in inputs; `guard_plugin_environment.rs`: `equivalent_appended_requirements`, `layer_lists_append`, `layer_pair_from_two_layers`, `layer_inputs_mount`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_plugin_environment::/) | test(/^guard_settings_out_of_cell::/)'` (expected: PASS)
**Commit**: `feat(isolation): append a plug-in environment's lists and inputs`

## Step 3: Serve live MCP tools from a plug-in environment
**Goal**: A real container built from a plug-in environment runs its MCP server, and the agent's process lists the server's tools inside it.
**Implementation Logic**:
1. The image is the real chain: the harness base through `DockerImageService::base()`, the env layer, then the extension from the environment's own `dockerfile` + `context`. The testagent image is busybox and has no Python, so it cannot serve here.
2. The fixture plug-in `tests/fixtures/plugins/agent-env/` declares environment `tiny`:
   - a stdlib-only MCP stdio server that imports one small dependency;
   - the dependency installed by the extension from a vendored wheel (`--no-index`), not listed in `requirements` (the env layer installs with `--no-build`, and the lock compile mounts only the requirement files);
   - `mcp.*` values that launch the server;
   - the server file and a stdlib-only MCP client script as agent inputs;
   - an evaluator-only canary input.
3. The run goes through the core's isolation path:
   - `DockerIsolation::plan` and `run_request` on the resolved cell;
   - the client script as the command. It launches the server from the resolved `mcp.*` values, calls `initialize` then `tools/list`, and prints the tool names and input schemas.
4. **Trust boundary.** A harness driver's rendering of `mcp.*` into its own config (Claude Code's `.mcp.json`) is guarded in its driver crate. The core may not depend on a driver crate (I1), so here the test renders the resolved `mcp.*` values into the file the client reads.
5. `plugin_environment_serves_live_mcp_tools` asserts four things:
   - the listed tools equal the fixture's declared tools (oracle: the fixture, written by hand);
   - the core's evaluator-side catalog of the run (`runner/evidence/catalog.rs`) equals them. Verify first what the catalog captures for MCP servers, and report a spec defect if it cannot capture this server;
   - the canary is absent from the container by name and bytes;
   - everything built is removed by id.
6. Bites: the inputs not mounted, `mcp.*` dropped, or the extension not built.
7. The run is hermetic. The env build reaches the package index only for `uv venv --seed`, as `env_image_e2e` already does.
**Deliverables**: `crates/candler-core/tests/fixtures/plugins/agent-env/` (manifest, server, client script, vendored wheel, Dockerfile); `crates/candler-core/tests/it/guard_container_e2e/plugin_env_e2e.rs`: `plugin_environment_serves_live_mcp_tools`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail --run-ignored only -E 'test(/^guard_container_e2e::.*plugin_environment_serves_live_mcp_tools/)'` (expected: PASS)
**Commit**: `test(isolation): serve live MCP tools from a plug-in environment`
