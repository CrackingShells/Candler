# Plug-in manifest

## Surface

The manifest a plug-in author writes beside a plug-in so that Candler can read its name, version, protocol, command and environment without running anything.

## Version

<!-- generated:version -->
Wire tag: `plugin-manifest/1`.

<!-- versions -->
Wire line: `schemas@0.7.1`.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
| Path | Type | Class | Default |
|---|---|---|---|
| `command` | `array of string` | required | - |
| `environment` | `PluginEnvironment` | required | - |
| `environment.kind` | `"uv" \| "none"` | required | - |
| `environment.lock` | `RelPath` | required (environment.kind = uv) | - |
| `environment.project` | `RelPath` | required (environment.kind = uv) | - |
| `name` | `Name` | required | - |
| `protocol` | `ProtocolVersion` | required | - |
| `protocol.major` | `integer` | required | - |
| `protocol.minor` | `integer` | required | - |
| `schema` | `"plugin-manifest/1"` | required | - |
| `version` | `string` | required | - |

<!-- /generated:fields -->

## Refusals

<!-- generated:refusals -->
| Code | When | Path at fault | Side to fix |
|---|---|---|---|
| `plugin.command_empty` | The `command` of a plug-in manifest has no member, so there is nothing to start. | - | plugin |
| `plugin.describe_failed` | The evaluator did not answer `describe` with a description, or answered with something that is not one. | - | plugin |
| `plugin.describe_mismatch` | A plug-in describes itself with a name or a version that its manifest does not give. | - | plugin |
| `plugin.environment_file_unreadable` | A lock or project file named by the environment section of a plug-in manifest cannot be read. | - | plugin |
| `plugin.environment_not_ready` | The environment of a plug-in the suite names has not been built: its `ready` marker is missing. | - | environment |
| `plugin.manifest_ambiguous` | A plug-in directory holds both `plugin.toml` and `candler-plugin.toml`. | - | plugin |
| `plugin.manifest_invalid` | A plug-in manifest is not valid TOML, or does not have the shape of a manifest. | The field at fault, or its parent table when a field is missing; none at the root. | plugin |
| `plugin.manifest_unreadable` | A plug-in manifest file does not exist or cannot be read. | - | plugin |
| `plugin.name_repeated` | Two plug-in manifests have the same name. | - | plugin |
| `plugin.need_glob_invalid` | An artifact glob in the needs of a kind is not a valid glob. | - | plugin |
| `plugin.need_glob_outside_outputs` | An artifact glob in the needs of a kind is not under `outputs/`. | - | plugin |
| `plugin.need_input_reserved` | A kind needs the input kind that is reserved for the image. | - | plugin |
| `plugin.need_unsupported` | A kind declares a need that this version of Candler does not supply. | - | plugin |
| `plugin.params_schema_invalid` | The params schema a plug-in declares for a kind does not compile. | - | plugin |
| `plugin.unknown_field` | The plug-in manifest holds a key its type does not have, such as a misspelt option. | The unknown key itself. | plugin |
<!-- /generated:refusals -->

## Notes

- The document has no `free` fields: an unknown key anywhere, such as `color` at the top or `project` under `kind = "none"`, is refused with `plugin.unknown_field`. [checked: v@0.6.0]
- `command` runs with the plug-in directory as its working directory, so `./run.sh` names the file beside the manifest. [checked: v@0.6.0]
- `environment.kind = "uv"` needs both `project` and `lock`; a missing one is `plugin.manifest_invalid`. [checked: v@0.6.0]
- A plug-in directory holds `plugin.toml` or `candler-plugin.toml`, never both (`plugin.manifest_ambiguous`). [checked: v@0.6.0]
- A missing `lock` file, or an environment that `uv sync --frozen` cannot build, prints `resolve.preflight_failed` with side `environment`, not `plugin.environment_file_unreadable` or `plugin.environment_not_ready`. [checked: v@0.6.0]
