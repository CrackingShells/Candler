# Resolve: config loader, value layers, preflight steps

**Goal**: `candler_core::resolve` loads the project and user config, layers values by R01's resolution order with a source for every value, and runs registered preflight steps in order, so M2a and M2b each add their checks as steps without changing `resolve`.
**Pre-conditions**:
- [ ] [cmd] DEP(wire_foundations) holds: `git log --first-parent --format=%s feat/m1-contracts | grep -qx 'merge(.*): land wire_foundations'`
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
**Success Gates**:
- ✅ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_resolve)'` passes, holding four B tests:
  - **precedence**: a six-row table written from R01 §Operation API, row k setting one path at levels k..6 only (explicit argument, experiment or suite file, project config, user config, derived, core default), asserts the winning value and its `ValueSource`; every row also asserts that `sources` covers exactly the paths of `values` and `settings` (I12), and that `store.root` and an explicit path sit in `settings`;
  - **config refusals**: an unknown key in each table of the two files, the tables listed by hand from R01 §Configuration, is refused with an error naming the file and the key; `slots.count` in a project config is refused naming the file; each refusal has the valid file as positive control;
  - **path bases** (I14): `store.root = "data/store"` in `<root>/candler.toml` resolves to `<root>/data/store` when the working directory is `<root>/a/b`, and an explicit argument `data/store` resolves to `<root>/a/b/data/store`;
  - **steps**: a second step sees the value the first derived, with source `derived {step}`; registering a duplicate name is refused; two failing steps out of three are both reported, the third still runs, and resolution fails
- ✅ [cmd] `cargo nextest run -p candler-core -E 'binary(public_surface)'` passes, naming `Resolver`, `PreflightStep`, `ResolveContext`, `ResolveInput`, `Resolution`, `StepReport`, `Config`, `ProjectConfig`, `UserConfig` and `ConfigEnv`, and asserting `Box<dyn PreflightStep>` is `Send + Sync`
- ✅ [cmd] STD and SUBJ pass
- ✅ [nl] The leaf report has a bite mutation per B test; api_review (end of this level) reports no open finding on this leaf
**References**: [R01 §Operation API (resolution order, Value sources, Preflight steps), §Configuration, §Invariants (I12, I14)](../../../__design__/architecture.md) — every rule; [R07](../../candler-v1/contracts/parity/README.md) — M2a registers the evaluator steps, M2b the harness and secret steps

## Step 1: Load the project and user config
**Goal**: Both config files are found, parsed, validated and their paths made absolute.
**Implementation Logic**:
1. `ConfigEnv {xdg_config_home: Option<PathBuf>, home: Option<PathBuf>}`, built by the caller (`ConfigEnv::from_process()` reads the two variables), so tests never touch the real environment.
2. `ProjectConfig::discover(cwd: &Path) -> Result<Option<ProjectConfig>, ConfigError>`: the nearest ancestor of `cwd` holding `candler.toml`. `ProjectConfig {root: PathBuf, file: PathBuf, store_root?: PathBuf, plugin_dirs: Vec<PathBuf>}` from `[store] root` and `[plugins] dirs`; relative paths resolve against the file's directory.
3. `UserConfig::load(&ConfigEnv) -> Result<UserConfig, ConfigError>`: `$XDG_CONFIG_HOME/candler/config.toml`, else `<home>/.config/candler/config.toml`; a missing file is an empty config. `UserConfig {file?: PathBuf, slots_count?: u32}` from `[slots] count`.
4. Unknown keys are refused at every table (`deny_unknown_fields`-equivalent on read), naming the file and the dotted key; `slots` in a project config is refused naming the file.
5. `Config {cwd: PathBuf, project: Option<ProjectConfig>, user: UserConfig}`; `Config::load(cwd, &ConfigEnv)`.
**Deliverables**: `crates/candler-core/src/resolve/config.rs` (`ConfigEnv`, `ProjectConfig`, `UserConfig`, `Config`, `ConfigError`); `crates/candler-core/src/resolve/mod.rs` (module declarations only; consumers import by module path, AGENTS.md)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(resolve): load the project and user config`

## Step 2: Layer resolved values with their sources
**Goal**: Values from every layer resolve to one value per path, each with the source it came from.
**Implementation Logic**:
1. `ResolveInput {explicit: Vec<(FieldPath, JsonValue)>, file: Option<FileValues {kind: experiment | suite, values: Vec<(FieldPath, JsonValue)>}>, core_defaults: Vec<(FieldPath, JsonValue)>}`; `ResolveInput::explicit_path(path, &Path)` makes a relative path absolute against `Config::cwd` (I14); a path is a location, so it resolves into `settings`, and a preflight step that reads the file derives its digest under `inputs` (D4).
2. Config values enter as paths: `store.root`, `plugins.dirs` (`project_config`), `slots.count` (`user_config`); the core default `store.root` is `<project root>/.candler/store`, and with no project `$XDG_STATE_HOME/candler/store`, else `~/.local/state/candler/store` (api_review G6). Two steps deriving one path are refused with `ResolveError::DerivedTwice` (G13). Config values and explicit paths are operational settings: they land in `Resolution.settings`, never in `values`, so they never reach a cell digest.
3. `Layered::resolve` picks, per path, the highest layer that sets it: explicit argument > experiment or suite file > project config > user config > derived > core default; the source is `explicit_argument`, `experiment_file`, `project_config`, `user_config`, `derived {step}` or `core_default`. `Resolution {values: Vec<ResolvedValue>, settings: Vec<ResolvedValue>, sources: Vec<SourceEntry>, preflight: Vec<StepReport>}`: `values` and `settings` sorted by path and disjoint, `sources` sorted and covering exactly their union.
**Deliverables**: `crates/candler-core/src/resolve/layers.rs` (`ResolveInput`, `FileValues`, `Resolution`, `Layered`; `FileKind` dropped, since `ValueSource` has no suite variant and the suite-file source is deferred to M2a)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(resolve): layer resolved values with their sources`

## Step 3: Run registered preflight steps in order
**Goal**: Steps run in registration order, each seeing what the previous ones derived, and every failure is reported.
**Implementation Logic**:
1. `pub trait PreflightStep: Send + Sync { fn name(&self) -> &NameToken; fn run(&self, ctx: Arc<ResolveContext>) -> BoxFuture<'static, Result<StepReport, PreflightError>>; }` (`futures::future::BoxFuture`).
2. `ResolveContext {config: Arc<Config>, values: Vec<ResolvedValue>, settings: Vec<ResolvedValue>, sources: Vec<SourceEntry>}`, rebuilt after each step with that step's derived values layered in.
3. `StepReport {step: NameToken, derived: Vec<(FieldPath, JsonValue)>, checks: Vec<CheckResult {name: NameToken, passed: bool, explanation: String}>}`; a step that returns `Err` or a failed check has its derived values dropped, and later steps still run.
4. `Resolver::new(Arc<Config>)`, `register(self, Box<dyn PreflightStep>) -> Result<Self, ResolveError>` (a duplicate name is refused), `async fn resolve(&self, ResolveInput) -> Result<Resolution, ResolveError>`; `ResolveError::Preflight {reports}` lists every failure once all steps ran. `resolve` itself registers no step.
5. `tests/guard_resolve.rs`: the four B tests of the Success Gates, on `tempfile` directories with a hand-built `ConfigEnv`. Then delete the leaf's scaffolding and write the bite record.
**Deliverables**: `crates/candler-core/src/resolve/preflight.rs` (`PreflightStep`, `ResolveContext`, `StepReport`, `CheckResult`, `PreflightError`, `Resolver`, `ResolveError`); `crates/candler-core/tests/guard_resolve.rs`; `crates/candler-core/tests/public_surface.rs` (the resolve items)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_resolve) | binary(public_surface)' && ! git grep --untracked -n 'mod scaffold' -- crates/candler-core/src/resolve` (expected: PASS)
**Commit**: `feat(resolve): run registered preflight steps in order`
