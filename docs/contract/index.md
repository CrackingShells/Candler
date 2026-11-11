# Contract pages

One page per surface an author writes or a plug-in speaks. Each page holds the sections Surface, Version, Fields, Refusals and Notes, in that order. Version and Fields are generated from the committed schema files in `crates/candler-schemas/schema/`.

## Pages

| Page | Surface | Written by | Wire tag |
|---|---|---|---|
| [plugin-manifest.md](plugin-manifest.md) | Plug-in manifest | plug-in author | `plugin-manifest/1` |
| [suite-file.md](suite-file.md) | Suite file | suite author | `suite/1` |
| [evaluator-protocol.md](evaluator-protocol.md) | Evaluator protocol | Candler and the evaluator plug-in | none |
| [outcome-record.md](outcome-record.md) | Outcome record | evaluator plug-in; Candler fills the rest | none |
| [experiment-file.md](experiment-file.md) | Experiment file | experimenter | `experiment/1` |
| [evaluator-helper.md](evaluator-helper.md) | Evaluator helper | evaluator kind author | none |
| [driver-traits.md](driver-traits.md) | Driver traits | driver author, by hand | none |

`driver-traits.md` is written by hand and is not generated.

## Field classes

Every row of a Fields table has one class.

| Class | Meaning |
|---|---|
| `required` | An author sets it, and the object's `required` lists it. |
| `optional` | An author may set it, and `required` does not list it. |
| `candler` | Candler writes it; an author never sets it. |
| `free` | Stored and never read; it needs no schema of its own. |

## Compatibility

<!-- versions -->
| Line | Version | Manifest |
|---|---|---|
| core | `v0.6.0` | `Cargo.toml` |
| wire schemas | `schemas@0.6.0` | `crates/candler-schemas/Cargo.toml` |
| evaluator helper | `candler-evaluator@0.1.0` | `python/candler-evaluator/pyproject.toml` |
<!-- /versions -->
