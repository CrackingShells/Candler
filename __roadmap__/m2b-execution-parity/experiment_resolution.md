# Experiment resolution and preflight

**Goal**: An experiment file resolves into cells whose configuration surface (model, tools, MCP, permission mode, prompt, isolation limits, per-run caps) is classified exactly as `field_paths.toml` says, with the derived typed roots (`harness`, `treatment`, `snapshots`, `inputs`) and `isolation.image` filled by four registered preflight steps, and the per-run budget split from the job caps.
**Pre-conditions**:
- [ ] [cmd] The Phase 0 scaffold is landed: `for s in '^docs(design): ' '^build(repo): ' '^ci(repo): ' '^feat(schemas)!*: ' '^feat(orchestrator): scaffold'; do git log --format=%s origin/main..feat/m2b-execution-parity | grep -q "$s" || exit 1; done && test -s __roadmap__/m2b-execution-parity/feature_map/feature_map.json`
- [ ] [nl] api_review closed with no open finding on the scaffold's `ImageService`, `CatalogProvider`, `DockerDriver` and `experiment/paths.rs` against M2a, M3, M4 and M5
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
- [ ] [cmd] `test -s crates/candler-core/tests/oracle/vectors/field_paths.toml` (the hand-written oracle of B19; the lead writes it in S3, D5)
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_resolve_surface)'` passes, holding two B tests. B19: for every row of `field_paths.toml`, a cell value changes the cell digest, a setting does not, a typed root fills its typed field, and nothing lands under a reserved root (oracle: the TOML read by relative path, never `experiment::paths`). B20: `per_run.{wall_s,turns,max_cost_usd}` are cell values, every other `budget.*` is a job cap, and the container expiry is `wall_s + 60` (literals from architecture §10.4 and R4)
- ⬜ [cmd] STD, LINUX and SUBJ pass
- ⬜ [nl] The bite record gives a tried mutation per B test: B19 classifies `isolation.limits.memory` as a setting and the test fails; B20 treats `max_cost_usd` as `per_run` and the test fails
- ⬜ [cmd] No scaffolding: `! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/resolve/steps crates/candler-core/src/experiment/surface.rs crates/candler-core/src/experiment/budget.rs crates/candler-core/tests/guard_resolve_surface.rs`
- ⬜ [cmd] No leaf edit outside the owned paths: `git diff --name-only <base>..HEAD | grep -v -e '^crates/candler-core/src/resolve/steps/' -e '^crates/candler-core/src/experiment/\(surface\|budget\)\.rs$' -e '^crates/candler-core/tests/guard_resolve_surface.rs$' -e '^crates/candler-core/tests/scaffold_' | wc -l` prints 0
- ⬜ [nl] No verifier is required for this leaf; every gate above is `[cmd]` or the bite record
**References**:
- [R01 §Experiment file, §Budget, §Operation API](../../__design__/architecture.md) — factors, `fixed`, `per_run` caps, the preflight resolver
- [R03 §7, §8, §10.4, §11, §13](../../__reports__/m2b-execution-parity/00-architecture_v0.md) — stubs, public items, expiry (R4), the field-path table
- [R04 B19, B20](../../__reports__/m2b-execution-parity/01-test_definition_v0.md) — oracles and bites
- [M1 resolve](../m1-contracts/README.md) — `Resolver`, `PreflightStep`, `StepReport`, `Comparable::from_resolved`

## Step 1: Surface and per-run budget split
**Goal**: A cell's configuration surface is read into a typed `Surface`, and the per-run caps become cell values while the job caps stay settings.
**Implementation Logic**:
1. `experiment/surface.rs`: `Surface::from_cell(&Cell<'_>) -> Result<Surface, SurfaceError>` reads `model.id`, `model.endpoint`, `tools.allowed`, `tools.disallowed`, `mcp.*`, `permission_mode`, `prompt.template`, `prompt.values`, `isolation.network`, `isolation.limits.*`. A missing required path is `SurfaceError::Missing(FieldPath)`; a wrong JSON type is `SurfaceError::Type`. It refuses any value below a reserved root (`harness`, `treatment`, `snapshots`, `inputs`, `environments`) and any absolute-path string, with the same refusals as `Comparable::from_resolved`. It never reads `request.*` or `isolation.build.*`: those are settings.
2. `experiment/budget.rs`: `to_caps(&Budget) -> RunCaps` over M1's `PerRunCaps {wall_s, turns, max_cost_usd}`. `RunCaps` holds the three optional caps; `RunCaps::flags(&self) -> Vec<String>` gives `--max-turns <n>` and `--max-budget-usd <x>` for the caps that are set and nothing for `wall_s` (the wall cap is enforced by the supervisor deadline and the container expiry, R4). The expiry is `RunCaps::expiry(&self) -> Option<Duration>`, the wall cap plus the S3 constant `EXPIRY_MARGIN`, so the supervisor deadline (`wall_s`) always fires first.
3. `per_run.*` are cell values and every other `budget.*` path is a job cap that never enters the cell; the split follows the S3 `is_setting` and `paths.rs` as they stand and this leaf edits neither.
4. `tests/scaffold_resolution.rs` (class A, deleted in Step 5): a table of required-path and wrong-type cases for `from_cell`, and `flags()` for set and unset caps.
**Deliverables**: `crates/candler-core/src/experiment/surface.rs` (`Surface`, `SurfaceError`, `Surface::from_cell`); `crates/candler-core/src/experiment/budget.rs` (`RunCaps`, `to_caps`, `RunCaps::flags`, `RunCaps::expiry`); `crates/candler-core/tests/scaffold_resolution.rs`
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(scaffold_resolution)'` (expected: PASS)
**Commit**: `feat(experiment): read the configuration surface and split the budget`

## Step 2: Harness and treatment identity steps
**Goal**: The typed roots `harness` and `treatment` are derived from the settings `request.harness` and `request.treatment` (F2 ruling (a), R21), never written as cell values.
**Implementation Logic**:
1. `resolve/steps/identity.rs`: `HarnessIdentityStep` implements `PreflightStep` with `name() = "harness_identity"`. `run(Arc<ResolveContext>)` reads the setting `request.harness` (`HarnessRequest {name, version: Option<VersionReq>}`), asks `M2bServices.images` for the Claude Code version (`ImageService::cc_version(&ImageRef)`, the image from the `image_id` step's derived value), checks it against both `M2bServices.harness_supported` (a `semver::VersionReq` that `cli_run` fills from the driver's `supported()`, D6) and the requested `VersionReq`, and returns `StepReport::derive("harness", json!({name, version, driver}))`. A version outside either requirement is a `PreflightError` naming the version and the failing requirement.
2. `TreatmentIdentityStep` (`name() = "treatment_identity"`) reads the settings `request.treatment` (the typed table `{name, tag, commit}`) and `request.treatment_source` (a path setting, D2: a location, not identity), resolves the tag to a commit with `git -C <source> rev-parse <tag>^{commit}` through `M2bServices.supervisor` (`run_captured`, I15), and derives `treatment` as `TreatmentRef {name, tag, commit}`. A tag that does not resolve, or whose commit differs from the requested `commit`, is a `PreflightError`.
3. Both steps derive values only at typed roots. Neither writes below a reserved root, and the derived `harness` has no path below it.
4. Class A in `tests/scaffold_resolution.rs`: the derived shapes from a recorded fake `ImageService`, a fixture git repo for the tag, and the refusal of a version outside `harness_supported` and of one outside the request (each with a positive control).
**Deliverables**: `crates/candler-core/src/resolve/steps/identity.rs` (`HarnessIdentityStep`, `TreatmentIdentityStep`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(scaffold_resolution)'` (expected: PASS)
**Commit**: `feat(resolve): derive the harness and treatment identities`

## Step 3: Image and input-digest steps
**Goal**: `isolation.image` holds the local image ID, and the `isolation.build` request enters the cell only as an `inputs[]` digest.
**Implementation Logic**:
1. `resolve/steps/image.rs`: `ImageIdStep` (`name() = "image_id"`) reads the settings `isolation.build.dockerfile` and `isolation.build.context` and the derived `isolation.env_lock`, hashes the Dockerfile and the context listing into a `LockDigest`, calls `M2bServices.images.ensure(&LockDigest, &BuildSetting)`, and derives `isolation.image` as the image ID `sha256:...` (never a tag) and the digest as an `inputs[]` entry. Because `isolation.image` is a cell value and `isolation.build.*` a setting (R03 §11), changing the build request moves the cell only through the new image ID and the input digest.
2. The step depends on the `ImageService` trait, never on the image module (R14).
3. Class A: a fake `ImageService` returning two IDs shows that two Dockerfile contents give two digests and one content gives one.
**Deliverables**: `crates/candler-core/src/resolve/steps/image.rs` (`ImageIdStep`, `BuildInputs`, `build_input_digest`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(scaffold_resolution)'` (expected: PASS)
**Commit**: `feat(resolve): derive the image id and the build input digest`

## Step 4: Catalog step and registration
**Goal**: The catalog digest enters `snapshots`, and one function registers the four M2b steps in dependency order.
**Implementation Logic**:
1. `resolve/steps/catalog.rs`: `CatalogStep` (`name() = "catalog"`) reads the derived image and the cell, calls `M2bServices.catalog.capture(&ImageRef, &Cell<'_>)`, and derives `snapshots` as the catalog digest; the catalog itself is kept for the run evidence leaf, never in the cell.
2. `register_m2b_steps(r: Resolver, services: M2bServices) -> Result<Resolver, ResolveError>` (D3), with `M2bServices { supervisor: Arc<Supervisor>, images: Arc<dyn ImageService>, catalog: Arc<dyn CatalogProvider>, harness_supported: semver::VersionReq }` declared by S3, not here; the function calling `Resolver::register` for `image_id`, `harness_identity`, `treatment_identity`, `catalog` in that order. The module declaration itself belongs to S3.
3. Class A: a registration order test over a fake resolver run, and the catalog step with a fake provider.
**Deliverables**: `crates/candler-core/src/resolve/steps/catalog.rs` (`CatalogStep`); `crates/candler-core/src/resolve/steps/mod.rs` body (`register_m2b_steps`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(scaffold_resolution)'` (expected: PASS)
**Commit**: `feat(resolve): register the m2b preflight steps`

## Step 5: B19 and B20 guards, scaffold deletion and bite record
**Goal**: The surface classification and the budget split are held by two guards with independent oracles, and no scaffolding stays.
**Implementation Logic**:
1. `tests/guard_resolve_surface.rs` (`#![cfg(test)]`). B19, `b19_every_path_class`: reads `tests/oracle/vectors/field_paths.toml` by relative path, and for each row builds two cells that differ at that path. Cell value: the cell digests differ. Setting: they are equal. Typed root: `Comparable` carries the typed field and no entry sits below the root. Each class has its positive control. Doc lines: `Guards: I7, I12 — the surface classification`, `Oracle: field_paths.toml, hand-written`, `Bite: classify isolation.limits.memory as a setting`.
2. B20, `b20_budget_split_and_expiry`: `per_run.*` change the digest, `budget.max_runs`, `budget.max_parallel`, `budget.max_tokens`, `budget.max_cost_usd`, `budget.wall_s` do not; `RunCaps::expiry` for `wall_s = 3600` equals 3660 s (literal, R4). Doc lines: `Guards: I20 — the budget split`, `Oracle: R03 §10.4 formula`, `Bite: treat max_cost_usd as per_run`.
3. Delete `tests/scaffold_resolution.rs` and every `mod scaffold` in the owned files. Try each mutation, restore with `git checkout -- <file>`, and write the bite record in the leaf report.
**Deliverables**: `crates/candler-core/tests/guard_resolve_surface.rs` (`b19_every_path_class`, `b20_budget_split_and_expiry`); the leaf report's bite record
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_resolve_surface)' && ! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core` (expected: PASS)
**Commit**: `test(resolve): guard the surface classification and the budget split`
