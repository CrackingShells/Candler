# Driver traits

## Surface

The traits a harness driver crate implements: `HarnessDriver` describes how to prepare, launch, stop and collect one harness as data, `TraceReader` turns its output into sessions, and `SkillTree` places a treatment's skills; the core calls them and runs what they describe.

## Version

<!-- generated:version -->
<!-- versions -->
Core line: `v0.17.0`; the `candler-harness` crate has the same version.
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
| `HarnessDriver::config_check` | `(&self, plan: &Plan) -> Option<CommandSpec>` | required | - |
| `HarnessDriver::read_config_check` | `(&self, output: &[u8]) -> CheckVerdict` | required | - |
| `HarnessDriver::workspace_instruction_file` | `(&self) -> &'static str` | required | - |
| `HarnessDriver::answer` | `(&self, setting: AgentSetting) -> Answer` | required | - |
| `HarnessDriver::fact` | `(&self, fact: DriverFact) -> Answer` | required | - |
| `HarnessDriver::plan` | `(&self, cell: &Cell<'_>, workspace: &Workspace<'_>) -> Result<Plan, DriverError>` | required | - |
| `HarnessDriver::check_settings` | `(&self, cell: &Cell<'_>) -> Result<(), DriverError>` | optional | `Ok(())` |
| `HarnessDriver::gateway` | `(&self) -> Option<GatewayEnv>` | optional | `None` |
| `HarnessDriver::state_paths` | `(&self) -> Vec<StatePath>` | optional | `Vec::new()` |
| `HarnessDriver::census_from_gateway` | `(&self) -> bool` | optional | `false` |
| `HarnessDriver::loaded_instructions` | `(&self, stream: &StreamName, bytes: &[u8]) -> Vec<LoadedFile>` | optional | `Vec::new()` |
| `HarnessDriver::skills` | `(&self) -> SkillTree` | required | - |
| `HarnessDriver::base_image` | `(&self) -> BaseImage` | required | - |
| `HarnessDriver::offline_streams` | `(&self) -> Vec<StreamName>` | optional | `Vec::new()` |
| `HarnessDriver::detect` | `(&self, sample: &[u8]) -> Option<semver::Version>` | required | - |
| `HarnessDriver::session_tracker` | `(&self) -> Option<Box<dyn SessionTracker>>` | optional | `None` |
| `HarnessDriver::stop` | `(&self) -> StopSpec` | required | - |
| `HarnessDriver::live_reader` | `(&self) -> Option<Box<dyn TraceReader>>` | required | - |
| `HarnessDriver::offline_reader` | `(&self, stream: &StreamName, servers: &[String]) -> Option<Box<dyn TraceReader>>` | required | - |
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

<!-- /generated:fields -->

## Refusals

<!-- generated:refusals -->
| Code | When | Path at fault | Side to fix |
|---|---|---|---|
| `experiment.cell_value_invalid` | A cell value cannot be used by the harness driver. | The field of the value, such as `/mcp/servers/fs/command`. | experiment |
| `experiment.cell_value_missing` | A cell lacks a value that the harness driver needs, such as a hooks table that has no entry for the cell's harness. | The key of `[fixed]` that holds the value, such as `/fixed/hooks`. | experiment |
| `experiment.harness_rejected_input` | The harness's own check reports an error or a warning for a file Candler installed for the cell, such as a hook configuration the harness cannot read. | The setting in the experiment file that the file comes from, such as `/fixed/hooks`. | experiment |
| `experiment.instructions_collide` | The workspace instruction text and a declared input mount a file at the same path, the harness's workspace instruction file; the text is refused, never shadowed. | The workspace instruction text in the experiment file, `/fixed/instructions.workspace` or the factor that sets it. | experiment |
| `experiment.mcp_server_prefix_ambiguous` | Two MCP servers of a cell have names that make one tool name read two ways: the name of one, then `_`, is a prefix of the other's name, so a tool of the longer-named server cannot be told from a tool of the shorter-named one. | The table of servers, `/mcp/servers`. | experiment |
| `experiment.setting_unsupported` | The experiment sets a setting that the harness has no way to honour, such as a turn cap for a harness with no turn limit; the run is refused rather than the setting dropped. | The setting in the experiment file: a field under `/budget`, or the key of `[fixed]` that holds it, such as `/fixed/model.effort`. | experiment |
| `harness.session_record_unreadable` | A session record that the harness wrote, such as a hook payload or a stream event, cannot be read by the driver's tracker. | - | environment |
| `harness.version_unreadable` | The output of the harness version probe names no version. | - | environment |
<!-- /generated:refusals -->

## Notes

- Drivers never act on a process: they return commands, specs and readers, and the core runs them. [checked: v@0.7.0]
- A reader is synchronous and holds no clock; the core supplies each frame's place and wall time. [checked: v@0.7.0]
- `census_from_gateway()` says the census is the tool names the gateway saw in the model requests; the core reads them itself and reports them as the inventory. [checked: v@0.16.0]
- A driver that does not refuse `AgentSetting::SessionMode` also gives its `plan` a `session` and returns a `session_tracker`. [checked: v@0.7.0]
