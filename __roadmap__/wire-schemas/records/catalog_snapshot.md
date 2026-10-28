# Tool-catalog snapshot

**Goal**: `CatalogSnapshot` records per run what the targeted MCP servers offered, in MCP `tools/list` shape and without loss. Its schema is generated and drift-checked, and it is tested by properties, the MCP specification and two real catalogs.
**Pre-conditions**:
- [x] `../workspace.md` and `../fixtures.md` are `done` and merged on `feat/wire-schemas`
- [ ] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
**Success Gates**:
- ⬜ `cargo nextest run -p candler-schemas catalog` passes
- ⬜ Adding a field to `CatalogSnapshot` makes `cargo nextest run -p candler-schemas drift` fail; `CANDLER_BLESS_SCHEMAS=1` then makes it pass (the scratch change is reverted)
- ⬜ `cargo clippy --workspace --all-targets -- -D warnings` passes
- ⬜ Adversarial verification and Grit review report no open finding
**References**: [R01 §Wire encodings, §Wire schema: tool-catalog snapshot](../../../__design__/architecture.md) — fields and rules; `capture_catalog_snapshot.py` and `_catalog_snapshot.py` at R04's tag — the shape HTN writes and reads today

## Step 1: Define the catalog snapshot
**Goal**: The snapshot and its members exist, and every MCP tool key survives a round trip.
**Implementation Logic**:
Module `candler_schemas::catalog`:
- `CatalogSnapshot { schema: "tool-catalog-snapshot/1", captured_at: Timestamp, provider: NameToken, environment: Option<BTreeMap<String, String>>, ext: Option<Map<String, Value>>, servers: Vec<CatalogServer> }`;
- `CatalogServer { name, source_path: RelPath, source_digest: Digest, transport: Transport, tools: Vec<Tool> }`;
- `Transport`: `stdio`, `http`, `sse`;
- `Tool`: serde names in MCP's camelCase. `name` and `inputSchema` (a JSON object) are required; `title`, `description`, `outputSchema` and `annotations` are optional. Every other key is kept in a `#[serde(flatten)]` map, and its schema allows additional properties.

Same Grit shape as the outcome record:
- `#[non_exhaustive]` structs with validating builders, and deserialisation through `try_from`;
- `Transport` is `#[non_exhaustive]`;
- errors are one `thiserror` enum, `CatalogError`.

Register the root in `generated_schemas()` and bless `catalog-snapshot.schema.json`.
**Deliverables**: `crates/candler-schemas/src/catalog.rs` (`CatalogSnapshot`, `CatalogSnapshotBuilder`, `CatalogServer`, `Transport`, `Tool`, `CatalogError`); `crates/candler-schemas/src/lib.rs` (`pub mod catalog`); `crates/candler-schemas/src/generate.rs` (the root's entry); `crates/candler-schemas/schema/catalog-snapshot.schema.json`
**Consistency Checks**: `cargo nextest run -p candler-schemas drift` (expected: PASS)
**Commit**: `feat(schemas): define the tool-catalog snapshot in MCP tools/list shape`

## Step 2: Test it in three layers
**Goal**: The snapshot accepts what MCP accepts, rejects what Candler forbids, and reads both real catalogs.
**Implementation Logic**:
1. **Properties** (`proptest`):
   - generated snapshots, with tools carrying arbitrary extra keys, are schema-valid, deserialise and round-trip equal;
   - one mutation per rule is rejected by both the schema and Rust: a missing `inputSchema`, a non-object `inputSchema`, a bad digest, an absolute `source_path`, an unknown `transport`, a `null` field.
2. **MCP specification:**
   - every `fixtures/spec/tools/*.json` validates against the vendored MCP `Tool` definition (`jsonschema`);
   - wrapped in a snapshot, each one validates against `catalog-snapshot.schema.json` and round-trips `Value`-equal, unknown keys included.
3. **Real catalogs:** an in-test conversion from HTN's `tool-catalog-snapshot/0` to `/1` sets the tag and moves `servers_dir` and `stubbed_modules` under `ext`. Both converted upstream files validate, deserialise and round-trip `Value`-equal, keeping their server and tool counts (rikyu: 2 servers, 29 tools). The verbatim `/0` file is rejected.
**Deliverables**: `crates/candler-schemas/tests/catalog_properties.rs`; `crates/candler-schemas/tests/catalog_fixtures.rs`
**Consistency Checks**: `cargo nextest run -p candler-schemas catalog` (expected: PASS)
**Commit**: `test(schemas): validate the catalog snapshot by properties, the MCP spec and real catalogs`
