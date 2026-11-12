# Driver traits

## Surface

The traits a harness driver crate implements: `HarnessDriver` describes how to prepare, launch, stop and collect one harness as data, `TraceReader` turns its output into sessions, and `SkillTree` places a treatment's skills; the core calls them and runs what they describe.

## Version

<!-- generated:version -->
<!-- versions -->
Core line: `v0.7.0`; the `candler-harness` crate has the same version.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
### `HarnessDriver`

| Path | Signature | Class | Default |
|---|---|---|---|
| `HarnessDriver::id` | `(&self) -> HarnessId` | required | - |
| `HarnessDriver::supported` | `(&self) -> semver::VersionReq` | required | - |
| `HarnessDriver::version_probe` | `(&self) -> CommandSpec` | required | - |
| `HarnessDriver::parse_version` | `(&self, output: &[u8]) -> Result<semver::Version, DriverError>` | required | - |
| `HarnessDriver::capabilities` | `(&self) -> Capabilities` | required | - |
| `HarnessDriver::prepare` | `(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Prepared, DriverError>` | required | - |
| `HarnessDriver::command` | `(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<CommandSpec, DriverError>` | required | - |
| `HarnessDriver::settings_table` | `(&self) -> Vec<SettingRow>` | optional | `Vec::new()` |
| `HarnessDriver::check_settings` | `(&self, cell: &Cell<'_>) -> Result<(), DriverError>` | optional | `Ok(())` |
| `HarnessDriver::gateway` | `(&self) -> Option<GatewayEnv>` | optional | `None` |
| `HarnessDriver::state_paths` | `(&self) -> Vec<StatePath>` | optional | `Vec::new()` |
| `HarnessDriver::census_stream` | `(&self) -> Option<StreamName>` | optional | `None` |
| `HarnessDriver::session` | `(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Option<SessionSpec>, DriverError>` | optional | `Ok(None)` |
| `HarnessDriver::session_tracker` | `(&self) -> Option<Box<dyn SessionTracker>>` | optional | `None` |
| `HarnessDriver::stop` | `(&self) -> StopSpec` | required | - |
| `HarnessDriver::live_reader` | `(&self) -> Option<Box<dyn TraceReader>>` | required | - |
| `HarnessDriver::offline_reader` | `(&self, stream: &StreamName) -> Option<Box<dyn TraceReader>>` | required | - |
| `HarnessDriver::classify_exit` | `(&self, exit: &ProcessExit, sessions: &[Session]) -> EndedBy` | required | - |
| `HarnessDriver::classify_end` | `(&self, exit: &ProcessExit, sessions: &[Session], observed: &Observed) -> EndedBy` | optional | `self.classify_exit(exit, sessions)` |
| `HarnessDriver::collect` | `(&self, workspace: &Workspace<'_>, observed: &Observed) -> Vec<CollectSpec>` | required | - |

### `TraceReader`

| Path | Signature | Class | Default |
|---|---|---|---|
| `TraceReader::id` | `(&self) -> NameVersion` | required | - |
| `TraceReader::framing` | `(&self) -> Framing` | required | - |
| `TraceReader::detect` | `(&self, sample: &[u8]) -> Option<(FormatVersion, Confidence)>` | required | - |
| `TraceReader::feed` | `(&mut self, frame: FrameIn<'_>) -> Result<Vec<SessionEvent>, ReaderError>` | required | - |
| `TraceReader::observed` | `(&self) -> Observed` | required | - |
| `TraceReader::finish` | `(self: Box<Self>) -> Result<Vec<Session>, ReaderError>` | required | - |

### `SkillTree`

| Path | Signature | Class | Default |
|---|---|---|---|
| `SkillTree::archive_command` | `(&self, treatment: &TreatmentRef, source: &Path) -> Result<CommandSpec, DriverError>` | required | - |
| `SkillTree::files` | `(&self, treatment: &TreatmentRef, archive: &[u8]) -> Result<Vec<PreparedFile>, DriverError>` | required | - |

<!-- /generated:fields -->

## Refusals

<!-- generated:refusals -->
| Code | When | Path at fault | Side to fix |
|---|---|---|---|
| `experiment.cell_value_invalid` | A cell value cannot be used by the harness driver. | - | experiment |
| `experiment.cell_value_missing` | A cell lacks a value that the harness driver needs. | - | experiment |
| `experiment.setting_unsupported` | The experiment sets a setting that the harness has no way to honour, such as a turn cap for a harness with no turn limit; the run is refused rather than the setting dropped. | The setting in the experiment file: a field under `/budget`, or the key of `[fixed]` that holds it, such as `/fixed/model.effort`. | experiment |
| `harness.session_record_unreadable` | A session record that the harness wrote, such as a hook payload or a stream event, cannot be read by the driver's tracker. | - | environment |
| `harness.version_unreadable` | The output of the harness version probe names no version. | - | environment |
<!-- /generated:refusals -->

## Notes

- Drivers never act on a process: they return commands, specs and readers, and the core runs them. [checked: v@0.7.0]
- A reader is synchronous and holds no clock; the core supplies each frame's place and wall time. [checked: v@0.7.0]
- `census_stream()` names the raw stream the gateway's census is stored under; the core reads the gateway census itself and reports it as the inventory. [checked: v@0.7.0]
- A driver that sets `Capabilities::session` also returns `session` and `session_tracker`. [checked: v@0.7.0]
