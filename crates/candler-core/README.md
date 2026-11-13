# candler-core

The core library of Candler: the parts that resolve an experiment, supervise processes, store runs
and import legacy results. It holds no domain knowledge of any instrument and spawns no harness of
its own choosing: a harness arrives through the `candler-harness` traits, and the wire records come
from `candler-schemas`. Layering is [R01 §Workspace](../../__design__/architecture.md).

## Public modules

`lib.rs` declares the modules and re-exports nothing. Callers write `candler_core::store::Store`.

| Module | Holds |
|---|---|
| `resolve` | `config` (`candler.toml` and `config.toml`), `layers` (six layers, one value per path with its source) and `preflight` (steps, reports and the `Resolver` that runs them). |
| `supervise` | `Supervisor` and `EventSink`. `spawn` starts a child in its own process group, `record` writes its child record, `output` pumps its streams into frame files, `stop` runs the escalation ladder, `reap` ends a group known only by its record, `platform` reads process start times. Unix only. |
| `frames` | `FrameWriter`, `FrameReader` and `frame_bytes`: `raw-frames/1` files, written as `<stream>.jsonl.tmp` and finished as `<stream>.jsonl.gz`. |
| `trace` | `normalise`: feeds the frames of a stored stream to a `TraceReader` and returns the sessions. |
| `store` | `Store` with `layout` (every path), `blob` (write-once content), `run` (a run written step by step and sealed by `run.done`) and `outcome` (outcome files, never replaced). Unix only. |
| `import` | `assemble::import` writes a legacy results folder into a store. `read` types the folder, `manifest` maps runs to run manifests, `grading` maps rows to outcome records, `assets` and `streams` import files and traces, `rules` carries the caller's domain names, `report` states every difference. |

## Feature

`test-support` exposes the signal guard's `Signaller`, `Supervisor::with_signaller`,
`Supervisor::kill_all`, `Supervisor::holds_group` and `supervise::http`. It is off by default. The
crate's own tests turn it on through the dev-dependency on itself; no other crate enables it.
`candler-harness-claude-code` is a dev-dependency of this crate and reaches the tests only.

## Guard files

Each file under `tests/` pins one module or seam.

| File | Pins |
|---|---|
| `guard_resolve.rs` | configuration loading, layering and preflight |
| `guard_signal.rs` | the signal guard; it never spawns a process |
| `guard_supervise.rs` | records, liveness, events, stops and reaping; it spawns |
| `guard_frames.rs` | the framing property and the literal frame cases |
| `guard_store.rs` | what a stopped writer leaves, append-only files, the frozen layout |
| `guard_seams.rs` | one run from spawn to its stored outcomes through a fake harness; it spawns |
| `guard_legacy_mapping.rs` | legacy runs to run manifests and outcome records, over the mini-corpus |
| `guard_import_mini.rs` | the import into a store, over the mini-corpus |
| `guard_golden.rs` | the import of the 240-run golden corpus; both tests are `#[ignore]` |
| `guard_oracle_mini.rs`, `guard_oracle_aggregates.rs` | the independent oracle over the mini-corpus and over the golden corpus (the second is ignored) |
| `guard_layering_i1.rs` | the instrument's domain stays out of the core, the harness crate and the schemas |
| `guard_assert_clean.rs` | the scan helper the scan guards share |

`tests/support/` holds the shared helpers, `tests/oracle/` the oracle and its vectors, and
`tests/fixtures/` the data ([fixtures/README.md](tests/fixtures/README.md)).

## Running the guards

```sh
cargo nextest run -p candler-core
```

The golden guards read the corpus named by `CANDLER_GOLDEN_CORPUS` and write their report to
`CANDLER_GOLDEN_REPORT`:

```sh
CANDLER_GOLDEN_CORPUS=<corpus> CANDLER_GOLDEN_REPORT=<file> \
  cargo nextest run -p candler-core --run-ignored only -E 'test(/^guard_golden::/)'
```

`guard_supervise` and `guard_seams` spawn processes. Run `guard_signal` first and only spawn when it
passes against unmodified `src/`.
