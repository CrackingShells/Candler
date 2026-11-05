# candler-harness

The interface between the core and an agent harness. The crate holds data types and two traits and
no behaviour: a driver never spawns, signals or opens a socket. The core runs every command and
HTTP call a driver describes. The traits are specified in
[R01 §Harness drivers](../../__design__/architecture.md).

## Public modules

`lib.rs` declares three modules and re-exports nothing.

| Module | Holds |
|---|---|
| `spec` | The data a driver returns: `CommandSpec` (built with `CommandSpecBuilder`), `EnvEntry` and `EnvValue`, `StopSpec` with `Action`, `StopSignal` and `HttpMethod`, `CollectSpec`, `Capabilities` with `BudgetCaps`, `ProcessExit` and `ProcessEnd`, `Observed`, `Framing`, `FormatVersion`, `Confidence`, `HarnessId`, `Prepared` and `PreparedFile`, and the read-only views `Cell` and `Workspace`. |
| `driver` | `HarnessDriver`, the trait that says how to prepare, launch, observe, stop and collect one harness, and `DriverError`. |
| `reader` | `TraceReader`, the trait that turns the frames of one stream into normalised sessions and events, its input `FrameIn` and its error `ReaderError`. A reader is synchronous and holds no clock; the core supplies each frame with its place and wall time. |

## Dependencies

`candler-schemas` for the wire types a reader returns, `semver`, `serde`, `serde_json` and
`thiserror`. The crate does not depend on `candler-core`; the core depends on it.

## Guard files

| File | Pins |
|---|---|
| `tests/guard_two_shapes.rs` | two fake harnesses of opposite shapes (a Claude Code-shaped line stream with a live reader, an OpenCode-shaped exported document without one) drive both traits |
| `tests/public_surface.rs` | the public items other crates consume, used from outside; usage only |

```sh
cargo nextest run -p candler-harness
```

## Implementations

`candler-harness-claude-code` implements `TraceReader` for Claude Code stream-json output. No
driver implementation is in the workspace.
