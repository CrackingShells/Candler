# design_record, leaf report v0

Doc-only leaf: `__design__/architecture.md` (R01) records M3's decisions D2–D10 as they were finally ruled. D1, how the changes land, is development process and stays out of R01. Implementer: Sonnet. Wall clock: 3 min, then a 3-min fix round. Branch `m3/design_record` (5 `docs(design)` commits, head f897d8d), merged as `merge(design): land design_record`.

## Sections

| Section | Change |
|---|---|
| `### Refusals` (new) | code `<surface>.<reason>`, side, RFC 6901 path; `refusal/1`; the `Refusal` trait; append-only `codes.toml`; `candler.internal` and `[internal]`; exit codes 0/1/2; records Candler writes stay lenient (cites §Wire encodings) |
| `### Contract pages` (new) | classes derived from committed schemas; `x-candler-class`; the `contract_pages` drift test; `[helper]`; the driver-traits slot |
| `### CLI surface over the jobs` | `--error-format`; `ls` final; `ingest` (synchronous, `import` hidden alias); `resolve`, `inspect`, `analyze` (experiment by digest), `evaluate`; Terminals paragraph |
| §Operation API | the `ingest` row runs in the caller |
| `### Job` | evaluate job, `JobSpecFile`, `evaluation.planned` before the first outcome |
| `### Budget`, `### Configuration` | two slot pools; `slots.evaluators` defaults to half the CPUs, minimum 1, refused in a project config; `plugins.dirs[]` |
| `### Plug-in manifest` | plug-ins from `plugins.dirs[]`; `plugin.toml` or `candler-plugin.toml`; zero or two manifests refused |
| `### Analysis statistics` (new) | the run as the unit; Wilson for 0/1 weights only (coverage floor 0.91, exact equality to scipy's table: 0.9185 at n=5, p=0.1); Fisher two-sided with tie tolerance 1e-7; Newcombe; Holm per role per call; α 0.05 `core_default`; defaults, not the analysis |
| `### Workspace targets` (new) | Linux and macOS, WSL2, one `compile_error!`, the 21-file inventory with its guard, the Windows counterparts, a native port after M7 |
| §Wire encodings, "Unknown fields" row | records Candler writes ignore unknown fields; the three author documents refuse them |
| `## Deliberately open` | "where `ls` looks" and "how plug-ins are registered" removed |

## Gates (base 2ef954e)
OWNED, §Process supervision untouched, the four removals, the 54-row terms loop, no code/schema/roadmap moved, level-2 heading count, SUBJ: all PASS.

## Defects found and fixed
- The leaf asked for a `### Landing order` section. It was process talk, so it was removed from both the leaf and R01.
- The `ls` term of §Deliberately open could not hold together with its own deletion, so it was dropped.
- §Wire encodings contradicted the new refusal of unknown fields, so it was reworded.

## Open
- The `[nl]` gate that greps the names R01 gives against `crates/`, `docs/` and the leaves runs after L2.
