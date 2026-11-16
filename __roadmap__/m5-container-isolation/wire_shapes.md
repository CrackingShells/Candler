# Wire shapes

**Goal**: The wire shapes M5 needs exist in one `feat(schemas)` release commit, and the core admits them in a second commit. Each commit leaves the workspace compiling, and no existing document becomes invalid.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] OWNED: `crates/candler-schemas/**`; in the core only `resolve/layers.rs` (`LEAVES`, `is_location`), `tests/oracle/vectors/field_paths.toml` (rows added only), `tests/it/guard_settings_out_of_cell.rs` (one row), and the role arms at `isolation/container/guard.rs` and `jobs/lost.rs`
**Success Gates**:
- ✅ [cmd] `cargo check --workspace --all-targets` passes on each of the leaf's two commits: `for c in $(git rev-list --reverse <base>..HEAD); do git checkout -q $c && cargo check --workspace --all-targets -q || exit 1; done; git checkout -q -`
- ✅ [cmd] Old documents stay valid, the new map reads set and unset, unknown keys and paths outside the allowed set are refused: `cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/^guard_wire_shapes::agent_environments_/) | test(/^guard_execution_wire::container_role_egress_/)'`
- ✅ [cmd] The new setting never reaches a cell: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_settings_out_of_cell::/)'`
- ✅ [cmd] DRIFT, DIGEST and SUBJ pass; exactly one release-triggering commit touches `crates/candler-schemas`, it is `feat(schemas)`, and it touches no other crate (the campaign README's G6 form over `<base>..HEAD`)
- ✅ [nl] The leaf report records step 1's answer (whether `ValueSource::Derived {step, from}` records a plug-in fragment truthfully), each guard's bite, and the allowed fragment paths as written
**References**: [R01 §Plug-in manifest, §Agent environment and secrets](../../__design__/architecture.md) — the configuration table; [R06 D4, D5](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md) — the shapes

## Step 1: Declare the shapes on the schemas line
**Goal**: The plug-in manifest can carry named agent environments, the experiment has the two new paths, and a container record can have the role `egress`.
**Implementation Logic**:
1. Verify first: read `ValueSource` (`crates/candler-schemas/src/values.rs`) and the preflight code that writes `Derived`. Can a step record a value as `Derived {step: "plugin_environment@<token>", from: "<plugin>/<env>"}` truthfully? Answer in the report. Only if it cannot, add one `ValueSource` variant in this same commit, and report why.
2. Guards first, seen red.
   - `agent_environments_*` in `guard_wire_shapes`: every committed manifest vector still reads; one vector has the map set; an unknown key inside an environment and a value path outside the allowed set are each refused, the key with `plugin.unknown_field` and the path with `plugin.environment_path_unsupported`.
   - `container_role_egress_*` in `guard_execution_wire`: a `container-record/1` with role `egress` round-trips, and every committed record vector still reads.
   - Oracles: the committed vectors and the allowed set as written in R06 D4.
3. `PluginManifest` gains `agent_environments: BTreeMap<Name, AgentEnvironment>` (optional, absent when empty, `deny_unknown_fields`).
   - `AgentEnvironment {values: BTreeMap<FieldPath, JsonValue>, inputs: Vec<AgentInput>}`.
   - `AgentInput` is the experiment's input shape without `audience`: agent-only by construction.
   - The allowed value paths are the prefixes `isolation.build.` and `mcp.` and the path `isolation.egress`, checked in the reader.
4. `execution/paths.rs` gains `ISOLATION_EGRESS` (a cell value) and `ISOLATION_BUILD_PLUGIN` (a setting by its `isolation.build.` prefix). The field-class vectors gain their rows. The container-record role gains `Egress`.
5. Regenerate the schemas (`CANDLER_BLESS_SCHEMAS=1`), and add the code to the schemas section of `docs/contract/codes.toml` (append only).
**Deliverables**: `crates/candler-schemas/src/evaluation.rs`: `PluginManifest.agent_environments`, `AgentEnvironment`, `AgentInput`; `crates/candler-schemas/src/execution/paths.rs`: `ISOLATION_EGRESS`, `ISOLATION_BUILD_PLUGIN`; the container-record role `Egress`; `crates/candler-schemas/schema/*.schema.json` regenerated; `crates/candler-schemas/tests/it/guard_wire_shapes.rs`: `agent_environments_*`; `guard_execution_wire.rs`: `container_role_egress_*`
**Consistency Checks**: `cargo nextest run -p candler-schemas --no-tests=fail -E 'test(/agent_environments_|container_role_egress_/)'` (expected: PASS)
**Commit**: `feat(schemas): declare agent environments and an egress allowlist`

## Step 2: Admit the paths and the role in the core
**Goal**: The core's path tables and role matches know the new paths and role, so the workspace compiles and the new setting stays out of every cell.
**Implementation Logic**:
1. `resolve/layers.rs`: add both paths to `LEAVES`, and to `is_location` where a path names a location (`isolation.build.plugin` names a plug-in environment, not a file, so verify which applies).
2. `field_paths.toml`: add one row per path, with its class. Additions only; DIGEST forbids removed lines.
3. `guard_settings_out_of_cell`: add a row showing that `isolation.build.plugin` never reaches a cell. Bite: the path classed as a cell value.
4. The role matches at `isolation/container/guard.rs` and `jobs/lost.rs`: `egress` is destroyed under the same rule as `gateway`, with an explicit arm and no catch-all.
**Deliverables**: `crates/candler-core/src/resolve/layers.rs`: `LEAVES`, `is_location`; `crates/candler-core/tests/oracle/vectors/field_paths.toml`: two rows; `crates/candler-core/tests/it/guard_settings_out_of_cell.rs`: the new row; the `Egress` arms in `isolation/container/guard.rs` and `jobs/lost.rs`
**Consistency Checks**: `cargo check --workspace --all-targets && cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_settings_out_of_cell::/) | test(/^guard_container_guard::/)'` (expected: PASS)
**Commit**: `feat(isolation): admit the egress and plug-in environment paths`
