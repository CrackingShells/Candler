# Plug-in manifest

## Surface

The manifest a plug-in author writes beside a plug-in so that Candler can read its name, version, protocol, command and environment without running anything.

## Version

<!-- generated:version -->
Wire tag: `plugin-manifest/1`.

<!-- versions -->
Wire line: `schemas@0.6.0`.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
| Path | Type | Class | Default |
|---|---|---|---|
| `command` | `array of string` | required | - |
| `environment` | `PluginEnvironment` | required | - |
| `environment.kind` | `"uv" \| "none"` | required | - |
| `environment.lock` | `RelPath` | required | - |
| `environment.project` | `RelPath` | required | - |
| `name` | `Name` | required | - |
| `protocol` | `ProtocolVersion` | required | - |
| `protocol.major` | `integer` | required | - |
| `protocol.minor` | `integer` | required | - |
| `schema` | `"plugin-manifest/1"` | required | - |
| `version` | `string` | required | - |

<!-- /generated:fields -->

## Refusals

<!-- generated:refusals -->

## Notes
