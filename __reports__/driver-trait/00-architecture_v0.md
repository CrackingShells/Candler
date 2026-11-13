# Driver trait consolidation — Architecture Analysis (v0)

Date: 2026-11-13

This note fixes the shape of the harness surface before M5. It answers findings C1, C2, C3, C4 and C7 of slice C in [`../interlude-audit/00-audit_slices_v0.md`](../interlude-audit/00-audit_slices_v0.md). Each claim was checked against `ad343bf`; all five still hold.

## Executive Summary
- **Problem.** A harness is defined in three places: its driver, the CLI's `DriverEntry` (`crates/candler/src/drivers.rs`), and parallel copies in the other driver. The two drivers duplicate a tar reader, prompt rendering, cell paths and version parsing, and build constants through fallible constructors whose errors are then dropped with `.ok()`.
- **Proposed change.** The `HarnessDriver` trait owns everything the CLI glue holds. Shared code moves to `candler-harness` and `candler-schemas`. Constants become infallible.
- **Staging.** The $5 session budget does not cover all of it. The first PR implements C1 and C3, plus the part of C4 that makes the registry infallible: driver and reader constructors, `ClaudeCodeError`, `OpenCodeError`, and the `live_reader` drops. C2, C7 and the rest of C4 follow in a second PR, as this note designs them.
- **Effect.** `DriverEntry`, `BuildError`, `ClaudeCodeError` and `OpenCodeError` are deleted, and so are about 400 duplicated lines. The registry is built once per process.

## Current State
```text
CLI drivers.rs ── DriverEntry { driver, skills: dyn SkillTree, base_context,
                                offline_streams, reader: fn, detect: fn }
                  Registry::builtin() -> Result   (called 6x: ingest x2, run, resolve, supervise x2;
                                                   ingest's reader factory calls process::exit)
claude-code ── ClaudeSkills (tar reader, full pax)  render_prompt  argv in driver.rs AND session.rs
opencode    ── OpenCodeSkills (tar reader, path-only pax)  render_prompt (copy)
both        ── VersionReq::parse(SUPPORTED).unwrap_or_else(nothing)   ::new() -> Result<_, *Error>
Dockerfiles ── ARG CC_VERSION=2.1.311 / ARG OC_VERSION=2.2.7   (not tied to the driver)
```

## Proposed State
```text
trait HarnessDriver {
    ...existing methods...
    fn skills(&self) -> SkillTree;                          // C1, C3: a value, not a trait object
    fn base_image(&self) -> BaseImage;                      // C1, C7: { context: &'static str,
                                                            //   build_args: Vec<(String, String)> }
    fn offline_streams(&self) -> Vec<StreamName>;           // C1
    fn detect(&self, sample: &[u8]) -> Option<semver::Version>;   // C1: supported or not
}
core:  Recognised.supported = driver.supported().matches(&version)
core:  type ReaderFactory = dyn Fn(&str) -> Option<Box<dyn TraceReader>> + Send + Sync  // harness name
CLI:   Registry { BTreeMap<String, Arc<dyn HarnessDriver>> }, built once in a LazyLock (infallible)
```

### Shared code
| What | Home | Replaces |
|---|---|---|
| `SkillTree { source_root, target_root }` with `archive_command` and `files` | `candler-harness::skills` | trait `SkillTree`, `ClaudeSkills`, `OpenCodeSkills`, both tar readers. The Claude Code reader is kept, since it handles `path` and `linkpath` pax records. |
| `plain_segment(&str) -> bool` | `candler-schemas::primitives` | the four copies |
| Cell paths (prompt, model, treatment, …) | `candler-schemas::execution` | `core experiment/paths.rs`, `execution.rs`, both drivers |
| Typed `Cell` accessors (`cell.prompt()`, `cell.model()`, …) and one `render_prompt` | `candler-harness::spec` | `render_prompt`, `is_key`, `scalar_text` in both drivers |
| `fn argv(cell, workspace) -> Vec<String>` | `candler-harness-claude-code` | the argv built in both `driver.rs` and `session.rs` |

### Constants (C4, C7)
- **Infallible constants.** Each `supported()` requirement is built from `semver::Comparator` literals, and each version from `Version::new`. Reader identities use a `const fn` constructor that rejects bad text at compile time. `nothing()` and every `new() -> Result` go.
- **Variants and rows.** `ClaudeCodeError`, `OpenCodeError`, `DriverError::HarnessName` and the CLI's `BuildError` are deleted. So are their `[internal.*]` rows in `docs/contract/codes.toml`; those rows are not released codes. If a released code reports only a bad constant, it is marked `retired`, never removed (CONTRIBUTING: `codes.toml` is append-only).
- **No silent drops.** The `.ok()` drops go: opencode's `stop()` interrupt script, both `live_reader()` calls, and `claude_code_detect`/`opencode_detect`.
- **Version source.** The driver's `base_image().build_args` gives `CC_VERSION` / `OC_VERSION`. `ensure_base` passes them as `--build-arg` and hashes them into the base tag. The Dockerfiles keep the `ARG` with no default, so a build without the argument fails.

## Decisions
- **`SkillTree` is a struct, not a trait.** The two layouts differ only in their roots. One parameterised reader cannot drift the way the two did over pax.
- **The registry is infallible.** Once constants cannot fail, `builtin()` has no error to return. One `LazyLock` serves every verb, so `supervise` and `ingest` stop building it twice.
- **`detect` lives on the driver.** The CLI's `opencode_detect` and `claude_code_detect` were driver knowledge held in the CLI. The core alone decides support, from `supported()`.

## Next step (out of scope): one `plan(cell)` (C8)
`fn plan(&self, cell: &Cell, ws: &Workspace) -> Result<Plan, DriverError>`, where `Plan { rows: Vec<SettingRow>, prepared: Prepared, command: CommandSpec, session: Option<SessionSpec> }`. `settings_table`, `check_settings`, `prepare`, `command` and `session` become reads of the one plan, which the runner computes once per cell. That removes opencode's parallel `table()`/`settings()` and the test that guards their agreement.

## Ranked risk register
| # | Risk | Mitigation |
|---|---|---|
| 1 | Moving cell paths changes a wire struct | Run the schema drift test with `CANDLER_BLESS_SCHEMAS=1`; paths are constants, not fields |
| 2 | The unified tar reader rejects an archive the opencode one took | It is the superset (it adds `linkpath`); the conformance suite runs both drivers through it |
| 3 | A build-arg change does not rebuild the base image | Hash the build args into the base tag |
| 4 | Deleting a `codes.toml` row breaks the contract pages | Delete only `[internal.*]` rows; the contract-pages test regenerates them |
