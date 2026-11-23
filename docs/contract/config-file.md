# Configuration files

## Surface

The keys of the project configuration `candler.toml` and of the user configuration `config.toml`; Candler reads them to find the store, the plug-in directories, the run slots and the gateway's trust.

## Version

<!-- generated:version -->
<!-- versions -->
Core line: `v0.13.0`.
<!-- /versions -->
<!-- /generated:version -->

## Fields

<!-- generated:fields -->
### `candler.toml` (project)

| Table | Key | Meaning |
|---|---|---|
| `store` | `root` | The store root; a relative path is read from the project root. |
| `plugins` | `dirs` | The directories that hold plug-ins; relative paths are read from the project root. |

### `config.toml` (user)

| Table | Key | Meaning |
|---|---|---|
| `slots` | `count` | How many run slots the machine offers. |
| `slots` | `evaluators` | How many of the slots evaluators may use. |
| `gateway` | `upstream_ca` | A PEM file whose certificates the gateway trusts for its upstream. |

<!-- /generated:fields -->

## Refusals

<!-- generated:refusals -->
| Code | When | Path at fault | Side to fix |
|---|---|---|---|
| `config.unknown_key` | A configuration file holds a key that no table defines, such as a misspelt setting. | The unknown key in the configuration file named in the message. | config |
| `config.wrong_type` | A configuration key holds a value of the wrong type. | The key in the configuration file named in the message. | config |
| `config.machine_wide_key` | A project configuration sets a machine-wide key, which only the user configuration may set. | The key in the project configuration file named in the message. | config |
| `config.toml_invalid` | A configuration file is not valid TOML. | - | config |
| `config.file_unreadable` | A configuration file exists and cannot be read. | - | config |
| `config.upstream_ca_invalid` | `gateway.upstream_ca` names a file that cannot be read or holds no PEM certificate. | The `gateway.upstream_ca` key in the user configuration file named in the message. | config |
<!-- /generated:refusals -->

## Notes
